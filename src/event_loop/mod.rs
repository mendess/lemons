pub mod action_task;
pub mod signal_task;
pub mod update_channel;

use crate::{
    display::{display_block, Bar},
    global_config,
    model::{
        block::{self, Block, BlockId, BlockText, BlockUpdate},
        ActivationLayer, AffectedMonitor, Alignment, Config,
    },
    util::{cmd::child_debug_loop, one_or_more::OneOrMore},
};
use enum_iterator::IntoEnumIterator;
use std::{
    ffi::OsStr,
    ops::{Index, IndexMut},
    process::Stdio,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc, Once,
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

fn spawn_bar<A, S, W, B>(args: A, monitor: u8) -> (Child, ChildStdin, ChildStdout)
where
    A: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    B: Bar<W>,
    W: std::fmt::Write,
{
    let mut lemonbar = if std::env::var("USE_CAT").is_ok() {
        let mut cat = Command::new("bash");
        cat.args(["-c", "cat >&2"]);
        cat
    } else {
        let mut lemonbar = Command::new(B::PROGRAM);
        if log::log_enabled!(log::Level::Debug) {
            let args = args.into_iter().collect::<Vec<_>>();
            let args = args.iter().map(|s| s.as_ref()).collect::<Vec<_>>();
            log::debug!("spawning {} with args {:?}", B::PROGRAM, args);
            lemonbar.args(args);
        } else {
            lemonbar.args(args);
        }
        lemonbar
    };

    lemonbar
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut lemonbar = lemonbar.spawn().expect("Couldn't start lemonbar");

    child_debug_loop(
        lemonbar.stderr.take().unwrap(),
        B::PROGRAM,
        monitor.into(),
        ActivationLayer::All,
    );
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

struct RunningBlock {
    block: Block<'static>,
    last_run: OneOrMore<Vec<BlockText>>,
}

struct RunningConfig([Vec<RunningBlock>; 3]);

impl From<Config<'static>> for RunningConfig {
    fn from(value: Config<'static>) -> Self {
        Self(value.0.map(|monitor_blocks| {
            monitor_blocks
                .into_iter()
                .map(|block| {
                    let mut last_run = OneOrMore::default();
                    block.active_in.resize_one_or_more(&mut last_run);
                    RunningBlock { block, last_run }
                })
                .collect()
        }))
    }
}

impl Index<Alignment> for RunningConfig {
    type Output = Vec<RunningBlock>;

    fn index(&self, a: Alignment) -> &Self::Output {
        &self.0[a as usize]
    }
}

impl IndexMut<Alignment> for RunningConfig {
    fn index_mut(&mut self, a: Alignment) -> &mut Self::Output {
        &mut self.0[a as usize]
    }
}

impl RunningConfig {
    pub fn update(&mut self, update: block::BlockUpdate) -> bool {
        let (alignment, index, monitor) = update.id();
        // if we have to update something that affects all monitors than we assume that `last_run`
        // in the `OneOrMore::One` state.
        let block = &mut self[alignment][index].last_run[match monitor {
            AffectedMonitor::Single(n) => n,
            AffectedMonitor::All => u8::MAX,
        }];
        let new_block = update.into_inner_text();
        if *block != new_block {
            log::debug!("bar update '{new_block:?}' from {:?}", (alignment, index));
            block.clone_from(&new_block);
            true
        } else {
            false
        }
    }
}

pub async fn start_event_loop<B>(
    config: Config<'static>,
    events: broadcast::Sender<Event>,
    mut updates: mpsc::Receiver<BlockUpdate>,
) where
    B: Bar<String>,
{
    let global_config = crate::global_config::get();
    let (mut bars, mut lemon_inputs, lemon_outputs) = if global_config.outputs.is_empty() {
        let (bar, lemon_inputs, lemon_outputs) =
            spawn_bar::<_, _, _, B>(&global_config.to_arg_list::<_, B>(None), 0);
        (vec![bar], vec![lemon_inputs], vec![lemon_outputs])
    } else {
        unzip_n::unzip_n!(3);
        global_config
            .outputs
            .iter()
            .map(|g| global_config.to_arg_list::<_, B>(Some(g)))
            .enumerate()
            .map(|(i, args)| spawn_bar::<_, _, _, B>(args, i as u8))
            .unzip_n()
    };
    let events = Arc::new(Mutex::new(events));
    action_task::run(lemon_outputs, events.clone());
    signal_task::refresh(events.clone());
    signal_task::layer(events.clone());

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
        if bars.iter_mut().all(|c| matches!(c.try_wait(), Ok(Some(_)))) {
            break;
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
