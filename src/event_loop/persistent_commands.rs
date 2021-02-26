use super::Event;
use crate::{block::Content, global_config, Config};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{mpsc, Arc, RwLock},
    thread::{self, JoinHandle},
};

pub fn persistent_block_threads(config: &Config, sx: &mpsc::Sender<Event>) -> Vec<JoinHandle<()>> {
    config
        .values()
        .flat_map(|blocks| blocks.iter())
        .filter_map(|b| {
            if let Content::Persistent { cmd, last_run } = &b.content {
                Some((cmd, last_run))
            } else {
                None
            }
        })
        .flat_map(|(cmd, last_run)| last_run.iter().enumerate().map(move |(m, r)| (m, cmd, r)))
        .map(|(m, cmd, r)| {
            let cmd = cmd.to_string();
            let ch = sx.clone();
            let r = Arc::clone(&r);
            thread::spawn(move || persistent_command(cmd, ch, r, m))
        })
        .collect::<Vec<_>>()
}

fn persistent_command<'a>(
    cmd: String,
    ch: mpsc::Sender<Event>,
    last_run: Arc<RwLock<String>>,
    monitor: usize,
) {
    let mut persistent_cmd = Command::new("sh")
        .args(&["-c", &cmd])
        .stdout(Stdio::piped())
        .env("MONITOR", &monitor.to_string())
        .envs(global_config::get().as_env_vars())
        .spawn()
        .expect("Couldn't start persistent cmd");
    let _ = BufReader::new(
        persistent_cmd
            .stdout
            .take()
            .expect("Couldn't get persistent cmd stdout"),
    )
    .lines()
    .map(Result::unwrap)
    .try_for_each(|l| {
        *last_run.write().unwrap() = l;
        ch.send(Event::Update)
    });
    let _ = persistent_cmd.kill();
    let _ = persistent_cmd.wait();
}
