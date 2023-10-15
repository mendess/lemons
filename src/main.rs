use clap::Parser;
use enum_iterator::IntoEnumIterator;
use env_logger::Env;
use lemon::{
    display::{self, Program},
    event_loop,
    model::Alignment,
    parsing::parse,
};
use std::{env, fs, io, path::PathBuf};
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the config file
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// The parameters to pass to each bar
    #[arg(short, long("output"))]
    outputs: Vec<String>,
    #[arg(short, long)]
    tray: bool,
    #[arg(short, long, default_value = "lemonbar")]
    program: Program,
}

// TODO:
// Manpage
// - Sdir
// - attribute
#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();
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
    let blocks = match parse(
        input,
        args.outputs,
        args.tray,
        args.program,
        &bc_send,
        &mpsc_send,
    ) {
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
    match args.program {
        Program::Zelbar => {
            event_loop::start_event_loop::<display::Zelbar<_>>(blocks, bc_send, mpsc_recv).await
        }
        Program::Lemonbar => {
            event_loop::start_event_loop::<display::Lemonbar<_>>(blocks, bc_send, mpsc_recv).await
        }
    }
    Ok(())
}
