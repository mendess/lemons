use std::time::Duration;

use chrono::{Timelike, Utc};
use tokio::{sync::broadcast::Sender, time::{sleep, timeout}};

use crate::{event_loop::{Event, current_layer}, model::block::{BlockTask, TaskData}};

#[derive(Debug)]
pub struct Clock;

impl BlockTask for Clock {
    fn start(&self, sender: &Sender<Event>, TaskData { updates, bid, .. }: TaskData) {
        let mut events = sender.subscribe();
        tokio::spawn(async move {
            loop {
                let layer = current_layer();
                let out = Utc::now()
                    .format(if layer == 0 {
                        "%d/%m %H:%M"
                    } else {
                        "%a %d %b %T %Z %Y"
                    })
                    .to_string();
                if updates.send((out, bid, u8::MAX).into()).await.is_err() {
                    log::info!("clock shutting down")
                }
                match timeout(dur_to_next_tick(layer), events.recv()).await {
                    Ok(Ok(Event::Refresh | Event::NewLayer)) => {},
                    Ok(Ok(_)) => continue,
                    Ok(Err(e)) => {
                        log::error!("Failed to receive events: {:?}", e);
                        sleep(dur_to_next_tick(layer)).await;
                    }
                    Err(_) => {},
                }
            }
        });
    }
}

fn dur_to_next_tick(layer: u16) -> Duration {
    if layer == 0 {
        Duration::from_secs((60 - Utc::now().time().second()).into())
    } else {
        Duration::from_millis(1000u32.saturating_sub(Utc::now().timestamp_subsec_millis()).into())
    }
}
