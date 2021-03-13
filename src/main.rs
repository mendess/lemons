mod display;
mod event_loop;
mod model;
mod one_or_more;
mod parsing;
mod util;

use enum_iterator::IntoEnumIterator;
pub use model::block::{self, Alignment, Block};
pub use model::color::{self, Color};
pub use model::global_config::{self, GlobalConfig};
use parsing::parse;
use std::{
    env, fs, io,
    ops::{Index, IndexMut},
    path::PathBuf,
};
use structopt::StructOpt;

type Config<'a> = [Vec<Block<'a>>; 3];

impl<'a> Index<Alignment> for [Vec<Block<'a>>; 3] {
    type Output = Vec<Block<'a>>;
    fn index(&self, a: Alignment) -> &Self::Output {
        &self[a as usize]
    }
}

impl<'a> IndexMut<Alignment> for [Vec<Block<'a>>; 3] {
    fn index_mut(&mut self, a: Alignment) -> &mut Self::Output {
        &mut self[a as usize]
    }
}

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
fn main() -> io::Result<()> {
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
        .or_else(|_| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Couldn't find config file",
            ))
        })?;
    let input = Box::leak(input.into_boxed_str());
    let blocks = match parse(input, args.bars, args.tray) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("Staring blocks");
    for al in Alignment::into_enum_iter() {
        println!("{:?}", al);
        for b in &blocks[al] {
            println!("{:?}", b);
        }
    }
    event_loop::start_event_loop(blocks);
    Ok(())
}
