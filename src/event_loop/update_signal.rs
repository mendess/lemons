use super::Event;
use crate::Config;
use signal_hook::{consts::signal::SIGUSR1, iterator::Signals};
use std::{
    sync::{mpsc, atomic::{AtomicU16, Ordering}, Weak},
    thread::{self, JoinHandle},
};

pub fn update_signal(config: Weak<Config<'static>>, layer: Weak<AtomicU16>, ch: &mpsc::Sender<Event>) -> JoinHandle<()> {
    let ch = ch.clone();
    let signal_thread = thread::spawn(move || {
        for _ in &mut Signals::new(&[SIGUSR1]).unwrap() {
            if let (Some(c), Some(layer)) = (config.upgrade(), layer.upgrade()) {
                let layer = layer.load(Ordering::Relaxed);
                c.values().flatten().filter(|b| b.signal).for_each(|b| {
                    b.content.update(layer);
                });
                if let Err(_) = ch.send(Event::Update) {
                    eprintln!("Exiting signal thread");
                    return;
                }
            } else {
                break;
            }
        }
    });
    signal_thread
}
