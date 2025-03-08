use core::fmt;
use std::sync::Arc;

use futures::{future::BoxFuture, stream, FutureExt, StreamExt, TryFutureExt};
use hyprland::{
    data::{Clients, Workspaces},
    event_listener::{
        AsyncEventListener, MonitorEventData, WindowEventData, WindowMoveEvent, WindowOpenEvent,
        WorkspaceDestroyedEventData, WorkspaceRenameEventData,
    },
    shared::{Address, HyprData, HyprDataActive, WorkspaceId, WorkspaceType},
};
use tokio::sync::{broadcast, oneshot, Mutex};

use crate::{
    event_loop::{update_task::UpdateChannel, Event},
    global_config,
    model::{
        block::{BlockId, BlockTask, TaskData, TextDecorations},
        Color,
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
    let hypr = stream::iter(monitors.iter())
        .map(|m| match m {
            crate::model::AffectedMonitor::Single(n) => n,
            crate::model::AffectedMonitor::All => {
                unreachable!("multi_monitor should always be enabled for this block")
            }
        })
        .map(|mon| {
            let mut updates = updates.clone();
            async move {
                let (state, cancelation) = {
                    let (mut m, cancelation) = {
                        loop {
                            match Monitor::new(updates, bid, mon.into()).await {
                                Ok(m) => break m,
                                Err((ch, e)) => {
                                    updates = ch;
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
                    (Arc::new(Mutex::new(m)), cancelation)
                };
                let mut listener = AsyncEventListener::new();
                listener.add_urgent_state_handler(handler(
                    &state,
                    |state, address: Address| async move {
                        let mut state = state.lock().await;
                        let Some(ws) = state.find_window(&address) else {
                            return;
                        };
                        ws.urgent = true;
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_active_window_change_handler(handler(
                    &state,
                    |state, event: Option<WindowEventData>| async move {
                        let mut state = state.lock().await;
                        let Some(event) = event else {
                            return;
                        };
                        let Some(w) = state.find_window(&event.window_address) else {
                            return;
                        };
                        w.urgent = false;
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_window_open_handler(handler(
                    &state,
                    |state, open_event: WindowOpenEvent| async move {
                        let mut state = state.lock().await;
                        if let Some(ws) = state.find_ws_by_name(&open_event.workspace_name) {
                            ws.windows.push(open_event.window_address.into());
                        }
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_window_moved_handler(handler(
                    &state,
                    |state, move_event: WindowMoveEvent| async move {
                        let mut state = state.lock().await;
                        state.remove_window(&move_event.window_address);
                        match state
                            .find_ws_by_name_or_create(&move_event.workspace_name)
                            .await
                        {
                            Ok(Some(w)) => w.windows.push(move_event.window_address.into()),
                            Ok(None) => {}
                            Err(e) => log::error!("failed to create target ws: {e:?}"),
                        }
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_window_close_handler(handler(
                    &state,
                    |state, address: Address| async move {
                        let mut state = state.lock().await;
                        state.remove_window(&address);
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_workspace_added_handler(handler(
                    &state,
                    move |state, ws: WorkspaceType| async move {
                        if let WorkspaceType::Regular(ws_name) = ws {
                            let mut state = state.lock().await;
                            match state.find_ws_by_name_or_create(&ws_name).await {
                                Ok(Some(_) | None) => {}
                                Err(e) => log::error!("creating the workspace: {e:?}"),
                            };
                            if let Err(e) = state.emit().await {
                                log::error!("failed to emit event: {e:?}");
                            };
                        }
                    },
                ));
                listener.add_workspace_destroy_handler(handler(
                    &state,
                    move |state, event: WorkspaceDestroyedEventData| async move {
                        let mut state = state.lock().await;
                        state.ws.retain(|ws| ws.name != event.workspace_name);
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));
                listener.add_workspace_change_handler(handler(
                    &state,
                    move |state, ws: WorkspaceType| async move {
                        if let WorkspaceType::Regular(ws_name) = ws {
                            let mut state = state.lock().await;
                            if let Some(ws) = state.find_ws_by_name(&ws_name) {
                                state.visible_ws = Some(ws.id);
                            }
                            if let Err(e) = state.emit().await {
                                log::error!("failed to emit event: {e:?}");
                            };
                        }
                    },
                ));
                listener.add_workspace_rename_handler(handler(
                    &state,
                    move |state, ws: WorkspaceRenameEventData| async move {
                        let mut state = state.lock().await;
                        if let Some(w) = state.ws.iter_mut().find(|w| w.id == ws.workspace_id) {
                            w.name = ws.workspace_name;
                            if let Err(e) = state.emit().await {
                                log::error!("failed to emit event: {e:?}");
                            };
                        }
                    },
                ));
                listener.add_active_monitor_change_handler(handler(
                    &state,
                    move |state, m: MonitorEventData| async move {
                        let mut state = state.lock().await;
                        if let WorkspaceType::Regular(ws_name) = m.workspace {
                            state.is_focused = state.find_ws_by_name(&ws_name).is_some();
                        }
                        if let Err(e) = state.emit().await {
                            log::error!("failed to emit event: {e:?}");
                        };
                    },
                ));

                tokio::select! {
                    e = cancelation => {
                        log::error!("hyprland module shutting down: {e:?}");
                        Ok(())
                    }
                    r = listener.start_listener_async() => r,
                }
            }
        })
        .zip(stream::iter(monitors.iter()))
        .map(|(fut, mon)| async move { (mon, fut.await) })
        .buffered(monitors.len().get())
        .for_each(|(mon, result)| async move {
            if let Err(e) = result {
                log::error!("hyprland daemon for monitor {mon} failed: {e}")
            }
        });

    let cancel = async { while events.recv().await.is_ok() {} };

    tokio::select! {
        _ = hypr => {}
        _ = cancel => {}
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
    ) -> Result<
        (Self, oneshot::Receiver<CancelationError>),
        (UpdateChannel, hyprland::shared::HyprError),
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
            Err(e) => return Err((sender, e)),
        };
        let mut ws = ws
            .into_iter()
            .filter(|w| w.id > 0)
            .filter(|w| w.monitor_id == monitor)
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
                if new_ws.monitor_id == self.monitor.into() {
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

    fn remove_window(&mut self, addr: &Address) {
        self.ws
            .iter_mut()
            .for_each(|w| w.windows.retain(|w| w.addr != *addr));
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
        {
            if let Some(c) = self.cancelation.take() {
                let _ = c.send(Box::new(e));
            }
        }
        Ok(())
    }
}

fn handler<D, F, Fut>(
    state: &Arc<Mutex<Monitor>>,
    f: F,
) -> impl Fn(D) -> VoidFuture + use<D, F, Fut>
where
    F: Fn(Arc<Mutex<Monitor>>, D) -> Fut + Send,
    D: Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = state.clone();
    move |data| f(state.clone(), data).boxed()
}
