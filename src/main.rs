use enum_iterator::IntoEnumIterator;
use lemon::{display::Lemonbar, event_loop, model::Alignment, parsing::parse};
use std::{env, fs, io, path::PathBuf};
use structopt::StructOpt;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Default, StructOpt)]
#[structopt(name = "lemons")]
struct Args {
    #[structopt(short, long, help("Path to the config file"))]
    config: Option<PathBuf>,
    #[structopt(short, long("--bar"), help("The parameters to pass to each bar"))]
    bars: Vec<String>,
    #[structopt(short, long)]
    tray: bool,
}

// TODO:
// Manpage
// - Sdir
// - attribute
#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();
    let args = Args::from_args();
    let input = args
        .config
        .ok_or(io::ErrorKind::NotFound)
        .map(|cfg| {
            log::info!("Loading config from command line");
            cfg
        })
        .or_else(|_| {
            env::var_os("XDG_CONFIG_HOME")
                .ok_or(io::ErrorKind::NotFound)
                .map(|arg| {
                    log::info!("Loading config from xdg config home {:?}", arg);
                    let mut path = PathBuf::from(arg);
                    path.extend(["lemonbar", "lemonrc.md"]);
                    path
                })
        })
        .or_else(|_| {
            env::var_os("HOME")
                .ok_or(io::ErrorKind::NotFound)
                .map(|home| {
                    log::info!("Loading config from ~/.config/lemonbar/lemonrc.md");
                    let mut path = PathBuf::from(home);
                    path.extend([".config", "lemonbar", "lemonrc.md"]);
                    path
                })
        })
        .map_err(io::Error::from)
        .and_then(fs::read_to_string)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "Couldn't find config file"))?;
    let input = Box::leak(input.into_boxed_str());
    let (bc_send, mut bc_recv) = broadcast::channel(100);
    let (mpsc_send, mpsc_recv) = mpsc::channel(100);
    let blocks = match parse(input, args.bars, args.tray, &bc_send, &mpsc_send) {
        Ok(bs) => bs,
        Err(e) => {
            log::error!("Parse error: {:?}", e);
            std::process::exit(1)
        }
    };
    log::trace!("Parsed blocks");
    for al in Alignment::into_enum_iter() {
        log::trace!("{:?}", al);
        for b in &blocks[al] {
            log::trace!("{:?}", b);
        }
    }
    if log::log_enabled!(log::Level::Debug) {
        tokio::task::spawn({
            async move {
                while let Ok(ev) = bc_recv.recv().await {
                    log::debug!("[{:?}] event {:?} broadcasted", chrono::Utc::now(), ev)
                }
            }
        });
    } else {
        drop(bc_recv);
    }
    event_loop::start_event_loop::<Lemonbar<_>>(blocks, bc_send, mpsc_recv).await;
    Ok(())
}
