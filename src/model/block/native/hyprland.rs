use core::fmt;
use std::sync::Arc;

use futures::{future::BoxFuture, FutureExt};
use hyprland::{
    data::{Clients, Workspaces},
    event_listener::{
        AsyncEventListener, MonitorEventData, WindowMoveEvent, WindowOpenEvent,
        WorkspaceRenameEventData,
    },
    shared::{Address, HyprData, HyprDataActive, WorkspaceId, WorkspaceType},
};
use tokio::sync::{broadcast, Mutex};

use crate::{
    display::{zelbar::ZelbarDisplayBlock, DisplayBlock},
    event_loop::{update_channel::UpdateChannel, Event},
    global_config,
    model::{
        block::{BlockId, BlockTask, TaskData},
        Color,
    },
};

#[derive(Debug)]
pub struct HyprLand;

impl BlockTask for HyprLand {
    fn start(
        &self,
        _events: &broadcast::Sender<Event>,
        TaskData {
            updates,
            bid,
            monitors,
            ..
        }: TaskData,
    ) {
        for mon in monitors {
            let updates = updates.clone();
            tokio::spawn(async move {
                let state = {
                    let m = Monitor::new(updates, bid, mon).await?;
                    if let Err(e) = m.emit().await {
                        log::error!("failed to emit event: {e:?}");
                    };
                    Arc::new(Mutex::new(m))
                };
                let mut listener = AsyncEventListener::new();
                listener.add_urgent_state_handler(handler(
                    &state,
                    |state, address: Address| async move {
                        let mut state = state.lock().await;
                        if let Some(ws) = state.find_ws_containing(&address) {
                            ws.urgent = true;
                        };
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
                            ws.windows.push(open_event.window_address);
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
                            Ok(Some(w)) => w.windows.push(move_event.window_address),
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
                    move |state, ws: WorkspaceType| async move {
                        if let WorkspaceType::Regular(ws_name) = ws {
                            let mut state = state.lock().await;
                            state.ws.retain(|ws| ws.name != ws_name);
                            if let Err(e) = state.emit().await {
                                log::error!("failed to emit event: {e:?}");
                            };
                        }
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

                listener.start_listener_async().await
            });
        }
    }
}

#[derive(Debug)]
struct Ws {
    id: WorkspaceId,
    name: String,
    urgent: bool,
    windows: Vec<Address>,
}

struct Monitor {
    ws: Vec<Ws>,
    visible_ws: Option<WorkspaceId>,
    is_focused: bool,

    sender: UpdateChannel,
    block_id: BlockId,
    monitor: u8,
}

impl Monitor {
    async fn new(sender: UpdateChannel, block_id: BlockId, monitor: u8) -> hyprland::Result<Self> {
        let (ws, clients, active_monitor) = tokio::try_join!(
            Workspaces::get_async(),
            Clients::get_async(),
            hyprland::data::Monitor::get_active_async()
        )?;
        let mut ws = ws
            .filter(|w| w.id > 0)
            .filter(|w| w.monitor_id == monitor)
            .map(|w| Ws {
                id: w.id,
                name: w.name,
                urgent: false,
                windows: Default::default(),
            })
            .collect::<Vec<_>>();

        for c in clients {
            let Some(w) = ws.iter_mut().find(|w| w.id == c.workspace.id) else {
                continue;
            };
            w.windows.push(c.address);
        }

        Ok(Self {
            visible_ws: ws.first().map(|w| w.id),
            ws,
            is_focused: active_monitor.id == i128::from(monitor),
            sender,
            block_id,
            monitor,
        })
    }

    fn find_ws_containing(&mut self, addr: &Address) -> Option<&mut Ws> {
        self.ws
            .iter_mut()
            .find(|ws| ws.windows.iter().any(|w| w == addr))
    }

    fn find_ws_by_name(&mut self, name: &str) -> Option<&mut Ws> {
        self.ws.iter_mut().find(|ws| ws.name == name)
    }

    async fn find_ws_by_name_or_create(&mut self, name: &str) -> hyprland::Result<Option<&mut Ws>> {
        match self.ws.iter().position(|ws| ws.name == name) {
            Some(idx) => return Ok(self.ws.get_mut(idx)),
            None if self.is_focused => {
                let w = match Workspaces::get_async().await?.find(|w| w.name == name) {
                    Some(w) => w,
                    None => return Ok(None),
                };
                let id = w.id;
                self.ws.push(Ws {
                    id: w.id,
                    name: w.name,
                    urgent: false,
                    windows: Default::default(),
                });
                self.ws.sort_by_key(|w| w.id);
                Ok(self.ws.iter_mut().rfind(|w| w.id == id))
            }
            None => Ok(None),
        }
    }

    fn remove_window(&mut self, addr: &Address) {
        self.ws
            .iter_mut()
            .for_each(|w| w.windows.retain(|w_addr| w_addr != addr));
    }

    async fn emit(&self) -> fmt::Result {
        let global_conf = global_config::get();
        let mut text = String::new();
        for w in &self.ws {
            let mut block = ZelbarDisplayBlock::new_raw(&mut text);
            match (self.visible_ws, self.is_focused) {
                // visible and on focused monitor
                (Some(wid), true) if wid == w.id => {
                    block.fg(global_conf.get_color("black").unwrap_or(&Color::BLACK))?;
                    block.bg(global_conf.get_color("blue").unwrap_or(&Color::BLUE))?;
                }
                // visible but not on focused monitor
                (Some(wid), false) if wid == w.id => {
                    block.fg(global_conf.get_color("black").unwrap_or(&Color::BLACK))?;
                    block.bg(global_conf.get_color("green").unwrap_or(&Color::GREEN))?;
                }
                // not visible on focused monitor
                (_, true) => {
                    block.fg(global_conf.get_color("white").unwrap_or(&Color::WHITE))?;
                    block.underline(global_conf.get_color("blue").unwrap_or(&Color::BLUE))?;
                }
                // not visible and not on focused monitor
                (_, false) => {
                    block.fg(global_conf.get_color("white").unwrap_or(&Color::WHITE))?;
                    block.underline(global_conf.get_color("green").unwrap_or(&Color::GREEN))?;
                }
            }
            block.text(" ", false)?;
            block.text(&w.name, false)?;
            block.text(" ", false)?;
            block.finish()?;
        }
        if log::max_level() >= log::Level::Info {
            log::info!(
                "{:?} => {:?}",
                self.ws.iter().map(|w| w.id).collect::<Vec<_>>(),
                self.visible_ws
            );
        }
        self.sender
            .send(crate::model::block::BlockUpdate {
                text,
                alignment: self.block_id.0,
                index: self.block_id.1,
                monitor: self.monitor,
            })
            .await
            .unwrap();
        Ok(())
    }
}

fn handler<D, F, Fut>(state: &Arc<Mutex<Monitor>>, f: F) -> impl Fn(D) -> BoxFuture<'static, ()>
where
    F: Fn(Arc<Mutex<Monitor>>, D) -> Fut + Send,
    D: Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = state.clone();
    move |data| f(state.clone(), data).boxed()
}
