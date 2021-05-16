use super::{Event, CURRENT_LAYER};
use crate::global_config;
use std::sync::{atomic::Ordering, Arc};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::{broadcast::Sender, Mutex},
    task,
};

pub fn refresh(events: Arc<Mutex<Sender<Event>>>) {
    task::spawn({
        async move {
            let mut signals = match signal(SignalKind::user_defined1()) {
                Ok(s) => s,
                Err(e) => panic!("signal task failed: {:?}", e),
            };
            while signals.recv().await.is_some() {
                if events.lock().await.send(Event::Signal).is_err() {
                    break;
                }
            }
        }
    });
}

pub fn layer(events: Arc<Mutex<Sender<Event>>>) {
    task::spawn(async move {
        let mut signals = match signal(SignalKind::user_defined2()) {
            Ok(s) => s,
            Err(e) => panic!("layer task failed: {:?}", e),
        };
        while signals.recv().await.is_some() {
            let n_layers = global_config::get().n_layers;
            CURRENT_LAYER
                .fetch_update(Ordering::Release, Ordering::Relaxed, move |c| {
                    Some((c + 1) % n_layers)
                })
                .unwrap();
            if events.lock().await.send(Event::NewLayer).is_err() {
                break;
            }
        }
    });
}
