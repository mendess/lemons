use super::{Event, TaskData};
use crate::{
    event_loop::current_layer,
    global_config,
    model::{ActivationLayer, AffectedMonitor},
    util::{
        cmd::{self, child_debug_loop},
        trim_new_lines,
    },
};
use futures::stream::{Stream, StreamExt};
use std::process::Stdio;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::broadcast,
};
use tokio_stream::wrappers::LinesStream;

#[derive(Debug, Clone, Copy)]
pub struct Persistent;

impl super::BlockTask for Persistent {
    fn start(&self, events: &broadcast::Sender<Event>, data: TaskData) {
        let TaskData {
            block_name,
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
                    let mut output = match run_cmd(block_name, cmd, mon, current_layer()).await {
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
                                if let Ok(mut l) = l {
                                    trim_new_lines(&mut l);
                                    if updates.send((l, bid, mon)).await.is_err() {
                                        break
                                    }
                                }
                            }
                            Ok(e) = events.recv() => {
                                match e {
                                    Event::MouseClicked(id, mon, button) if id == bid => {
                                        if let Some(a) = actions[button] {
                                            let _ = cmd::run_cmd(block_name, a, mon.into(), current_layer()).await;
                                        }
                                    }
                                    Event::MouseClicked(..)
                                    | Event::Signal
                                    | Event::NewLayer => (),
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

#[pin_project::pin_project]
struct ChildStream {
    child: Child,
    #[pin]
    stream: LinesStream<BufReader<tokio::process::ChildStdout>>,
}

impl Stream for ChildStream {
    type Item = io::Result<String>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

async fn run_cmd(
    block_name: &'static str,
    cmd: &str,
    monitor: AffectedMonitor,
    layer: u16,
) -> io::Result<impl Stream<Item = io::Result<String>>> {
    let mut spawned = Command::new("bash")
        .args(["-c", cmd])
        .envs(global_config::get().as_env_vars(monitor, u16::MAX))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    child_debug_loop(
        spawned.stderr.take().unwrap(),
        block_name,
        monitor,
        ActivationLayer::L(layer),
    );
    Ok(ChildStream {
        stream: LinesStream::new(
            BufReader::new(spawned.stdout.take().ok_or(io::ErrorKind::UnexpectedEof)?).lines(),
        ),
        child: spawned,
    })
}
