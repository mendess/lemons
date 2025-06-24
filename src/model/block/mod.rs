pub mod constant;
pub mod native;
pub mod persistent;
pub mod signal_task;
pub mod timed;

use super::{ActivationLayer, ActiveMonitors, AffectedMonitor, Alignment, Color};
use crate::{
    event_loop::{Event, MouseButton, update_task::UpdateChannel},
    parsing::parser::Title,
};
use derive_builder::Builder;
use futures::future::BoxFuture;
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

#[derive(Clone, Default, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Signal {
    #[default]
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
    pub block_name: Title<'static>,
    pub cmd: &'static str,
    pub updates: UpdateChannel,
    pub actions: Actions<'static>,
    pub bid: BlockId,
    pub activation_layer: ActivationLayer,
    pub monitors: ActiveMonitors,
    pub signal: Signal,
    pub precondition: Option<Precondition<'static>>,
}

pub trait BlockTask: std::fmt::Debug {
    fn start(&self, events: broadcast::Receiver<Event>, data: TaskData) -> BoxFuture<'static, ()>;
}

pub type Actions<'s> = [Option<&'s str>; 5];

impl<'s> Index<MouseButton> for Actions<'s> {
    type Output = Option<&'s str>;

    fn index(&self, index: MouseButton) -> &Self::Output {
        &self[index as usize - 1]
    }
}

impl IndexMut<MouseButton> for Actions<'_> {
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

#[derive(Clone, Copy, Debug)]
pub enum Precondition<'a> {
    FileExists(&'a std::path::Path),
}

impl Precondition<'_> {
    pub async fn holds(this: &Option<Precondition<'_>>) -> bool {
        if let Some(pre) = this {
            match pre {
                Precondition::FileExists(path) => {
                    return matches!(tokio::fs::try_exists(path).await, Ok(true));
                }
            }
        }
        true
    }
}

#[derive(Builder, Debug)]
#[builder(setter(strip_option), build_fn(skip, name = "build"))]
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
    #[builder(default)]
    pub active_in: ActiveMonitors,
    #[builder(default)]
    pub signal: Signal,
    #[builder(default)]
    pub precondition: Option<Precondition<'a>>,

    // mandatory
    #[builder(setter(skip), default)] // custom setter is just not providing one
    title: Title<'a>,
    #[builder(setter(skip), default)]
    pub alignment: Alignment,
    #[builder(setter(skip), default)]
    pub available_actions: [Option<&'a str>; 5],
    #[builder(setter(skip), default)]
    pub cmd: &'a str,
    #[builder(setter(skip), default)]
    pub task: Box<dyn BlockTask>,
}

impl Block<'static> {
    pub fn start(
        &self,
        block_id: BlockId,
        broadcast: broadcast::Receiver<Event>,
        updates: UpdateChannel,
    ) -> BoxFuture<'static, ()> {
        self.task.start(
            broadcast,
            TaskData {
                block_name: self.title,
                cmd: self.cmd,
                updates,
                actions: self.available_actions,
                bid: block_id,
                activation_layer: self.layer,
                monitors: self.active_in,
                signal: self.signal,
                precondition: self.precondition,
            },
        )
    }
}
impl<'a> BlockBuilder<'a> {
    pub fn has_signal(&self) -> bool {
        self.signal.is_some()
    }

    pub fn build(
        self,
        title: Title<'a>,
        cmd: &'a str,
        alignment: Alignment,
        available_actions: [Option<&'a str>; 5],
        task: Box<dyn BlockTask>,
    ) -> Block<'a> {
        Block {
            title,
            cmd,
            decorations: self.decorations.unwrap_or_default(),
            font: self.font.unwrap_or_default(),
            offset: self.offset.unwrap_or_default(),
            raw: self.raw.unwrap_or_default(),
            layer: self.layer.unwrap_or_default(),
            alignment,
            active_in: self.active_in.unwrap_or_default(),
            available_actions,
            task,
            signal: self.signal.unwrap_or(Signal::None),
            precondition: self.precondition.unwrap_or_default(),
        }
    }
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

impl From<[bool; 5]> for AvailableActions {
    fn from(value: [bool; 5]) -> Self {
        let mut s = Self::default();
        s.set_all(value.into_iter());
        s
    }
}
