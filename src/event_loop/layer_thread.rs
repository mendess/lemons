use signal_hook::{
    consts::signal::{SIGUSR1, SIGUSR2},
    iterator::Signals,
    low_level::raise,
};
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Weak,
    },
    thread::{self, JoinHandle},
};

pub fn layer_thread(layer: Weak<AtomicU16>, n_layers: u16) -> JoinHandle<()> {
    let layer_thread = thread::spawn(move || {
        for _ in &mut Signals::new(&[SIGUSR2]).unwrap() {
            if let Some(layer) = layer.upgrade() {
                let _ = layer.fetch_update(Ordering::SeqCst, Ordering::SeqCst, move |c| {
                    Some((c + 1) % n_layers)
                });
                let _ = raise(SIGUSR1);
            } else {
                break;
            }
        }
    });
    layer_thread
}
