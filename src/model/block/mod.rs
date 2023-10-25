pub mod constant;
pub mod native;
pub mod persistent;
pub mod signal_task;
pub mod timed;

use super::{ActiveMonitors, Alignment, Color, Layer};
use crate::{
    event_loop::{update_channel::UpdateChannel, Event, MouseButton},
    util::one_or_more::OneOrMore,
};
use derive_builder::Builder;
use std::{
    convert::TryFrom,
    ops::{Index, IndexMut},
    os::raw::c_int,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct BlockUpdate {
    text: String,
    alignment: Alignment,
    index: usize,
    monitor: u8,
}

pub type BlockId = (Alignment, usize);

impl From<(String, BlockId, u8)> for BlockUpdate {
    fn from((text, (alignment, index), monitor): (String, BlockId, u8)) -> Self {
        Self {
            text,
            alignment,
            index,
            monitor,
        }
    }
}

impl BlockUpdate {
    pub fn id(&self) -> (Alignment, usize, u8) {
        (self.alignment, self.index, self.monitor)
    }

    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Signal {
    None,
    Any,
    Num(c_int),
}

impl Signal {
    fn is_some(&self) -> bool {
        matches!(self, Signal::Any | Signal::Num(_))
    }
}

pub struct TaskData {
    pub cmd: &'static str,
    pub updates: UpdateChannel,
    pub actions: Actions<'static>,
    pub bid: BlockId,
    pub activation_layer: Layer,
    pub monitors: ActiveMonitors,
    pub signal: Signal,
}

pub trait BlockTask: std::fmt::Debug {
    fn start(&self, events: &broadcast::Sender<Event>, data: TaskData);
}

pub type Actions<'s> = [Option<&'s str>; 5];

impl<'s> Index<MouseButton> for Actions<'s> {
    type Output = Option<&'s str>;

    fn index(&self, index: MouseButton) -> &Self::Output {
        &self[index as usize]
    }
}

impl<'s> IndexMut<MouseButton> for Actions<'s> {
    fn index_mut(&mut self, index: MouseButton) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Font<'a>(pub &'a str);

impl<'a> TryFrom<&'a str> for Font<'a> {
    type Error = &'static str;

    fn try_from(font: &'a str) -> std::result::Result<Self, Self::Error> {
        if font == "-" || font.parse::<u32>().map_err(|_| "Invalid font")? > 0 {
            Ok(Self(font))
        } else {
            Err("Invalid index")
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Offset<'a>(pub &'a str);

impl<'a> TryFrom<&'a str> for Offset<'a> {
    type Error = <u32 as std::str::FromStr>::Err;

    fn try_from(offset: &'a str) -> std::result::Result<Self, Self::Error> {
        offset.parse::<u32>()?;
        Ok(Self(offset))
    }
}

#[derive(Builder, Debug)]
#[builder(setter(strip_option))]
pub struct Block<'a> {
    #[builder(default)]
    pub bg: Option<Color>,
    #[builder(default)]
    pub fg: Option<Color>,
    #[builder(default)]
    pub un: Option<Color>,
    #[builder(default)]
    pub font: Option<Font<'a>>, // 1-infinity index or '-'
    #[builder(default)]
    pub offset: Option<Offset<'a>>, // u32
    #[builder(default)]
    pub raw: bool,
    #[builder(default)]
    pub layer: Layer,

    pub alignment: Alignment,

    #[builder(setter(skip))]
    pub last_run: OneOrMore<String>,

    #[builder(setter(skip), default)]
    pub available_actions: AvailableActions,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct AvailableActions(u8);

impl AvailableActions {
    pub fn set(&mut self, index: u8) {
        debug_assert!(index < 6);
        self.0 |= 1u8 << index;
    }

    pub fn set_all<I: Iterator<Item = bool>>(&mut self, i: I) {
        for (i, b) in i.enumerate() {
            if b {
                self.set(i as _)
            }
        }
    }

    pub fn iter(self) -> impl Iterator<Item = MouseButton> {
        (0..5)
            .filter(move |i| self.0 & (1 << i) != 0)
            .map(|i| i + 1)
            .map(Into::into)
    }
}
