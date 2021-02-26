use crate::{global_config, color::Color, one_or_more::OneOrMore};
use std::{
    io,
    process::{Command, Stdio},
    sync::{Arc, RwLock},
    time::Duration,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Alignment {
    Left,
    Middle,
    Right,
}

#[derive(Debug)]
pub enum Content<'a> {
    Static(&'a str),
    Cmd {
        cmd: &'a str,
        last_run: OneOrMore<RwLock<String>>,
    },
    Persistent {
        cmd: &'a str,
        last_run: OneOrMore<Arc<RwLock<String>>>,
    },
}

impl<'a> Content<'a> {
    #![allow(dead_code)]
    pub fn cmd(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Cmd { cmd, .. } => cmd,
            Self::Persistent { cmd, .. } => cmd,
        }
    }

    pub fn update(&self, layer: u16) {
        if let Self::Cmd { cmd, last_run } = self {
            for m in 0..last_run.len() {
                match Command::new("sh")
                    .args(&["-c", cmd])
                    .env("MONITOR", m.to_string())
                    .envs(global_config::get().as_env_vars())
                    .env("LAYER", layer.to_string())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .and_then(|c| c.wait_with_output())
                    .and_then(|o| {
                        if o.status.success() {
                            Ok(o.stdout)
                        } else {
                            Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                String::from_utf8_lossy(&o.stdout),
                            ))
                        }
                    })
                    .map_err(|e| e.to_string())
                    .and_then(|o| String::from_utf8(o).map_err(|e| e.to_string()))
                    .map(|mut l| {
                        if let Some(i) = l.find('\n') {
                            l.truncate(i);
                            l
                        } else {
                            l
                        }
                    }) {
                    Ok(o) => *(last_run[m].write().unwrap()) = o,
                    Err(e) => *(last_run[m].write().unwrap()) = e,
                }
            }
        }
    }

    pub fn is_empty(&self, monitor: usize) -> bool {
        match self {
            Self::Static(s) => s.is_empty(),
            Self::Cmd { last_run, .. } => last_run[monitor].read().unwrap().is_empty(),
            Self::Persistent { last_run, .. } => last_run[monitor].read().unwrap().is_empty(),
        }
    }

    pub fn replicate_to_mon(mut self, n_monitor: usize) -> Self {
        match &mut self {
            Self::Cmd { last_run, .. } => last_run.resize_with(n_monitor, Default::default),
            Self::Persistent { last_run, .. } => last_run.resize_with(n_monitor, Default::default),
            _ => (),
        }
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Layer {
    All,
    L(u16),
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::All, _) => true,
            (_, Self::All) => true,
            (Self::L(l1), Self::L(l2)) => l1 == l2,
        }
    }
}

impl PartialEq<u16> for Layer {
    fn eq(&self, other: &u16) -> bool {
        self == &Layer::L(*other)
    }
}

impl Layer {
    pub fn next(&mut self, bound: u16) {
        *self = match self {
            Self::L(n) => Self::L((*n + 1) % bound),
            Self::All => panic!("Can't next an all layer"),
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug)]
pub struct Block<'a> {
    pub bg: Option<Color<'a>>,
    pub fg: Option<Color<'a>>,
    pub un: Option<Color<'a>>,
    pub font: Option<&'a str>,   // 1-infinity index or '-'
    pub offset: Option<&'a str>, // u32
    pub actions: [Option<&'a str>; 5],
    pub content: Content<'a>,
    pub interval: Option<(Duration, RwLock<Duration>)>,
    pub alignment: Alignment,
    pub raw: bool,
    pub signal: bool,
    pub layer: Layer,
}
