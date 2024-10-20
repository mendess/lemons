use super::{
    super::{ActivationLayer, ActiveMonitors},
    BlockId, Event, Signal, TaskData,
};
use crate::{
    event_loop::{current_layer, update_channel::UpdateChannel},
    util::{cmd::run_cmd, result_ext::ResultExt, signal::sig_rt_min, trim_new_lines},
};
use std::time::Duration;
use tokio::{
    signal::unix::{signal as signal_stream, SignalKind},
    sync::broadcast,
    time,
};

#[derive(Debug, Clone, Copy)]
pub struct Timed(pub Duration);

impl super::BlockTask for Timed {
    fn start(&self, events: &broadcast::Sender<Event>, data: TaskData) {
        let timeout = self.0;
        let mut events = events.subscribe();
        let TaskData {
            block_name,
            cmd,
            updates,
            actions,
            bid,
            activation_layer,
            monitors,
            signal,
            ..
        } = data;
        if let Signal::Num(n) = signal {
            tokio::spawn({
                let updates = updates.clone();
                async move {
                    let mut signals = match signal_stream(SignalKind::from_raw(sig_rt_min() + n)) {
                        Ok(s) => s,
                        Err(e) => {
                            return log::error!(
                                "Failed to start signal task for '{}' because: {:?}",
                                cmd,
                                e
                            );
                        }
                    };
                    while signals.recv().await.is_some() {
                        if update_blocks(block_name, cmd, activation_layer, bid, monitors, &updates)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            });
        }
        tokio::spawn(async move {
            if update_blocks(block_name, cmd, activation_layer, bid, monitors, &updates)
                .await
                .is_err()
            {
                return;
            }
            loop {
                let event = if activation_layer == current_layer() {
                    time::timeout(timeout, events.recv()).await.ok()
                } else {
                    Some(events.recv().await)
                };
                if let Some(event) = event {
                    match event {
                        Ok(Event::MouseClicked(id, mon, button)) if id == bid => {
                            if let Some(a) = actions[button] {
                                let _ = run_cmd(block_name, a, mon.into(), current_layer()).await;
                            }
                            continue;
                        }
                        Ok(Event::Signal) if signal.is_some() => {}
                        Ok(Event::NewLayer) => {}
                        Ok(Event::MouseClicked(..)) | Ok(Event::Signal) => continue,
                        Err(_) => return,
                    }
                }
                if update_blocks(block_name, cmd, activation_layer, bid, monitors, &updates)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }
}

async fn update_blocks(
    block_name: &'static str,
    cmd: &'static str,
    activation_layer: ActivationLayer,
    bid: BlockId,
    monitors: ActiveMonitors,
    updates: &UpdateChannel,
) -> Result<(), ()> {
    let layer = current_layer();
    if activation_layer == layer {
        for m in monitors.iter() {
            let mut output = run_cmd(block_name, cmd, m, layer)
                .await
                .map_err(|e| e.to_string())
                .merge();
            trim_new_lines(&mut output);
            updates.send((output, bid, m)).await.map_err(|_| ())?
        }
    }
    Ok(())
}
