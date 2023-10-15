pub mod action_task;
pub mod signal_task;
pub mod update_channel;

use crate::{
    display::{display_block, Bar},
    global_config,
    model::{
        block::{BlockId, BlockUpdate},
        Alignment, Config,
    },
};
use enum_iterator::IntoEnumIterator;
use std::{
    ffi::OsStr,
    process::Stdio,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{broadcast, mpsc, Mutex},
};

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left = 1,
    Middle = 2,
    Right = 3,
    ScrollUp = 4,
    ScrollDown = 5,
}

impl From<u8> for MouseButton {
    fn from(x: u8) -> Self {
        use MouseButton::*;
        match x {
            1 => Left,
            2 => Middle,
            3 => Right,
            4 => ScrollUp,
            5 => ScrollDown,
            _ => unreachable!("Invalid mouse button number (must be inside 1..=5) {}", x),
        }
    }
}

impl std::fmt::Display for MouseButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// The update signal was received
    Signal,
    /// The current layer changed
    NewLayer,
    /// Mouse button clicked
    /// .0: the id of the clicked block
    /// .1: the monitor where the block was clicked
    /// .2: the mouse button used
    MouseClicked(BlockId, u8, MouseButton),
}

fn spawn_bar<A, S>(args: A) -> (Child, ChildStdin, ChildStdout)
where
    A: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut lemonbar = Command::new("lemonbar")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Couldn't start lemonbar");
    let (le_in, le_out) = (
        lemonbar.stdin.take().expect("Failed to find lemon stdin"),
        lemonbar.stdout.take().expect("Failed to find lemon stdout"),
    );
    (lemonbar, le_in, le_out)
}

static CURRENT_LAYER: AtomicU16 = AtomicU16::new(0);

pub fn current_layer() -> u16 {
    CURRENT_LAYER.load(Ordering::Acquire)
}

pub async fn start_event_loop<B>(
    mut config: Config<'static>,
    events: broadcast::Sender<Event>,
    mut updates: mpsc::Receiver<BlockUpdate>,
) where
    B: Bar<String>,
{
    let global_config = crate::global_config::get();
    let (mut bars, mut lemon_inputs, lemon_outputs) = if global_config.bars_geometries.is_empty() {
        let (bar, lemon_inputs, lemon_outputs) = spawn_bar(&global_config.to_arg_list(None));
        (vec![bar], vec![lemon_inputs], vec![lemon_outputs])
    } else {
        unzip_n::unzip_n!(3);
        global_config
            .bars_geometries
            .iter()
            .map(|g| global_config.to_arg_list(Some(g)))
            .map(spawn_bar)
            .unzip_n()
    };
    let events = Arc::new(Mutex::new(events));
    action_task::run(lemon_outputs, events.clone());
    signal_task::refresh(events.clone());
    signal_task::layer(events.clone());

    let mut line = String::new();
    while let Some(update) = updates.recv().await {
        let (al, index, monitor) = update.id();
        let up = update.as_str().to_owned();
        config.update(update);
        match lemon_inputs.get_mut(monitor as usize) {
            Some(input) => {
                line = build_line::<B>(&config, monitor, line);
                debug(&line, &up, (al, index));
                if let Err(e) = input.write_all(line.as_bytes()).await {
                    log::error!("Couldn't talk to lemon bar :( {:?}", e);
                }
            }
            None => {
                for (monitor, input) in lemon_inputs.iter_mut().enumerate() {
                    line = build_line::<B>(&config, monitor as _, line);
                    debug(&line, &up, (al, index));
                    if let Err(e) = input.write_all(line.as_bytes()).await {
                        log::error!("Couldn't talk to lemon bar :( {:?}", e);
                    }
                }
            }
        }
        if bars.iter_mut().all(|c| matches!(c.try_wait(), Ok(Some(_)))) {
            break;
        }
    }
}

fn build_line<B>(config: &Config, monitor: u8, mut line: String) -> String
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
            bar.set_alignment(al).unwrap();
            blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.last_run[monitor].is_empty())
                .filter(|(_, b)| b.layer == current_layer)
                .for_each(|(index, b)| display_block(&mut bar, b, index, monitor).unwrap());
        });
    // TODO: line.lemon('O', tray_offset).unwrap();
    let mut line = bar.into_inner();
    line.push('\n');
    line
}

#[inline(always)]
pub fn debug(line: &str, up: &str, bid: (Alignment, usize)) {
    log::trace!("[{:?}] {}", chrono::Utc::now(), line);
    log::debug!(
        "[{:?}] bar update '{}' from {:?}",
        chrono::Utc::now(),
        up,
        bid
    );
}
