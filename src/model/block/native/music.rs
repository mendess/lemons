use super::super::{BlockId, BlockTask, TaskData};
use crate::{
    event_loop::{current_layer, Event, MouseButton},
    model::{
        block::{BlockText, TextDecorations},
        AffectedMonitor, Color,
    },
};
use futures::{future::BoxFuture, stream::StreamExt, FutureExt};
use mlib::players::{
    self,
    event::{OwnedLibMpvEvent, PlayerEvent},
};
use std::{pin::pin, sync::Arc};
use tokio::{
    select,
    sync::{broadcast, watch},
};

#[derive(Debug, Clone, Copy)]
pub struct Music;

#[async_trait::async_trait]
impl BlockTask for Music {
    fn start(&self, events: broadcast::Receiver<Event>, td: TaskData) -> BoxFuture<'static, ()> {
        start(events, td).boxed()
    }
}

async fn start(events: broadcast::Receiver<Event>, TaskData { updates, bid, .. }: TaskData) {
    players::wait_for_music_daemon_to_start().await;
    let (bar_data, _) = watch::channel(BarData::fetch().await.unwrap());
    let mut receiver = bar_data.subscribe();
    bar_data.send_modify(|_| {});
    let mut bar_data = Arc::new(bar_data);
    let user_event_loop = pin!(user_event_loop(events, bid, bar_data.clone()));
    let player_event_loop = pin!(async move {
        loop {
            bar_data = player_event_loop(bar_data).await;
            bar_data.send_if_modified(|data| data.take().is_some());
            players::wait_for_music_daemon_to_start().await;
        }
    });
    let bar_event_loop = pin!(async {
        while receiver.changed().await.is_ok() {
            let data = receiver
                .borrow_and_update()
                .as_ref()
                .map(BarData::to_decorated_text)
                .unwrap_or_default();
            if updates
                .send((data, bid, AffectedMonitor::All))
                .await
                .is_err()
            {
                log::warn!("native music block shutting down");
                break;
            }
        }
    });

    select! {
        _ = user_event_loop => {}
        _ = player_event_loop => {}
        _ = bar_event_loop => {}
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

async fn player_event_loop(bar_data: BarDataWatcher) -> BarDataWatcher {
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
                "playlist-pos" => {
                    bar_data.send_if_modified(|data| {
                        let Some(data) = data.as_mut() else {
                            return false;
                        };
                        let get_state = |data: &BarData| data.title.chapter.is_some();
                        let before = get_state(data);
                        data.title.chapter.take();
                        let after = get_state(data);
                        before != after
                    });
                }
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
                "chapter-metadata" => {
                    let Ok(mut map) = change.into_map() else {
                        continue;
                    };

                    let Some(title) = map.remove("title") else {
                        continue;
                    };

                    let Ok(title) = title.into_string() else {
                        continue;
                    };
                    bar_data.send_if_modified(|data| update_chapter(data, title, player_index));
                }
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
                    let has_title =
                        data.title.media_title.is_some() || data.title.chapter.is_some();
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
    log::error!("player event loop exiting");
    bar_data
}

fn update_title(data: &mut Option<BarData>, title: String, player_index: usize) -> bool {
    match data {
        Some(t) => {
            let old = std::mem::replace(&mut t.title.media_title, Some(title));
            old != t.title.media_title
        }
        None => {
            *data = Some(BarData {
                player_index,
                title: Title::simple(title),
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
                title: Title::default(),
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
                title: Title::default(),
                paused: Some(paused),
                volume: None,
            });
            true
        }
    }
}

fn update_chapter(data: &mut Option<BarData>, chapter_title: String, player_index: usize) -> bool {
    match data {
        Some(d) => {
            let old = std::mem::replace(&mut d.title.chapter, Some(chapter_title));
            old != d.title.chapter
        }
        None => {
            *data = Some(BarData {
                player_index,
                title: Title::with_chapter(chapter_title),
                paused: None,
                volume: None,
            });
            true
        }
    }
}

#[derive(Debug, Default)]
struct Title {
    media_title: Option<String>,
    chapter: Option<String>,
}

impl Title {
    async fn fetch() -> Result<Title, players::Error> {
        let media_title = players::media_title().await?;
        if let Ok(Some(chmeta)) = players::chapter_metadata().await {
            Ok(Title {
                media_title: Some(media_title),
                chapter: Some(chmeta.title),
            })
        } else {
            Ok(Title {
                media_title: Some(media_title),
                chapter: None,
            })
        }
    }

    fn simple(title: String) -> Self {
        Self {
            media_title: Some(title),
            chapter: None,
        }
    }

    fn with_chapter(chapter: String) -> Self {
        Self {
            media_title: None,
            chapter: Some(chapter),
        }
    }

    fn to_decorated_text(&self, blocks: &mut Vec<BlockText>) {
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
        let Some(media_title) = &self.media_title else {
            return;
        };
        match &self.chapter {
            Some(chapter) => {
                let g = crate::global_config::get();
                let (v, el) = trunc(media_title);
                let (c, el1) = trunc(chapter);
                let blue = TextDecorations {
                    fg: Some(*g.get_color("blue").unwrap_or(&Color::BLUE)),
                    ..Default::default()
                };
                blocks.extend([
                    BlockText {
                        decorations: blue,
                        text: "Video:".into(),
                    },
                    BlockText {
                        decorations: Default::default(),
                        text: format!(" {v}{el} "),
                    },
                    BlockText {
                        decorations: blue,
                        text: "Song:".into(),
                    },
                    BlockText {
                        decorations: Default::default(),
                        text: format!(" {c}{el1} "),
                    },
                ]);
            }
            None => {
                let (title, elipsis) = trunc(media_title);
                blocks.push(BlockText::from(format!("{title}{elipsis}")));
            }
        }
    }
}

#[derive(Debug)]
struct BarData {
    player_index: usize,
    title: Title,
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
        let title = match Title::fetch().await {
            Ok(t) => t,
            Err(players::Error::Mpv(players::error::MpvError::NoMpvInstance)) => Title::default(),
            Err(e) => return Err(e),
        };
        Ok(Some(Self {
            player_index: index,
            title,
            paused: p!(players::is_paused().await),
            volume: p!(players::volume().await),
        }))
    }

    fn to_decorated_text(&self) -> Vec<BlockText> {
        let mut blocks = Vec::new();
        blocks.push(BlockText {
            decorations: Default::default(),
            text: format!("[{}] ", self.player_index),
        });
        self.title.to_decorated_text(&mut blocks);
        if let Some(paused) = self.paused {
            blocks.push(BlockText {
                decorations: Default::default(),
                text: if paused { " || " } else { " > " }.into(),
            })
        }
        if let Some(volume) = self.volume {
            blocks.push(BlockText {
                decorations: Default::default(),
                text: format!("{volume}%"),
            })
        }
        blocks
    }
}
