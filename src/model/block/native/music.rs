use super::super::{BlockId, BlockTask, Signal, TaskData};
use crate::{
    event_loop::{current_layer, update_channel::UpdateChannel, Event, MouseButton},
    util::{result_ext::ResultExt, signal::sig_rt_min},
};
use futures::stream::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize};
use signal_hook_tokio::SignalsInfo;
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
};

#[derive(Debug, Clone, Copy)]
enum Update {
    Full,
    Volume,
    Title,
    State,
}

#[derive(Debug)]
pub struct Music;

impl BlockTask for Music {
    #[allow(unused_variables, unused_mut)]
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
        let (mut tx, rx) = mpsc::channel(10);
        tokio::spawn(event_loop(events.subscribe(), bid, updates, rx));
        if let Signal::Num(n) = signal {
            tokio::spawn(async move {
                let mut signals = match SignalsInfo::new(once(sig_rt_min() + n)) {
                    Ok(s) => s,
                    Err(e) => {
                        return eprintln!(
                            "= = = = = = Failed to start signal task for native music {}",
                            e
                        );
                    }
                };
                let mut sends = signals.then(|_| tx.send(()));
                tokio::pin!(sends);
                while let Some(Ok(_)) = sends.next().await {}
                eprintln!("music native terminating");
            });
        }
    }
}

async fn update_bar(bar: &mut Option<BarData>, how: Update) {
    match bar {
        Some(b) => {
            if match how {
                Update::Full => Ok(*bar = BarData::fetch().await.ok()),
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
    let mut bar = BarData::fetch().await.ok();
    while let Ok(e) = tokio::select! {
        e = events.recv() => e,
        Some(_) = ch.recv() => Ok(Event::Signal)
    } {
        match e {
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
                update_bar(&mut bar, up).await;
            }
            Event::MouseClicked(..) => (),
            Event::Refresh | Event::Signal => {
                update_bar(&mut bar, Update::Full).await;
            }
            Event::NewLayer => {
                update_bar(&mut bar, Update::Title).await;
            }
        }
        if let Some(b) = bar.as_ref().map(ToString::to_string) {
            if updates.send((b, bid, u8::MAX).into()).await.is_err() {
                break;
            }
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
                .unwrap_or_else(|| Instant::now()),
        ))
    });

    let mut current = CURRENT.lock().await;
    if current.1.elapsed() >= INVALID_THREASHOLD {
        let mut available_sockets: Vec<_> = glob::glob("/tmp/.mpvsocket*")
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| x.to_str().map(|x| SOCKET.is_match(x)).unwrap_or(false))
            .collect();
        available_sockets.sort();
        for s in available_sockets.into_iter().rev() {
            if let Ok(sock) = UnixStream::connect(&s).await {
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
                (&s[..], "")
            } else {
                (&s[..TRUNC_LEN], "...")
            }
        }
        match self {
            Title::Simple(s) => {
                let (title, elipsis) = trunc(&s);
                write!(f, "{}{}", title, elipsis)
            }
            Title::Complex { video, chapter } => {
                let g = crate::global_config::get();
                let (v, el) = trunc(&video);
                let (c, el1) = trunc(&chapter);
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
        Ok(self.title = Title::fetch().await?)
    }

    async fn update_paused(&mut self) -> io::Result<()> {
        Ok(self.paused = get_property("pause")
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?)
    }

    async fn update_volume(&mut self) -> io::Result<()> {
        Ok(self.volume = get_property("volume")
            .await?
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?)
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

    let mut sock = socket().await?;
    sock.writable().await?;
    sock.write_all(&serde_json::to_vec(
        &serde_json::json!({ "command": [ "get_property", p ] }),
    )?)
    .await?;
    sock.write(b"\n").await?;

    let mut buf = Vec::with_capacity(1024);
    'readloop: loop {
        sock.readable().await?;
        loop {
            match sock.try_read_buf(&mut buf) {
                Ok(_) => break 'readloop,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    eprintln!("false positive read");
                    break;
                }
                Err(e) => return Err(e),
            };
        }
    }

    if let Some(i) = buf.iter().position(|b| *b != 0) {
        eprintln!(
            "{} => {}",
            p,
            std::str::from_utf8(&buf).unwrap_or("some error happened")
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
