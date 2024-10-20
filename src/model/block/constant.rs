use super::{Event, TaskData};
use crate::{event_loop::current_layer, util::cmd::run_cmd};
use futures::stream::{self, StreamExt};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy)]
pub struct Static;

impl super::BlockTask for Static {
    fn start(&self, events: &broadcast::Sender<Event>, data: TaskData) {
        let mut events = events.subscribe();
        let TaskData {
            block_name,
            cmd,
            monitors,
            bid,
            updates,
            actions,
            ..
        } = data;
        tokio::spawn(async move {
            stream::iter(monitors.iter())
                .fold(updates, |updates, mon| async move {
                    let _ = updates.send((cmd.to_owned(), bid, mon)).await;
                    updates
                })
                .await;
            while let Ok(e) = events.recv().await {
                match e {
                    Event::MouseClicked(id, mon, button) if id == bid => {
                        if let Some(a) = actions[button] {
                            let _ = run_cmd(block_name, a, mon.into(), current_layer()).await;
                        }
                    }
                    Event::Signal | Event::NewLayer | Event::MouseClicked(..) => {}
                }
            }
        });
    }
}
