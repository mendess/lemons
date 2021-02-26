use super::Event;
use crate::Config;
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc, Weak,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub fn timed_blocks(
    config: Weak<Config<'static>>,
    layer: Weak<AtomicU16>,
    ch: &mpsc::Sender<Event>,
) -> JoinHandle<()> {
    let ch = ch.clone();
    thread::spawn(move || {
        while let (Some(c), Some(layer)) = (config.upgrade(), layer.upgrade()) {
            let layer = layer.load(Ordering::Relaxed);
            for block in c
                .values()
                .flatten()
                .filter(|b| b.layer == layer)
            {
                if let Some((interval, mut b_timer)) = block
                    .interval
                    .as_ref()
                    .map(|(i, l)| (i, l.write().unwrap()))
                {
                    match b_timer.checked_sub(Duration::from_secs(1)) {
                        Some(d) => *b_timer = d,
                        None => {
                            block.content.update(layer);
                            *b_timer = *interval;
                        }
                    }
                }
            }
            if let Err(_) = ch.send(Event::Update) {
                eprintln!("Exiting timer blocks thread");
                return;
            }
            thread::sleep(Duration::from_secs(1))
        }
    })
}
