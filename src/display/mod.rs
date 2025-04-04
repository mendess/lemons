pub mod implementations;
pub mod lemonbar;
pub mod zelbar;

use std::{fmt, str::FromStr};

use crate::{
    event_loop::action_task::Action,
    model::{
        Alignment, Color,
        block::{AvailableActions, Block, BlockText, Font, Offset},
    },
};
pub use lemonbar::Lemonbar;
pub use zelbar::Zelbar;

#[derive(Debug, Clone, Copy, Default)]
pub enum Program {
    #[default]
    Lemonbar,
    Zelbar,
}

impl FromStr for Program {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lemonbar" => Ok(Self::Lemonbar),
            "zelbar" => Ok(Self::Zelbar),
            _ => Err(format!("unsuported program '{s}'")),
        }
    }
}

impl Program {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Zelbar => "zelbar",
            Self::Lemonbar => "lemonbar",
        }
    }
}

pub trait Bar<W: fmt::Write> {
    type BarBlockBuilder<'bar>: DisplayBlock
    where
        Self: 'bar;

    type CmdlineArgBuilder: CmdlineArgBuilder;

    const PROGRAM: &'static str;

    fn new(sink: W, separator: Option<String>) -> Self;

    fn cmdline_builder() -> Self::CmdlineArgBuilder;

    fn set_alignment(&mut self, alignment: Alignment) -> fmt::Result;

    fn start_block(&mut self, delimit: bool) -> Result<Self::BarBlockBuilder<'_>, fmt::Error>;

    fn into_inner(self) -> W;
}

pub trait CmdlineArgBuilder {
    fn output(&mut self, name: &str);
    fn height(&mut self, height: u32);
    fn bottom(&mut self);
    fn fonts<'s>(&mut self, fonts: impl Iterator<Item = &'s str>);
    fn name(&mut self, name: &str);
    fn underline_width(&mut self, width: u32);
    fn underline_color(&mut self, color: &Color);
    fn background(&mut self, color: &Color);
    fn foreground(&mut self, color: &Color);
    fn finish(self) -> Vec<String>;
}

pub trait DisplayBlock {
    fn offset(&mut self, offset: &Offset<'_>) -> fmt::Result;

    fn bg(&mut self, color: &Color) -> fmt::Result;

    fn fg(&mut self, color: &Color) -> fmt::Result;

    fn underline(&mut self, color: &Color) -> fmt::Result;

    fn font(&mut self, font: &Font<'_>) -> fmt::Result;

    fn add_action(&mut self, action: Action) -> fmt::Result;

    fn text(&mut self, body: &str, raw: bool) -> fmt::Result;

    fn finish(self) -> fmt::Result;
}

pub fn display_block<W: fmt::Write, B: Bar<W>>(
    bar: &mut B,
    block: &Block<'_>,
    text: &[BlockText],
    index: usize,
    monitor: u8,
) -> fmt::Result {
    for (i, text) in text.iter().filter(|b| !b.is_empty()).enumerate() {
        let mut builder = bar.start_block(i == 0)?;
        if let Some(x) = &block.offset {
            builder.offset(x)?;
        }
        if let Some(x) = text.decorations.bg.or(block.decorations.bg) {
            builder.bg(&x)?;
        }
        if let Some(x) = text.decorations.fg.or(block.decorations.fg) {
            builder.fg(&x)?;
        }
        if let Some(x) = text.decorations.underline.or(block.decorations.underline) {
            builder.underline(&x)?;
        }
        if let Some(x) = &block.font {
            builder.font(x)?;
        }
        for button in AvailableActions::from(block.available_actions.map(|o| o.is_some())).iter() {
            builder.add_action(Action::new(block.alignment, index, monitor, button))?;
        }
        builder.text(&text.text, block.raw)?;
        builder.finish()?;
    }
    Ok(())
}
