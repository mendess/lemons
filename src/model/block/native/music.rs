use super::super::{BlockId, BlockTask, TaskData};
use crate::event_loop::{current_layer, Event, MouseButton};
use futures::stream::StreamExt;
use mlib::players::{
    self,
    event::{OwnedLibMpvEvent, PlayerEvent},
};
use std::{
    fmt::{self, Display},
    sync::Arc,
};
use tokio::sync::{broadcast, watch};

#[derive(Debug)]
pub struct Music;

impl BlockTask for Music {
    fn start(&self, events: &broadcast::Sender<Event>, TaskData { updates, bid, .. }: TaskData) {
        let events = events.subscribe();
        tokio::spawn(async move {
            players::wait_for_music_daemon_to_start().await;
            let (bar_data, _) = watch::channel(BarData::fetch().await.unwrap());
            let mut receiver = bar_data.subscribe();
            bar_data.send_modify(|_| {});
            let bar_data = Arc::new(bar_data);
            tokio::spawn(user_event_loop(events, bid, bar_data.clone()));
            tokio::spawn(player_event_loop(bar_data));
            while receiver.changed().await.is_ok() {
                let data = receiver
                    .borrow_and_update()
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                if updates.send((data, bid, u8::MAX).into()).await.is_err() {
                    log::warn!("native music block shutting down");
                    break;
                }
            }
        });
    }
}

type BarDataWatcher = Arc<watch::Sender<Option<BarData>>>;

async fn user_event_loop(
    mut ui_events: broadcast::Receiver<Event>,
    bid: BlockId,
    bar_data: BarDataWatcher,
) {
    while let Ok(ev) = ui_events.recv().await {
        match ev {
            Event::MouseClicked(id, _, button) if id == bid => {
                let e = match button {
                    MouseButton::ScrollUp => players::change_volume(2).await,
                    MouseButton::ScrollDown => players::change_volume(-2).await,
                    MouseButton::Middle => players::cycle_pause().await,
                    MouseButton::Left => players::change_file(players::Direction::Prev).await,
                    MouseButton::Right => players::change_file(players::Direction::Next).await,
                };
                if let Err(e) = e {
                    log::error!("error pressing {button:?}: {e:?}");
                }
            }
            Event::MouseClicked(..) | Event::Signal => {}
            Event::NewLayer => bar_data.send_modify(|_| {}),
        }
    }
}

async fn reset_data(bar_data: &BarDataWatcher) {
    let _ = match BarData::fetch().await {
        Ok(data) => bar_data.send(data),
        Err(e) => {
            log::error!("failed to fetch data for new player: {e:?}");
            bar_data.send(None)
        }
    };
}

async fn player_event_loop(bar_data: BarDataWatcher) {
    let event_stream = players::subscribe().await.unwrap();
    reset_data(&bar_data).await;
    tokio::pin!(event_stream);
    while let Some(ev) = event_stream.next().await {
        let ev = match ev {
            Ok(ev) => ev,
            Err(e) => {
                log::error!("error receiving event: {e:?}");
                continue;
            }
        };
        log::debug!("got event: {ev:?}");
        let PlayerEvent {
            player_index,
            event,
        } = ev;
        if bar_data.borrow().as_ref().map(|d| d.player_index) != Some(player_index) {
            reset_data(&bar_data).await
        }
        match event {
            OwnedLibMpvEvent::PropertyChange { name, change, .. } => match name.as_str() {
                "media-title" => {
                    let Ok(title) = change.into_string() else {
                         continue;
                    };
                    bar_data.send_if_modified(|data| update_title(data, title, player_index));
                }
                "volume" => {
                    let Ok(volume) = change.into_double() else {
                        continue;
                    };
                    bar_data.send_if_modified(|data| update_volume(data, volume, player_index));
                }
                "pause" => {
                    let Ok(paused) = change.into_bool() else {
                        continue;
                    };
                    bar_data.send_if_modified(|data| update_paused(data, paused, player_index));
                }
                "chapter-metadata" => {}
                _ => {
                    log::debug!("ignoring property change with name '{name}'");
                }
            },
            OwnedLibMpvEvent::FileLoaded => 'fill: {
                let (should_update_volume, should_update_paused) = {
                    let data = bar_data.borrow();
                    let Some(data) = &*data else {
                        break 'fill;
                    };
                    let has_title = data.title.is_some();
                    (
                        has_title && data.volume.is_none(),
                        has_title && data.paused.is_none(),
                    )
                };
                if should_update_volume {
                    if let Ok(volume) = players::volume().await {
                        bar_data.send_if_modified(|data| update_volume(data, volume, player_index));
                    }
                }
                if should_update_paused {
                    if let Ok(paused) = players::is_paused().await {
                        bar_data.send_if_modified(|data| update_paused(data, paused, player_index));
                    }
                }
            }
            OwnedLibMpvEvent::Shutdown => reset_data(&bar_data).await,
            e => {
                log::debug!("ignoring event {e:?}");
            }
        }
    }
}

