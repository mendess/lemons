use super::{Event, TaskData};
use crate::{event_loop::current_layer, global_config, util::cmd};
use futures::stream::{Stream, StreamExt};
use std::process::Stdio;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
};
use tokio_stream::wrappers::LinesStream;

#[derive(Debug, Clone, Copy)]
pub struct Persistent;

impl super::BlockTask for Persistent {
    fn start(&self, events: &broadcast::Sender<Event>, data: TaskData) {
        let TaskData {
            cmd,
            updates,
            actions,
            bid,
            monitors,
            ..
        } = data;
        for mon in monitors.iter() {
            tokio::task::spawn({
                let updates = updates.clone();
                let mut events = events.subscribe();
                async move {
                    let mut output = match run_cmd(cmd, mon).await {
                        Ok(o) => o,
                        Err(e) => {
                            return log::error!(
                                "Failed to start persistent command: '{}', because '{:?}'",
                                cmd,
                                e
                            )
                        }
                    };
                    loop {
                        tokio::select! {
                            Some(l) = output.next() => {
                                if let Ok(l) = l {
                                    if updates.send((l, bid, mon).into()).await.is_err() {
                                        break
                                    }
                                }
                            }
                            Ok(e) = events.recv() => {
                                match e {
                                    Event::MouseClicked(id, mon, button) if id == bid => {
                                        if let Some(a) = actions[button] {
                                            let _ = cmd::run_cmd(a, mon, current_layer()).await;
                                        }
                                    }
                                    Event::MouseClicked(..)
                                    | Event::Signal
                                    | Event::NewLayer
                                    | Event::Refresh => (),
                                }
                            }
                            else => break
                        }
                    }
                }
            });
        }
    }
}

async fn run_cmd(cmd: &str, monitor: u8) -> io::Result<impl Stream<Item = io::Result<String>>> {
    let spawned = Command::new("bash")
        .args(&["-c", cmd])
        .envs(global_config::get().as_env_vars(monitor, u16::MAX))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(LinesStream::new(
        BufReader::new(spawned.stdout.ok_or(io::ErrorKind::UnexpectedEof)?).lines(),
    ))
}
