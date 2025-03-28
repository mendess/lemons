use super::{CURRENT_LAYER, Event};
use crate::global_config;
use std::{future::pending, sync::atomic::Ordering};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::broadcast::Sender,
};

pub async fn refresh(events: Sender<Event>) {
    let mut signals = match signal(SignalKind::user_defined1()) {
        Ok(s) => s,
        Err(e) => panic!("signal task failed: {:?}", e),
    };
    while signals.recv().await.is_some() {
        if events.send(Event::Signal).is_err() {
            break;
        }
    }
}

pub async fn layer(events: Sender<Event>) {
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
        if events.send(Event::NewLayer).is_err() {
            break;
        }
    }
}

pub async fn graceful_shutdown() {
    async fn wait_for_signal(s: SignalKind) {
        match signal(s) {
            Ok(mut s) => {
                let Some(_) = s.recv().await else {
                    // None is an error so lets pretend we didn't see anything
                    return pending().await;
                };
            }
            Err(_) => pending().await,
        }
    }

    select! {
        _ = wait_for_signal(SignalKind::interrupt())  => {}
        _ = wait_for_signal(SignalKind::terminate())  => {}
        _ = wait_for_signal(SignalKind::quit())  => {}
    }
}
