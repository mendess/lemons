use core::fmt;
use std::sync::Arc;

use futures::{
    FutureExt, StreamExt, TryFutureExt,
    future::BoxFuture,
    stream::{self, FuturesUnordered},
};
use hyprland::{
    data::{Clients, Workspaces},
    event_listener::{
        AsyncEventListener, MonitorEventData, NonSpecialWorkspaceEventData, WindowEventData,
        WindowMoveEvent, WindowOpenEvent, WorkspaceEventData, WorkspaceMovedEventData,
    },
    shared::{Address, HyprData, HyprDataActive, WorkspaceId, WorkspaceType},
};
use tokio::sync::{Mutex, broadcast, oneshot};

use crate::{
    event_loop::{Event, update_task::UpdateChannel},
    global_config,
    model::{
        Color,
        block::{BlockId, BlockTask, TaskData, TextDecorations},
    },
};

pub(crate) type VoidFuture = BoxFuture<'static, ()>;

#[derive(Debug, Clone, Copy)]
pub struct HyprLand;

impl BlockTask for HyprLand {
    fn start(&self, events: broadcast::Receiver<Event>, td: TaskData) -> BoxFuture<'static, ()> {
        start(events, td).boxed()
    }
}

async fn start(
    mut events: broadcast::Receiver<Event>,
    TaskData {
        updates,
        bid,
        monitors,
        ..
    }: TaskData,
) {
    let hypr_monitors = loop {
        match hyprland::data::Monitors::get_async().await {
            Ok(ms) => break ms,
            Err(e) => {
                log::error!("failed to get monitors: {e:?}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    };
    let (monitors, mut cancelations) = stream::iter(monitors.iter())
        .map(|m| match m {
            crate::model::AffectedMonitor::Single(n) => n,
            crate::model::AffectedMonitor::All => {
                unreachable!("multi_monitor should always be enabled for this block")
            }
        })
        .zip(stream::iter(hypr_monitors))
        .then(|(mon, mut hypr_mon)| {
            let mut updates = updates.clone();
            async move {
                let (mut m, cancelation) = {
                    loop {
                        match Monitor::new(updates, bid, mon.into(), hypr_mon).await {
                            Ok(m) => break m,
                            Err(((ch, m), e)) => {
                                updates = ch;
                                hypr_mon = m;
                                let error_update =
                                    (format!("failed to get monitor: {e}"), bid, mon);
                                updates
                                    .send(error_update)
                                    .await
                                    .expect("failed to send block update");
                                log::error!("failed getting monitor {e:?}");
                                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                            }
                        }
                    }
                };
                if let Err(e) = m.emit().await {
                    log::error!("failed to emit event: {e:?}");
                };
                (m, cancelation)
            }
        })
        .collect::<(Vec<_>, FuturesUnordered<_>)>()
        .await;
    let monitors = Arc::new(Mutex::new(Monitors { monitors }));
    let mut listener = AsyncEventListener::new();
    listener.add_urgent_state_changed_handler(handler(
        &monitors,
        |monitors, address: Address| async move {
            log::trace!("add_urgent_state_changed_handler: {address:?}");
            let mut monitors = monitors.lock().await;
            let Some((id, ws)) = monitors.find_window(&address) else {
                return;
            };
            ws.urgent = true;
            if let Err(e) = monitors.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            };
        },
    ));
    listener.add_active_window_changed_handler(handler(
        &monitors,
        |state, event: Option<WindowEventData>| async move {
            log::trace!("add_active_window_changed_handler: {event:?}");
            let mut state = state.lock().await;
            let Some(event) = event else {
                return;
            };
            let Some((id, w)) = state.find_window(&event.address) else {
                return;
            };
            w.urgent = false;
            if let Err(e) = state.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            };
        },
    ));
    listener.add_window_opened_handler(handler(
        &monitors,
        |monitors, open_event: WindowOpenEvent| async move {
            log::trace!("add_window_opened_handler: {open_event:?}");
            let mut monitors = monitors.lock().await;
            let Some((id, ws)) = monitors.find_ws_by_name(&open_event.workspace_name) else {
                return;
            };
            ws.windows.push(open_event.window_address.into());
            if let Err(e) = monitors.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            };
        },
    ));
    listener.add_window_moved_handler(handler(
        &monitors,
        |monitors, move_event: WindowMoveEvent| async move {
            log::trace!("add_window_moved_handler: {move_event:?}");
            let WorkspaceType::Regular(ws_name) = move_event.workspace_name else {
                return;
            };
            let mut monitors = monitors.lock().await;
            monitors.remove_window(&move_event.window_address);
            match monitors.find_ws_by_name_or_create(&ws_name).await {
                Ok(Some((_, w))) => {
                    w.windows.push(move_event.window_address.into());
                }
                Ok(None) => {}
                Err(e) => log::error!("failed to create target ws: {e:?}"),
            }
            for m in monitors.monitors.iter_mut() {
                if let Err(e) = m.emit().await {
                    log::error!("failed to emit event: {e:?}");
                };
            }
        },
    ));
    listener.add_window_closed_handler(handler(
        &monitors,
        |monitors, address: Address| async move {
            log::trace!("add_window_closed_handler: {address:?}");
            let mut monitors = monitors.lock().await;
            let Some(id) = monitors.remove_window(&address) else {
                return;
            };
            if let Err(e) = monitors.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            };
        },
    ));
    listener.add_workspace_added_handler(handler(
        &monitors,
        move |monitors, ws: WorkspaceEventData| async move {
            log::trace!("add_workspace_added_handler: {ws:?}");
            if let WorkspaceType::Regular(ws_name) = ws.name {
                let mut monitors = monitors.lock().await;
                match monitors.find_ws_by_name_or_create(&ws_name).await {
                    Ok(Some((id, _))) => {
                        if let Err(e) = monitors.emit(id).await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    }
                    Ok(None) => {}
                    Err(e) => log::error!("creating the workspace: {e:?}"),
                };
            }
        },
    ));
    listener.add_workspace_deleted_handler(handler(
        &monitors,
        move |monitors, event: WorkspaceEventData| async move {
            log::trace!("add_workspace_deleted_handler: {event:?}");
            let WorkspaceType::Regular(ws_name) = event.name else {
                return;
            };
            let mut monitors = monitors.lock().await;
            for m in monitors.monitors.iter_mut() {
                m.ws.retain(|ws| ws.name != ws_name);
                if let Err(e) = m.emit().await {
                    log::error!("failed to emit event: {e:?}");
                };
            }
        },
    ));
    listener.add_workspace_changed_handler(handler(
        &monitors,
        move |monitors, ws: WorkspaceEventData| async move {
            log::trace!("add_workspace_changed_handler: {ws:?}");
            if let WorkspaceType::Regular(ws_name) = ws.name {
                let mut monitors = monitors.lock().await;
                let Some((id, ws)) = monitors.find_ws_by_name(&ws_name) else {
                    return;
                };
                let ws_id = ws.id;
                let Some(m) = monitors.find_by(|m| m.monitor == id) else {
                    return;
                };
                m.visible_ws = Some(ws_id);
                if let Err(e) = m.emit().await {
                    log::error!("failed to emit event: {e:?}");
                };
            }
        },
    ));
    listener.add_workspace_renamed_handler(handler(
        &monitors,
        move |monitors, ws: NonSpecialWorkspaceEventData| async move {
            log::trace!("add_workspace_renamed_handler: {ws:?}");
            let mut monitors = monitors.lock().await;
            let Some((id, w)) = monitors.find_map_by(|m| m.ws.iter_mut().find(|w| w.id == ws.id))
            else {
                return;
            };
            w.name = ws.name;
            if let Err(e) = monitors.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            };
        },
    ));
    listener.add_active_monitor_changed_handler(handler(
        &monitors,
        move |monitors, m: MonitorEventData| async move {
            log::trace!("add_active_monitor_changed_handler: {m:?}");
            let mut monitors = monitors.lock().await;
            let Some(WorkspaceType::Regular(ws_name)) = m.workspace_name else {
                return;
            };

            for m in monitors.monitors.iter_mut() {
                m.is_focused = m.find_ws_by_name(&ws_name).is_some();
                if let Err(e) = m.emit().await {
                    log::error!("failed to emit event: {e:?}");
                };
            }
        },
    ));
    listener.add_workspace_moved_handler(handler(
        &monitors,
        move |monitors, move_event: WorkspaceMovedEventData| async move {
            log::trace!("add_workspace_moved_handler: {move_event:?}");
            let WorkspaceType::Regular(ws_name) = move_event.name else {
                return;
            };
            let mut monitors = monitors.lock().await;
            let Some((id, moved_ws)) = monitors.find_map_by(|m| {
                let ws =
                    m.ws.iter()
                        .position(|w| w.name == ws_name)
                        .map(|i| m.ws.remove(i));
                if let Some(ws) = &ws
                    && m.visible_ws == Some(ws.id)
                {
                    m.visible_ws = None;
                }
                ws
            }) else {
                return;
            };
            let Some(m) = monitors.find_by(|m| m.name.as_ref() == move_event.monitor.as_str())
            else {
                return;
            };
            let ws_id = moved_ws.id;
            m.ws.push(moved_ws);
            m.ws.sort_by_key(|w| w.id);
            m.visible_ws = Some(ws_id);
            if let Err(e) = m.emit().await {
                log::error!("failed to emit event: {e:?}");
            };
            if let Err(e) = monitors.emit(id).await {
                log::error!("failed to emit event: {e:?}");
            }
        },
    ));

    let cancel = async { while events.recv().await.is_ok() {} };
    tokio::select! {
        e = cancelations.next() => {
            log::error!("hyprland module shutting down: {e:?}");
        }
        _ = cancel => {
            log::error!("hyprland module shutting down: no more events comming");
        }
        r = listener.start_listener_async() => {
            if let Err(e) = r {
                log::error!("hyprland daemon failed: {e}")
            }
        }
    }
}

struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    fn find_by(&mut self, f: impl Fn(&Monitor) -> bool) -> Option<&mut Monitor> {
        self.monitors.iter_mut().find(|m| f(m))
    }

    fn find_map_by<'s, R: 's>(
        &'s mut self,
        f: impl Fn(&'s mut Monitor) -> Option<R>,
    ) -> Option<(u8, R)> {
        self.monitors.iter_mut().find_map(|m| {
            let id = m.monitor;
            f(m).map(|r| (id, r))
        })
    }

    fn find_window(&mut self, addr: &Address) -> Option<(u8, &mut Window)> {
        for m in &mut self.monitors {
            let id = m.monitor;
            if let Some(w) = m.find_window(addr) {
                return Some((id, w));
            }
        }
        None
    }

    fn find_ws_by_name(&mut self, name: &str) -> Option<(u8, &mut Ws)> {
        for m in &mut self.monitors {
            let id = m.monitor;
            if let Some(w) = m.find_ws_by_name(name) {
                return Some((id, w));
            }
        }
        None
    }

    async fn find_ws_by_name_or_create(
        &mut self,
        name: &str,
    ) -> hyprland::Result<Option<(u8, &mut Ws)>> {
        for m in &mut self.monitors {
            let id = m.monitor;
            if let Some(w) = m.find_ws_by_name_or_create(name).await? {
                return Ok(Some((id, w)));
            }
        }
        Ok(None)
    }

    fn remove_window(&mut self, addr: &Address) -> Option<u8> {
        for m in &mut self.monitors {
            if m.remove_window(addr) {
                return Some(m.monitor);
            }
        }
        None
    }

    async fn emit(&mut self, id: u8) -> fmt::Result {
        self.monitors
            .iter_mut()
            .find(|m| m.monitor == id)
            .unwrap()
            .emit()
            .await
    }
}

