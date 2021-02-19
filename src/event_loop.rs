mod layer_thread;
mod persistent_commands;
mod signal;
mod timed_blocks;
mod trayer;
mod update_signal;

use crate::{
    block::Alignment,
    display::{DisplayBlock, Lemonbar},
    Block, Config, GlobalConfig,
};
use layer_thread::layer_thread;
use persistent_commands::persistent_block_threads;
use std::{
    ffi::OsStr,
    fmt::Write as _,
    io::Write as _,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc, Arc,
    },
};
use timed_blocks::timed_blocks;
use trayer::trayer;
use update_signal::update_signal;

pub enum Event {
    Update,
    TrayResize(u32),
}

fn spawn_bar<A, S>(args: A) -> ([Child; 2], ChildStdin)
where
    A: IntoIterator<Item = S> + std::fmt::Debug,
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
    let shell = Command::new("bash")
        .stdin(Stdio::from(le_out))
        .spawn()
        .expect("Couldn't start action shell");
    ([lemonbar, shell], le_in)
}

pub fn start_event_loop(config: Config<'static>) {
    let global_config = crate::global_config::GLOBAL_CONFIG.load_full();
    let mut lemon_inputs = if global_config.bars_geometries.is_empty() {
        vec![spawn_bar(&global_config.to_arg_list(None))]
    } else {
        global_config
            .bars_geometries
            .iter()
            .map(|g| global_config.to_arg_list(Some(&g)))
            .map(spawn_bar)
            .collect()
    };
    let (sx, event_loop) = mpsc::channel();
    let config = Arc::new(config);
    let current_layer = Arc::new(AtomicU16::default());

    let mut t = persistent_block_threads(&config, &sx);
    t.push(timed_blocks(
        Arc::downgrade(&config),
        Arc::downgrade(&current_layer),
        &sx,
    ));

    if global_config.tray {
        t.push(trayer(&global_config, sx.clone()));
    }
    t.push(update_signal(Arc::downgrade(&config), &sx));
    t.push(layer_thread(
        Arc::downgrade(&current_layer),
        global_config.n_layers,
    ));
    let mut last_offset = 0;
    for e in event_loop {
        if let Event::TrayResize(o) = e {
            last_offset = o;
        }
        let mut tray_offset = last_offset;
        for (i, child) in lemon_inputs.iter_mut().enumerate() {
            let line = build_line(&global_config, &config, &current_layer, tray_offset, i);
            tray_offset = 0;
            if let Err(e) = child.1.write_all(line.as_bytes()) {
                eprintln!("Couldn't talk to lemon bar :( {:?}", e);
            }
        }
        if lemon_inputs
            .iter_mut()
            .flat_map(|chs| chs.0.iter_mut())
            .all(|c| matches!(c.try_wait(), Ok(Some(_))))
        {
            break;
        }
    }
    drop(config);
    drop(current_layer);
    for thread in t {
        thread.thread().unpark();
        thread.join().unwrap();
    }
}

fn build_line(
    global_config: &GlobalConfig,
    config: &Config,
    layer: &AtomicU16,
    tray_offset: u32,
    monitor: usize,
) -> String {
    let mut line = String::new();
    let add_blocks = |blocks: &[Block], l: &mut String| {
        blocks
            .iter()
            .filter(|b| !b.content.is_empty(monitor))
            .filter(|b| b.layer == layer.load(Ordering::SeqCst))
            .map(|b| DisplayBlock(b, monitor))
            .zip(std::iter::successors(Some(Some("")), |_| {
                Some(global_config.separator)
            }))
            .for_each(|(b, s)| {
                s.map(|s| l.push_str(s));
                write!(l, "{}", b).unwrap();
            })
    };
    if let Some(blocks) = config.get(&Alignment::Left) {
        line.push_str("%{l}");
        add_blocks(blocks, &mut line);
    }
    if let Some(blocks) = config.get(&Alignment::Middle) {
        line.push_str("%{c}");
        add_blocks(blocks, &mut line);
    }
    if let Some(blocks) = config.get(&Alignment::Right) {
        line.push_str("%{r}");
        add_blocks(blocks, &mut line);
    }
    line.lemon('O', tray_offset).unwrap();
    line + "\n"
}
