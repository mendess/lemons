use super::super::{BlockId, BlockTask, Signal, TaskData};
use crate::{
    event_loop::{current_layer, update_channel::UpdateChannel, Event, MouseButton},
    util::{result_ext::ResultExt, signal::sig_rt_min},
};
use futures::stream::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize};
use signal_hook_tokio::Signals;
use std::{
    fmt::{self, Display},
    iter::once,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::{
    io::{self, AsyncWriteExt},
    net::UnixStream,
    sync::{broadcast, mpsc, Mutex},
    time,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Update {
    Full,
    Volume,
    Title,
    State,
}

#[derive(Debug)]
pub struct Music;

impl BlockTask for Music {
    fn start(
        &self,
        events: &broadcast::Sender<Event>,
        TaskData {
            bid,
            updates,
            signal,
            ..
        }: TaskData,
    ) {
        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(event_loop(events.subscribe(), bid, updates, rx));
        tokio::spawn(async move {
            let _ = tx.send(()).await;
            if let Signal::Num(n) = signal {
                let signals = match Signals::new(once(sig_rt_min() + n)) {
                    Ok(s) => s,
                    Err(e) => {
                        return log::error!("Failed to start signal task for native music {}", e);
                    }
                };
                let sends = signals.then(|_| tx.send(()));
                tokio::pin!(sends);
                while let Some(Ok(_)) = sends.next().await {}
                log::info!("music native terminating");
            }
        });
    }
}

async fn update_bar(bar: &mut Option<BarData>, how: Update) {
    match bar {
        Some(b) => {
            if match how {
                Update::Full => {
                    *bar = BarData::fetch().await.ok();
                    Ok(())
                }
                Update::Volume => b.update_volume().await,
                Update::Title => b.update_title().await,
                Update::State => b.update_paused().await,
            }
            .is_err()
            {
                *bar = None;
            };
        }
        None => *bar = BarData::fetch().await.ok(),
    }
}

async fn event_loop(
    mut events: broadcast::Receiver<Event>,
    bid: BlockId,
    updates: UpdateChannel,
    mut ch: mpsc::Receiver<()>,
) -> io::Result<()> {
    let mut bar = None;
    while let Ok(e) = tokio::select! {
        e = events.recv() => e,
        Some(_) = ch.recv() => Ok(Event::Signal)
    } {
        let up = match e {
            Event::MouseClicked(id, _, button) if id == bid => {
                let mut sock = socket().await?;
                let (msg, up) = match button {
                    MouseButton::ScrollUp => ("add volume 2\n", Update::Volume),
                    MouseButton::ScrollDown => ("add volume -2\n", Update::Volume),
                    MouseButton::Middle => ("cycle pause\n", Update::State),
                    MouseButton::Left => ("playlist-prev\n", Update::Title),
                    MouseButton::Right => ("playlist-next\n", Update::Title),
                };
                sock.write_all(msg.as_bytes()).await?;
                if up == Update::Title {
                    // if we don't do this we'll have a useless update that will almost always be
                    // wrong since mpv still takes a bit of time to load the song
                    continue;
                }
                up
            }
            Event::MouseClicked(..) => continue,
            Event::Refresh | Event::Signal => Update::Full,
            Event::NewLayer => Update::Title,
        };
        time::sleep(Duration::from_millis(1)).await;
        update_bar(&mut bar, up).await;
        let b = bar.as_ref().map(ToString::to_string).unwrap_or_default();
        if updates.send((b, bid, u8::MAX).into()).await.is_err() {
            break;
        }
    }
    Ok(())
}

async fn socket() -> io::Result<UnixStream> {
    const INVALID_THREASHOLD: Duration = Duration::from_secs(5);

    static SOCKET: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.mpvsocket[0-9]+$").unwrap());
    static CURRENT: Lazy<Mutex<(PathBuf, Instant)>> = Lazy::new(|| {
        Mutex::new((
            PathBuf::new(),
            Instant::now()
                .checked_sub(INVALID_THREASHOLD)
                .unwrap_or_else(Instant::now),
        ))
    });
    static SOCKET_GLOB: Lazy<String> = Lazy::new(|| {
        let mut glob = whoami::username();
        glob.insert_str(0, "/tmp/");
        glob.push_str("/.mpvsocket*");
        glob
    });

    let mut current = CURRENT.lock().await;
    if current.1.elapsed() >= INVALID_THREASHOLD {
        let mut available_sockets: Vec<_> = glob::glob(&SOCKET_GLOB)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| x.to_str().map(|x| SOCKET.is_match(x)).unwrap_or(false))
            .collect();
        available_sockets.sort();
        for s in available_sockets.into_iter().rev() {
            if let Ok(sock) = UnixStream::connect(&s).await {
                log::trace!("Opening a new socket {}", s.display());
                current.0 = s;
                current.1 = Instant::now();
                return Ok(sock);
            }
        }
        Err(io::ErrorKind::NotFound.into())
    } else {
        current.1 = Instant::now();
        UnixStream::connect(&current.0).await
    }
}

#[derive(Debug)]
enum Title {
    Simple(String),
    Complex { video: String, chapter: String },
}

impl Title {
    async fn fetch() -> io::Result<Title> {
        let media_title = get_property("media-title").await?.merge();
        #[derive(Deserialize)]
        struct ChapterMetadata {
            title: String,
        }
        if let Ok(Ok(chmeta)) = get_property::<ChapterMetadata>("chapter-metadata").await {
            Ok(Title::Complex {
                chapter: chmeta.title,
                video: media_title,
            })
        } else {
            Ok(Title::Simple(media_title))
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
        match self {
            Title::Simple(s) => {
                let (title, elipsis) = trunc(s);
                write!(f, "{}{}", title, elipsis)
            }
            Title::Complex { video, chapter } => {
                let g = crate::global_config::get();
                let (v, el) = trunc(video);
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
        }
    }
}

#[derive(Debug)]
struct BarData {
    title: Title,
    paused: bool,
    volume: f32,
}

impl BarData {
    async fn fetch() -> io::Result<Self> {
        let mut s = Self {
            title: Title::fetch().await?,
            paused: false,
            volume: 0.0,
        };
        s.update_paused().await?;
        s.update_volume().await?;
        Ok(s)
    }

    async fn update_title(&mut self) -> io::Result<()> {
        self.title = Title::fetch().await?;
        Ok(())
    }

    async fn update_paused(&mut self) -> io::Result<()> {
        self.paused = get_property("pause")
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }

    async fn update_volume(&mut self) -> io::Result<()> {
        self.volume = get_property("volume")
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }
}

impl Display for BarData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}%",
            self.title,
            if self.paused { "||" } else { ">" },
            self.volume.round() as u64
        )
    }
}

