use super::{current_layer, Event, MouseButton};
use crate::{
    model::{block::BlockId, Alignment},
    util::cmd,
};
use std::{
    fmt::{self, Display},
    str::FromStr,
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::ChildStdout,
    sync::{broadcast::Sender, Mutex},
    task,
};

pub fn run(outputs: Vec<ChildStdout>, events: Arc<Mutex<Sender<Event>>>) {
    outputs.into_iter().for_each(|out| {
        task::spawn({
            let events = events.clone();
            let mut out = BufReader::new(out);
            let mut buf = String::new();
            async move {
                loop {
                    buf.clear();
                    let action = match out.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => match buf.parse::<Action>() {
                            Ok(a) => a,
                            Err(e) => {
                                if cfg!(debug_assertions) {
                                    log::error!("Failed to parse buf '{}' because: {}", buf, e);
                                }
                                let _ = cmd::run_cmd(&buf, 0, current_layer()).await;
                                continue;
                            }
                        },
                        Err(e) => {
                            log::error!("Error reading from lemonbar: {:?}", e);
                            continue;
                        }
                    };
                    if events.lock().await.send(action.into()).is_err() {
                        break;
                    }
                }
            }
        });
    });
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
