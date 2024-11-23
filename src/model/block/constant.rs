use super::{Event, TaskData};
use crate::{event_loop::current_layer, util::cmd::run_cmd};
use futures::{future::BoxFuture, FutureExt};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy)]
pub struct Static;

impl super::BlockTask for Static {
    fn start(&self, events: broadcast::Receiver<Event>, data: TaskData) -> BoxFuture<'static, ()> {
        start(events, data).boxed()
    }
}

async fn start(
    mut events: broadcast::Receiver<Event>,
    TaskData {
        block_name,
        cmd,
        monitors,
        bid,
        updates,
        actions,
        ..
    }: TaskData,
) {
    for mon in monitors.iter() {
        let _ = updates.send((cmd.to_owned(), bid, mon)).await;
    }
    while let Ok(e) = events.recv().await {
        match e {
            Event::MouseClicked(id, mon, button) if id == bid => {
                if let Some(a) = actions[button] {
                    let _ = run_cmd(block_name.title, a, mon.into(), current_layer()).await;
                }
            }
            Event::Signal | Event::NewLayer | Event::MouseClicked(..) => {}
        }
    }
}
