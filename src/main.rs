use enum_iterator::IntoEnumIterator;
use lemon::{event_loop, model::Alignment, parsing::parse};
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
    let args = Args::from_args();
    let input = args
        .config
        .ok_or_else(|| io::ErrorKind::NotFound.into())
        .and_then(fs::read_to_string)
        .or_else(|_| {
            let arg = env::var_os("XDG_CONFIG_HOME")
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
            eprintln!("Loading from xdg");
            let mut path = PathBuf::from(arg);
            path.push("lemonbar");
            path.push("lemonrc.md");
            fs::read_to_string(path)
        })
        .or_else(|_| {
            let home =
                env::var_os("HOME").ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
            let mut path = PathBuf::from(home);
            path.push(".config");
            path.push("lemonbar");
            path.push("lemonrc.md");
            fs::read_to_string(path)
        })
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "Couldn't find config file"))?;
    let input = Box::leak(input.into_boxed_str());
    let (bc_send, mut bc_recv) = broadcast::channel(100);
    let (mpsc_send, mpsc_recv) = mpsc::channel(100);
    let blocks = match parse(input, args.bars, args.tray, &bc_send, &mpsc_send) {
        Ok(bs) => bs,
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1)
        }
    };
    println!("Staring blocks");
    for al in Alignment::into_enum_iter() {
        println!("{:?}", al);
        for b in &blocks[al] {
            println!("{:?}", b);
        }
    }
    // if cfg!(debug_assertions) {
        tokio::task::spawn({
            async move {
                while let Ok(ev) = bc_recv.recv().await {
                    eprintln!("[{:?}] event {:?} broadcasted", chrono::Utc::now(), ev)
                }
            }
        });
    // } else {
    //     drop(bc_recv);
    // }
    event_loop::start_event_loop(blocks, bc_send, mpsc_recv).await;
    Ok(())
}
