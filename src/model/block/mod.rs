pub mod constant;
pub mod native;
pub mod persistent;
pub mod signal_task;
pub mod timed;

use super::{ActivationLayer, ActiveMonitors, AffectedMonitor, Alignment, Color};
use crate::event_loop::{update_channel::UpdateChannel, Event, MouseButton};
use derive_builder::Builder;
use std::{
    convert::TryFrom,
    ops::{Index, IndexMut},
    os::raw::c_int,
};
use tokio::sync::broadcast;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockText {
    pub decorations: TextDecorations,
    pub text: String,
}

impl From<String> for BlockText {
    fn from(text: String) -> Self {
        Self {
            decorations: Default::default(),
            text,
        }
    }
}

impl BlockText {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[derive(Debug)]
pub struct BlockUpdate {
    text: Vec<BlockText>,
    alignment: Alignment,
    index: usize,
    monitor: AffectedMonitor,
}

pub type BlockId = (Alignment, usize);

impl<M: Into<AffectedMonitor>> From<(String, BlockId, M)> for BlockUpdate {
    fn from((text, (alignment, index), monitor): (String, BlockId, M)) -> Self {
        Self {
            text: vec![BlockText {
                decorations: Default::default(),
                text,
            }],
            alignment,
            index,
            monitor: monitor.into(),
        }
    }
}

impl<M: Into<AffectedMonitor>> From<(Vec<BlockText>, BlockId, M)> for BlockUpdate {
    fn from((text, (alignment, index), monitor): (Vec<BlockText>, BlockId, M)) -> Self {
        Self {
            text,
            alignment,
            index,
            monitor: monitor.into(),
        }
    }
}

impl BlockUpdate {
    pub fn into_inner_text(self) -> Vec<BlockText> {
        self.text
    }
}

impl BlockUpdate {
    pub fn id(&self) -> (Alignment, usize, AffectedMonitor) {
        (self.alignment, self.index, self.monitor)
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
    pub block_name: &'static str,
    pub cmd: &'static str,
    pub updates: UpdateChannel,
    pub actions: Actions<'static>,
    pub bid: BlockId,
    pub activation_layer: ActivationLayer,
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
        &self[index as usize - 1]
    }
}

impl<'s> IndexMut<MouseButton> for Actions<'s> {
    fn index_mut(&mut self, index: MouseButton) -> &mut Self::Output {
        &mut self[index as usize - 1]
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

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct TextDecorations {
    pub bg: Option<Color>,
    pub fg: Option<Color>,
    pub underline: Option<Color>,
}

impl TextDecorations {
    pub fn is_default(&self) -> bool {
        self.bg.is_none() && self.fg.is_none() && self.bg.is_none()
    }
}

#[derive(Builder, Debug)]
#[builder(setter(strip_option))]
pub struct Block<'a> {
    #[builder(default)]
    pub decorations: TextDecorations,
    #[builder(default)]
    pub font: Option<Font<'a>>, // 1-infinity index or '-'
    #[builder(default)]
    pub offset: Option<Offset<'a>>, // u32
    #[builder(default)]
    pub raw: bool,
    #[builder(default)]
    pub layer: ActivationLayer,

    pub alignment: Alignment,

    #[builder(default)]
    pub active_in: ActiveMonitors,
    // #[builder(setter(skip))]
    // pub last_run: OneOrMore<String>,
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