async fn get_property<D: DeserializeOwned>(p: &str) -> io::Result<Result<D, String>> {
    #[derive(Deserialize)]
    struct Payload<D> {
        data: D,
        error: String,
    }

    log::trace!("Opening socket");
    let mut sock = socket().await?;
    log::trace!("Checking if socket is writable");
    sock.writable().await?;
    log::trace!("Writing to the socket property '{}'", p);
    sock.write_all(&serde_json::to_vec(
        &serde_json::json!({ "command": [ "get_property", p ] }),
    )?)
    .await?;
    sock.write_all(b"\n").await?;

    let mut buf = Vec::with_capacity(1024);
    'readloop: loop {
        log::trace!("Waiting for the socket to become readable");
        sock.readable().await?;
        loop {
            log::trace!("Trying to read from socket");
            match sock.try_read_buf(&mut buf) {
                Ok(_) => break 'readloop,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    log::warn!("false positive read");
                }
                Err(e) => return Err(e),
            };
        }
    }

    if let Some(i) = buf.iter().position(|b| *b != 0) {
        log::debug!(
            "{} => {}",
            p,
            std::str::from_utf8(&buf)
                .unwrap_or("some error happened")
                .trim()
        );
        match buf[i..]
            .split(|&b| b == b'\n')
            .find_map(|b| serde_json::from_slice::<Payload<D>>(b).ok())
        {
            Some(p) => Ok(if p.error == "success" {
                Ok(p.data)
            } else {
                Err(p.error)
            }),
            None => Err(io::ErrorKind::InvalidData.into()),
        }
    } else {
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}