#[derive(Debug)]
struct Window {
    addr: Address,
    urgent: bool,
}

impl From<Address> for Window {
    fn from(addr: Address) -> Self {
        Self {
            addr,
            urgent: false,
        }
    }
}

#[derive(Debug)]
struct Ws {
    id: WorkspaceId,
    name: String,
    windows: Vec<Window>,
}

struct Monitor {
    ws: Vec<Ws>,
    visible_ws: Option<WorkspaceId>,
    is_focused: bool,

    sender: UpdateChannel,
    block_id: BlockId,
    monitor: u8,
    name: Box<str>,
    /// Starts as true and turns to false as soon as a send error is encountered when emit is
    /// called.
    cancelation: Option<oneshot::Sender<CancelationError>>,
}

type CancelationError = Box<dyn std::error::Error + Send + Sync>;

impl Monitor {
    async fn new(
        sender: UpdateChannel,
        block_id: BlockId,
        monitor: i128,
        hypr_mon: hyprland::data::Monitor,
    ) -> Result<
        (Self, oneshot::Receiver<CancelationError>),
        (
            (UpdateChannel, hyprland::data::Monitor),
            hyprland::error::HyprError,
        ),
    > {
        let data = tokio::try_join!(
            Workspaces::get_async().map_err(|e| {
                log::error!("getting workspaces failed: {e:?}");
                e
            }),
            Clients::get_async().map_err(|e| {
                log::error!("getting clients failed: {e:?}");
                e
            }),
            hyprland::data::Monitor::get_active_async().map_err(|e| {
                log::error!("getting monitor failed: {e:?}");
                e
            })
        );
        let (ws, clients, active_monitor) = match data {
            Ok(x) => x,
            Err(e) => return Err(((sender, hypr_mon), e)),
        };
        let mut ws = ws
            .into_iter()
            .filter(|w| w.id > 0)
            .filter(|w| w.monitor_id == Some(monitor))
            .map(|w| Ws {
                id: w.id,
                name: w.name,
                windows: Default::default(),
            })
            .collect::<Vec<_>>();

        for c in clients {
            let Some(w) = ws.iter_mut().find(|w| w.id == c.workspace.id) else {
                continue;
            };
            w.windows.push(Window {
                addr: c.address,
                urgent: false,
            });
        }

        ws.sort_by_key(|w| w.id);

        let (cancelation_tx, cancelation_rx) = oneshot::channel();

        Ok((
            Self {
                visible_ws: ws.first().map(|w| w.id),
                ws,
                is_focused: active_monitor.id == monitor,
                sender,
                block_id,
                monitor: monitor.try_into().unwrap(),
                name: hypr_mon.name.into_boxed_str(),
                cancelation: Some(cancelation_tx),
            },
            cancelation_rx,
        ))
    }

