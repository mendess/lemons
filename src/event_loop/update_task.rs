use super::{RunningConfig, current_layer};
use crate::{
    Config,
    display::{Bar, display_block},
    global_config,
    model::{AffectedMonitor, Alignment, block::BlockUpdate},
};
use enum_iterator::IntoEnumIterator;
use std::sync::Once;
use tokio::{
    io::AsyncWriteExt as _,
    process::ChildStdin,
    sync::mpsc::{self, Sender, error::SendError},
};

#[derive(Clone)]
pub struct UpdateChannel(Sender<BlockUpdate>);

impl From<Sender<BlockUpdate>> for UpdateChannel {
    fn from(ch: Sender<BlockUpdate>) -> Self {
        Self(ch)
    }
}

impl From<&Sender<BlockUpdate>> for UpdateChannel {
    fn from(ch: &Sender<BlockUpdate>) -> Self {
        Self(ch.clone())
    }
}

impl UpdateChannel {
    pub async fn send(&self, u: impl Into<BlockUpdate>) -> Result<(), SendError<BlockUpdate>> {
        self.0.send(u.into()).await
    }
}

pub async fn update<B>(
    config: Config<'static>,
    mut updates: mpsc::Receiver<BlockUpdate>,
    lemon_inputs: &mut [ChildStdin],
) where
    B: Bar<String>,
{
    let mut config = RunningConfig::from(config);
    let mut line = String::new();
    while let Some(update) = updates.recv().await {
        let (_, _, monitor) = update.id();
        if !config.update(update) {
            // TODO: we could save an update, but zelbar is bugged and so redundant updates
            // actually fix it
            // continue;
        }
        match monitor {
            AffectedMonitor::Single(m) => match lemon_inputs.get_mut(usize::from(m)) {
                Some(input) => {
                    line = build_line::<B>(&config, m, line);
                    log::trace!("{m} => {line}");
                    if let Err(e) = input.write_all(line.as_bytes()).await {
                        log::error!("Couldn't talk to lemon bar :( {:?}", e);
                    }
                }
                None => log::error!("monitor: {m} is out of bounds"),
            },
            AffectedMonitor::All => {
                for (monitor, input) in lemon_inputs.iter_mut().enumerate() {
                    line = build_line::<B>(&config, monitor as _, line);
                    log::trace!("{monitor} => {line}");
                    if let Err(e) = input.write_all(line.as_bytes()).await {
                        log::error!("Couldn't talk to lemon bar :( {:?}", e);
                    }
                }
            }
        }
    }
}

fn build_line<B>(config: &RunningConfig, monitor: u8, mut line: String) -> String
where
    B: Bar<String>,
{
    line.clear();
    let global_config = global_config::get();
    let current_layer = current_layer();
    let mut bar = B::new(line, global_config.separator);
    Alignment::into_enum_iter()
        .map(|a| (a, &config[a]))
        .filter(|(_, c)| !c.is_empty())
        .for_each(|(al, blocks)| {
            let set_alignment = Once::new();
            blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.last_run[monitor].is_empty())
                .filter(|(_, b)| b.block.layer == current_layer)
                .for_each(|(index, b)| {
                    set_alignment.call_once(|| bar.set_alignment(al).unwrap());
                    display_block(&mut bar, &b.block, &b.last_run[monitor], index, monitor).unwrap()
                });
        });
    // TODO: line.lemon('O', tray_offset).unwrap();
    let mut line = bar.into_inner();
    line.push('\n');
    line
}
