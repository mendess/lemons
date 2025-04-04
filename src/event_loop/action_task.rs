use super::{Event, MouseButton, current_layer};
use crate::{
    model::{Alignment, block::BlockId},
    util::cmd,
};
use futures::{StreamExt, stream};
use std::{
    fmt::{self, Display},
    str::FromStr,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::ChildStdout,
    sync::broadcast::Sender,
};

pub async fn run(outputs: Vec<ChildStdout>, events: Sender<Event>) {
    stream::iter(outputs)
        .for_each_concurrent(None, |out| {
            let events = events.clone();
            async move {
                let mut out = BufReader::new(out);
                let mut buf = String::new();
                loop {
                    buf.clear();
                    let action = match out.read_line(&mut buf).await {
                        Ok(0) => break,
                        // TODO: zelbar currently only suports one action pre block, as such, the
                        // syntax is actually %{A:command} instead of %{AX:command} but the parser
                        // doesn't enforce this, it just assumes there is a `:` after the A and
                        // skips it. As such, if you output an action in the lemonbar format, the
                        // command will start with `:` and then you'll get a error running the
                        // command
                        //
                        // This method means that for lemobar no command can start with `:` but
                        // that's okay since I've never seen such a command (besides `true`).
                        Ok(_) => {
                            let buf = buf.trim();
                            if buf == ":" {
                                let _ = cmd::run_cmd(
                                    "action-task",
                                    buf,
                                    crate::model::AffectedMonitor::All,
                                    current_layer(),
                                )
                                .await;
                                continue;
                            } else {
                                log::trace!("lembar output: '{buf}'");
                                match buf.trim_start_matches(':').parse::<Action>() {
                                    Ok(a) => a,
                                    Err(e) => {
                                        if cfg!(debug_assertions) {
                                            log::error!(
                                                "Failed to parse buf '{}' because: {}",
                                                buf,
                                                e
                                            );
                                        }
                                        let _ = cmd::run_cmd(
                                            "action-task",
                                            buf,
                                            crate::model::AffectedMonitor::All,
                                            current_layer(),
                                        )
                                        .await;
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading from lemonbar: {:?}", e);
                            continue;
                        }
                    };
                    if events.send(action.into()).is_err() {
                        break;
                    }
                }
            }
        })
        .await;
}

pub struct Action {
    pub id: BlockId,
    pub monitor: u8,
    pub button: MouseButton,
}

impl Action {
    pub fn new(alignment: Alignment, index: usize, monitor: u8, button: MouseButton) -> Self {
        Self {
            id: (alignment, index),
            monitor,
            button,
        }
    }
}

impl From<Action> for Event {
    fn from(a: Action) -> Self {
        Event::MouseClicked(a.id, a.monitor, a.button)
    }
}

impl FromStr for Action {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = &mut s.trim_end_matches('\n').split('-');
        Ok(Self {
            id: (
                s.next()
                    .and_then(|s| s.parse::<u8>().ok())
                    .ok_or("Missing al")?
                    .into(),
                s.next()
                    .and_then(|s| s.parse().ok())
                    .ok_or("Missing index")?,
            ),
            monitor: s
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or("Missing monitor")?,
            button: s
                .next()
                .and_then(|s| s.parse::<u8>().ok())
                .ok_or("Missing button")?
                .into(),
        })
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{}-{}-{}",
            self.id.0 as u8, self.id.1, self.monitor, self.button
        )
    }
}