fn update_title(data: &mut Option<BarData>, title: String, player_index: usize) -> bool {
    match data {
        Some(t) => {
            match &mut t.title {
                Some(t) => t.media_title = title,
                None => t.title = Some(Title::simple(title)),
            };
            true
        }
        None => {
            *data = Some(BarData {
                player_index,
                title: Some(Title::simple(title)),
                volume: None,
                paused: None,
            });
            true
        }
    }
}

fn update_volume(data: &mut Option<BarData>, volume: f64, player_index: usize) -> bool {
    match data {
        Some(d) => {
            let old = std::mem::replace(&mut d.volume, Some(volume));
            old != d.volume
        }
        None => {
            *data = Some(BarData {
                player_index,
                volume: Some(volume),
                title: None,
                paused: None,
            });
            true
        }
    }
}

fn update_paused(data: &mut Option<BarData>, paused: bool, player_index: usize) -> bool {
    match data {
        Some(d) => {
            let old = std::mem::replace(&mut d.paused, Some(paused));
            old != d.paused
        }
        None => {
            *data = Some(BarData {
                player_index,
                title: None,
                paused: Some(paused),
                volume: None,
            });
            true
        }
    }
}

#[derive(Debug)]
struct Title {
    media_title: String,
    chapter: Option<String>,
}

impl Title {
    async fn fetch() -> Result<Title, players::Error> {
        let media_title = players::media_title().await?;
        if let Ok(chmeta) = players::chapter_metadata().await {
            Ok(Title {
                media_title,
                chapter: Some(chmeta.title),
            })
        } else {
            Ok(Title {
                media_title,
                chapter: None,
            })
        }
    }

    fn simple(title: String) -> Self {
        Self {
            media_title: title,
            chapter: None,
        }
    }
}

impl Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const TRUNC_LEN: usize = 22;
        fn trunc(s: &str) -> (&str, &'static str) {
            let layer = current_layer();
            if layer == 0 || s.len() <= TRUNC_LEN {
                (s, "")
            } else {
                let idx = s
                    .char_indices()
                    .take(TRUNC_LEN - 3)
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or_default();
                (&s[..idx], "...")
            }
        }
        match &self.chapter {
            Some(chapter) => {
                let g = crate::global_config::get();
                let (v, el) = trunc(&self.media_title);
                let (c, el1) = trunc(chapter);
                write!(
                    f,
                    "%{{F{blue}}}Video:%{{F-}} {}{} %{{F{blue}}}Song:%{{F-}} {}{}",
                    v,
                    el,
                    c,
                    el1,
                    blue = g.get_color("blue").map(|c| c.0).unwrap_or("#5498F8"),
                )
            }
            None => {
                let (title, elipsis) = trunc(&self.media_title);
                write!(f, "{}{}", title, elipsis)
            }
        }
    }
}

#[derive(Debug)]
struct BarData {
    player_index: usize,
    title: Option<Title>,
    paused: Option<bool>,
    volume: Option<f64>,
}

impl BarData {
    async fn fetch() -> Result<Option<Self>, players::Error> {
        macro_rules! p {
            ($result:expr) => {
                match $result {
                    Ok(t) => Some(t),
                    Err(players::Error::Mpv(players::error::MpvError::NoMpvInstance)) => None,
                    Err(e) => return Err(e),
                }
            };
        }
        let Some(index) = players::current().await? else {
            return Ok(None);
        };
        Ok(Some(Self {
            player_index: index,
            title: p!(Title::fetch().await),
            paused: p!(players::is_paused().await),
            volume: p!(players::volume().await),
        }))
    }
}

impl Display for BarData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.player_index)?;
        if let Some(title) = &self.title {
            write!(f, "{title} ")?;
        }
        if let Some(paused) = self.paused {
            write!(f, "{} ", if paused { "||" } else { ">" })?;
        }
        if let Some(volume) = self.volume {
            write!(f, "{volume}%")?;
        }
        Ok(())
    }
}
