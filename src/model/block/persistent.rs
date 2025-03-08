use super::{Event, TaskData};
use crate::{
    event_loop::current_layer,
    global_config,
    model::{ActivationLayer, AffectedMonitor},
    parsing::parser::Title,
    util::{
        cmd::{self, child_debug_loop},
        trim_new_lines,
    },
};
use futures::{
    FutureExt,
    future::BoxFuture,
    stream::{self, Stream, StreamExt},
};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::broadcast,
    time::timeout,
};
use tokio_stream::wrappers::LinesStream;

#[derive(Debug, Clone, Copy)]
pub struct Persistent;

impl super::BlockTask for Persistent {
    fn start(&self, events: broadcast::Receiver<Event>, data: TaskData) -> BoxFuture<'static, ()> {
        start(events, data).boxed()
    }
}

async fn start(events: broadcast::Receiver<Event>, data: TaskData) {
    let TaskData {
        block_name,
        cmd,
        updates,
        actions,
        bid,
        monitors,
        ..
    } = data;
    stream::iter(monitors.iter())
        .for_each_concurrent(monitors.len().get(), |mon| {
            let updates = updates.clone();
            let mut events = events.resubscribe();
            async move {
                let mut output = match ChildStream::start(block_name, cmd, mon, current_layer()).await {
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
                        e = events.recv() => {
                            let Ok(e) = e else {
                                break;
                            };
                            match e {
                                Event::MouseClicked(id, mon, button) if id == bid => {
                                    if let Some(a) = actions[button] {
                                        let _ = cmd::run_cmd(block_name.title, a, mon.into(), current_layer()).await;
                                    }
                                }
                                Event::MouseClicked(..)
                                | Event::Signal
                                | Event::NewLayer => (),
                            }
                        }
                    }
                }
                output.reap().await;
            }
        }).await;
}

#[pin_project::pin_project]
struct ChildStream {
    block_name: Title<'static>,
    child: Child,
    #[pin]
    stream: LinesStream<BufReader<tokio::process::ChildStdout>>,
}

impl ChildStream {
    async fn start(
        block_name: Title<'static>,
        cmd: &str,
        monitor: AffectedMonitor,
        layer: u16,
    ) -> io::Result<Self> {
        let mut spawned = Command::new("bash")
            .args(["-c", cmd])
            .envs(global_config::get().as_env_vars(monitor, u16::MAX))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        child_debug_loop(
            spawned.stderr.take().unwrap(),
            block_name.title,
            monitor,
            ActivationLayer::L(layer),
        );
        Ok(ChildStream {
            block_name,
            stream: LinesStream::new(
                BufReader::new(spawned.stdout.take().ok_or(io::ErrorKind::UnexpectedEof)?).lines(),
            ),
            child: spawned,
        })
    }

    async fn reap(mut self) {
        log::debug!("reaping {}", self.block_name);
        match timeout(Duration::from_secs(5), async {
            self.child.kill().await?;
            self.child.wait().await
        })
        .await
        {
            Ok(Ok(_)) => log::debug!("reaped: {}", self.block_name),
            Ok(Err(e)) => log::error!("failed to wait on child for {}: {e}", self.block_name),
            Err(_elapsed) => log::error!("timedout waiting on child for {}", self.block_name),
        }
    }
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
