use std::time::Duration;

use chrono::{offset::Local, Timelike};
use tokio::{sync::broadcast::Receiver, time::timeout};

use crate::{
    event_loop::{current_layer, Event},
    model::{
        block::{BlockTask, TaskData},
        AffectedMonitor,
    },
};
use futures::{future::BoxFuture, FutureExt};

#[derive(Debug, Clone, Copy)]
pub struct Clock;

async fn start(mut events: Receiver<Event>, TaskData { updates, bid, .. }: TaskData) {
    loop {
        let layer = current_layer();
        let out = Local::now()
            .format(if layer == 0 {
                "%d/%m %H:%M"
            } else {
                "%a %d %b %T %Z %Y"
            })
            .to_string();
        if updates
            .send((out, bid, AffectedMonitor::All))
            .await
            .is_err()
        {
            log::info!("clock shutting down")
        }
        match timeout(dur_to_next_tick(layer), events.recv()).await {
            Ok(Ok(Event::NewLayer)) => {}
            Ok(Ok(_)) => continue,
            Ok(Err(_)) => return,
            Err(_) => {}
        }
    }
}

impl BlockTask for Clock {
    fn start(&self, events: Receiver<Event>, td: TaskData) -> BoxFuture<'static, ()> {
        start(events, td).boxed()
    }
}

fn dur_to_next_tick(layer: u16) -> Duration {
    if layer == 0 {
        Duration::from_secs((60 - Local::now().time().second()).into())
    } else {
        Duration::from_millis(
            1000u32
                .saturating_sub(Local::now().timestamp_subsec_millis())
                .into(),
        )
    }
}
