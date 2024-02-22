use std::{
    io::{self, BufRead},
    iter::{repeat, zip},
    os::fd::{AsFd, AsRawFd},
    str::FromStr,
};

use clap::Parser;
use lemonade::bar::SurfaceConfig;
use lemonade::parser::Color;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1::Anchor,
};

#[derive(Debug, Clone)]
struct Geometry<const N: usize> {
    values: [u32; N],
}

impl<const N: usize> FromStr for Geometry<N> {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = || {
            format!(
                "invalid geometry. Expected format {} got {s}",
                zip(repeat("value"), repeat(":"))
                    .flat_map(|(v, s)| [v, s])
                    .take(N * 2 - 1)
                    .collect::<String>()
            )
        };
        let map_err = |_| error();

        let mut values = [0; N];
        let mut count = 0;
        for (s, v) in zip(s.split(':'), &mut values) {
            *v = s.parse().map_err(map_err)?;
            count += 1;
        }
        if count < N {
            return Err(error());
        }
        Ok(Self { values })
    }
}

fn color_parser(s: &str) -> Result<Color, String> {
    Color::parse(s)
        .ok_or_else(|| format!("invalid color: {s:?}"))
        .map(|(c, _)| c)
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    output: Option<String>,
    #[arg(long)]
    btm: bool,
    #[arg(short, long, default_value_t = 0)]
    layer: u8, // TODO: list possible values
    #[arg(short, long)]
    geometry: Geometry<2>,
    #[arg(short, long, default_value = "0:0:0:0")]
    margins: Geometry<4>,
    // fonts
    #[arg(value_parser = color_parser, short, long, default_value = "#000000")]
    bg: Color,
    #[arg(value_parser = color_parser, short, long, default_value = "#FFFFFF")]
    fg: Color,
}

fn main() -> io::Result<()> {
    let args = Args::parse();


    println!("setting up bar with args {args:?}");
    let (mut bar, mut event_loop) = lemonade::bar::Bar::new(SurfaceConfig {
        margins_top: args.margins.values[0].try_into().unwrap(),
        margins_bottom: args.margins.values[1].try_into().unwrap(),
        margins_right: args.margins.values[2].try_into().unwrap(),
        margins_left: args.margins.values[3].try_into().unwrap(),
        anchor: if args.btm {
            Anchor::Bottom
        } else {
            Anchor::Top
        },
        layer: match args.layer {
            0 => zwlr_layer_shell_v1::Layer::Background,
            1 => zwlr_layer_shell_v1::Layer::Bottom,
            2 => zwlr_layer_shell_v1::Layer::Top,
            _ => todo!(),
        },
        height: args.geometry.values[1],
    })
    .map_err(io::Error::other)?;

    let poll_descriptor = epoll::create(true)?;

    // wayland
    const STDIN_DESC: u64 = 1;
    let mut reader = std::io::stdin().lock();
    epoll::ctl(
        poll_descriptor,
        epoll::ControlOptions::EPOLL_CTL_ADD,
        reader.as_raw_fd(),
        epoll::Event {
            events: epoll::Events::EPOLLIN.bits(),
            data: STDIN_DESC,
        },
    )?;

    // stdin
    const WAY_DESC: u64 = 0;
    epoll::ctl(
        poll_descriptor,
        epoll::ControlOptions::EPOLL_CTL_ADD,
        event_loop.as_fd().as_raw_fd(),
        epoll::Event {
            events: epoll::Events::EPOLLIN.bits(),
            data: WAY_DESC,
        },
    )?;

    let mut buf = String::new();
    loop {
        buf.clear();
        let _wayland_read_guard = loop {
            if let Some(g) = event_loop.prepare_read() {
                break g;
            }
            event_loop.dispatch_pending(&mut bar).unwrap();
        };
        println!("epoll::wait");
        let mut event_buf = [epoll::Event { events: 0, data: 0 }; 2];
        let ready_count = epoll::wait(poll_descriptor, -1, &mut event_buf)?;
        println!("epoll::woke ({ready_count})");
        for ev in &event_buf[..ready_count] {
            if ev.data == WAY_DESC {
                event_loop.dispatch_pending(&mut bar).unwrap();
            } else if ev.data == STDIN_DESC {
                reader.read_line(&mut buf)?;
                match buf.pop() {
                    Some('\n') => {}
                    Some(other) => {
                        panic!("expected line to be terminated by newline, instead got {other}")
                    }
                    None => return Ok(()),
                }
                eprintln!("======================= NEW LINE =======================");
                eprintln!("{buf}");
                for b in lemonade::parser::parse(&buf) {
                    let b = b.map_err(io::Error::other)?;
                    eprintln!("{b:?}");
                }
            }
        }
    }
}
