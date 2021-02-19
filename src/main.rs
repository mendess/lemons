mod display;
mod event_loop;
mod model;
mod one_or_more;
mod parsing;

pub use model::block::{self, Alignment, Block};
pub use model::color::{self, Color};
pub use model::global_config::{self, GlobalConfig};
use parsing::parse;

use std::{collections::HashMap, env, fs, io};

type Config<'a> = HashMap<Alignment, Vec<Block<'a>>>;

#[derive(Debug, Default)]
struct Args {
    config: Option<String>,
    bars: Vec<String>,
    tray: bool,
}

fn arg_parse() -> io::Result<Args> {
    let mut args = Args::default();
    let mut argv = env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                args.config = Some(fs::read_to_string(argv.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "Expected argument to geometry parameter",
                    )
                })?)?);
            }
            "-t" | "--tray" => args.tray = true,
            "-b" | "--bar" => {
                args.bars.push(argv.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "Expected argument to geometry parameter",
                    )
                })?);
            }
            _ => (),
        }
    }
    Ok(args)
}

// TODO:
// Manpage
// - Sdir
// - attribute
//
// Features
// - Signal
fn main() -> io::Result<()> {
    let args = arg_parse()?;
    let input = if let Some(input) = args.config {
        input
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
