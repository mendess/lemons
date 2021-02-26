mod display;
mod event_loop;
mod model;
mod one_or_more;
mod parsing;

pub use model::block::{self, Alignment, Block};
pub use model::color::{self, Color};
pub use model::global_config::{self, GlobalConfig};
use parsing::parse;
use structopt::StructOpt;

use std::{collections::HashMap, env, fs, io, path::PathBuf};

type Config<'a> = HashMap<Alignment, Vec<Block<'a>>>;

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
    let input = if let Some(input) = args.config {
        fs::read_to_string(input)?
    } else {
        if let Some(arg) = env::var_os("XDG_CONFIG") {
            fs::read_to_string(arg)?
        } else if let Ok(home) = env::var("HOME") {
            fs::read_to_string(format!("{}/{}", home, ".config/lemonbar/lemonrc"))?
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Couldn't find config file",
            ));
        }
    };
    let input = Box::leak(input.into_boxed_str());
    let blocks = match parse(input, args.bars, args.tray) {
        Ok(b) => b,
        Err((bit, cause)) => {
            eprintln!("Parse error in '{}', {}", bit, cause);
            std::process::exit(1);
        }
    };
    println!("Staring blocks");
    for (al, bs) in &blocks {
        println!("{:?}", al);
        for b in bs {
            println!("{:?}", b);
        }
    }
    event_loop::start_event_loop(blocks);
    Ok(())
}
