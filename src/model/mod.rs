pub mod alignment;
pub mod block;
pub mod color;
pub mod global_config;
pub mod monitor;

use crate::event_loop::Event;
pub use alignment::Alignment;
use block::{Block, BlockUpdate};
pub use color::Color;
use core::fmt;
use futures::{stream::FuturesUnordered, StreamExt};
pub use monitor::ActiveMonitors;
use std::{
    future::Future,
    ops::{Index, IndexMut},
};
use tokio::sync::{broadcast, mpsc};

#[derive(Default)]
pub struct Config<'a>(pub [Vec<Block<'a>>; 3]);

impl Config<'static> {
    pub fn start_blocks(
        &self,
        broadcast: &broadcast::Sender<Event>,
        responses: mpsc::Sender<BlockUpdate>,
    ) -> impl Future<Output = ()> + use<> {
        let mut indexes = Indexes::default();
        let futures = self
            .0
            .iter()
            .flatten()
            .map(|b| {
                let task_as_str = format!("{:?}", b.task);
                let bid = (b.alignment, indexes.get(b.alignment));
                let broadcast = broadcast.subscribe();
                let updates = responses.clone().into();
                let cmd = b.cmd;

                let fut = b.start(bid, broadcast, updates);

                async move {
                    log::info!("Starting task {task_as_str}({}) {bid:?}", cmd);
                    fut.await;
                    log::info!("Terminating task {task_as_str}({}) {bid:?}", cmd);
                }
            })
            .collect::<FuturesUnordered<_>>();

        futures.collect::<()>()
    }
}

impl<'a> Index<Alignment> for Config<'a> {
    type Output = Vec<Block<'a>>;

    fn index(&self, a: Alignment) -> &Self::Output {
        &self.0[a as usize]
    }
}

impl IndexMut<Alignment> for Config<'_> {
    fn index_mut(&mut self, a: Alignment) -> &mut Self::Output {
        &mut self.0[a as usize]
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Indexes([usize; 3]);

impl Indexes {
    pub fn get(&mut self, alignment: Alignment) -> usize {
        let i = self.0[alignment as usize];
        self.0[alignment as usize] += 1;
        i
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActivationLayer {
    All,
    L(u16),
}

impl PartialEq for ActivationLayer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::All, _) => true,
            (_, Self::All) => true,
            (Self::L(l1), Self::L(l2)) => l1 == l2,
        }
    }
}

impl PartialEq<u16> for ActivationLayer {
    fn eq(&self, other: &u16) -> bool {
        self == &ActivationLayer::L(*other)
    }
}

impl ActivationLayer {
    pub fn next(&mut self, bound: u16) {
        *self = match self {
            Self::L(n) => Self::L((*n + 1) % bound),
            Self::All => panic!("Can't next an all layer"),
        }
    }
}

impl Default for ActivationLayer {
    fn default() -> Self {
        Self::All
    }
}

impl fmt::Display for ActivationLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L(n) => write!(f, "{n}"),
            Self::All => write!(f, "all"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AffectedMonitor {
    All,
    Single(u8),
}

impl From<u8> for AffectedMonitor {
    fn from(value: u8) -> Self {
        Self::Single(value)
    }
}

impl AffectedMonitor {
    pub fn single(self) -> Option<u8> {
        match self {
            Self::All => None,
            Self::Single(s) => Some(s),
        }
    }
}

impl fmt::Display for AffectedMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single(n) => write!(f, "{n}"),
            Self::All => write!(f, "all"),
        }
    }
}