    fn find_window(&mut self, addr: &Address) -> Option<&mut Window> {
        self.ws
            .iter_mut()
            .flat_map(|ws| ws.windows.iter_mut())
            .find(|w| w.addr == *addr)
    }

    fn find_ws_by_name(&mut self, name: &str) -> Option<&mut Ws> {
        self.ws.iter_mut().find(|ws| ws.name == name)
    }

    async fn find_ws_by_name_or_create(&mut self, name: &str) -> hyprland::Result<Option<&mut Ws>> {
        match self.ws.iter().position(|ws| ws.name == name) {
            Some(idx) => Ok(self.ws.get_mut(idx)),
            None => {
                let Some(new_ws) = Workspaces::get_async()
                    .await?
                    .into_iter()
                    .find(|w| w.name == name)
                else {
                    return Ok(None);
                };
                if new_ws.monitor_id == Some(self.monitor.into()) {
                    self.ws.push(Ws {
                        id: new_ws.id,
                        name: new_ws.name,
                        windows: Default::default(),
                    });
                    self.ws.sort_by_key(|w| w.id);
                    Ok(self.ws.iter_mut().rfind(|w| w.id == new_ws.id))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn remove_window(&mut self, addr: &Address) -> bool {
        self.ws
            .iter_mut()
            .any(|w| w.windows.extract_if(.., |w| w.addr == *addr).count() > 1)
    }

    async fn emit(&mut self) -> fmt::Result {
        let global_conf = global_config::get();
        let text = self
            .ws
            .iter()
            .map(|w| {
                let mut decorations = TextDecorations::default();
                if w.windows.iter().any(|w| w.urgent) {
                    decorations.fg = Some(*global_conf.get_color("black").unwrap_or(&Color::BLACK));
                    decorations.bg = Some(*global_conf.get_color("red").unwrap_or(&Color::RED));
                } else {
                    match (self.visible_ws, self.is_focused) {
                        // visible and on focused monitor
                        (Some(wid), true) if wid == w.id => {
                            decorations.fg =
                                Some(*global_conf.get_color("black").unwrap_or(&Color::BLACK));
                            decorations.bg =
                                Some(*global_conf.get_color("blue").unwrap_or(&Color::BLUE));
                        }
                        // visible but not on focused monitor
                        (Some(wid), false) if wid == w.id => {
                            decorations.fg =
                                Some(*global_conf.get_color("black").unwrap_or(&Color::BLACK));
                            decorations.bg =
                                Some(*global_conf.get_color("green").unwrap_or(&Color::GREEN));
                        }
                        // not visible on focused monitor
                        (_, true) => {
                            decorations.fg =
                                Some(*global_conf.get_color("white").unwrap_or(&Color::WHITE));
                            decorations.underline =
                                Some(*global_conf.get_color("blue").unwrap_or(&Color::BLUE));
                        }
                        // not visible and not on focused monitor
                        (_, false) => {
                            decorations.fg =
                                Some(*global_conf.get_color("white").unwrap_or(&Color::WHITE));
                            decorations.underline =
                                Some(*global_conf.get_color("green").unwrap_or(&Color::GREEN));
                        }
                    }
                }
                crate::model::block::BlockText {
                    decorations,
                    text: format!(" {} ", &w.name),
                }
            })
            .collect();
        if log::max_level() >= log::Level::Info {
            log::info!(
                "{:?} => {:?}",
                self.ws.iter().map(|w| w.id).collect::<Vec<_>>(),
                self.visible_ws
            );
        }
        if let Err(e) = self
            .sender
            .send(crate::model::block::BlockUpdate {
                text,
                alignment: self.block_id.0,
                index: self.block_id.1,
                monitor: self.monitor.into(),
            })
            .await
            && let Some(c) = self.cancelation.take()
        {
            let _ = c.send(Box::new(e));
        }
        Ok(())
    }
}

fn handler<D, F, Fut>(
    state: &Arc<Mutex<Monitors>>,
    f: F,
) -> impl Fn(D) -> VoidFuture + use<D, F, Fut>
where
    F: Fn(Arc<Mutex<Monitors>>, D) -> Fut + Send,
    D: Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = state.clone();
    move |data| f(state.clone(), data).boxed()
}
