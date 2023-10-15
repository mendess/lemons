mod implementations;
mod lemonbar;

use std::fmt;

use crate::{
    event_loop::action_task::Action,
    model::{
        block::{Block, Font, Offset},
        Alignment, Color,
    },
};
pub use lemonbar::Lemonbar;
use std::borrow::Cow;

pub trait Bar<W: fmt::Write> {
    type BarBlockBuilder<'bar>: DisplayBlock
    where
        Self: 'bar;

    fn new(sink: W, separator: Option<&'static str>) -> Self;

    fn set_alignment(&mut self, alignment: Alignment) -> fmt::Result;

    fn start_block(&mut self) -> Result<Self::BarBlockBuilder<'_>, fmt::Error>;

    fn into_inner(self) -> W;
}

pub trait DisplayBlock {
    fn offset(&mut self, offset: &Offset<'_>) -> fmt::Result;

    fn bg(&mut self, color: &Color<'_>) -> fmt::Result;

    fn fg(&mut self, color: &Color<'_>) -> fmt::Result;

    fn underline(&mut self, color: &Color<'_>) -> fmt::Result;

    fn font(&mut self, font: &Font<'_>) -> fmt::Result;

    fn add_action(&mut self, action: Action) -> fmt::Result;

    fn text(&mut self, body: &str) -> fmt::Result;

    fn finish(self) -> fmt::Result;
}

pub fn display_block<W: fmt::Write, B: Bar<W>>(
    bar: &mut B,
    block: &Block<'_>,
    index: usize,
    monitor: u8,
) -> fmt::Result {
    let mut builder = bar.start_block()?;
    if let Some(x) = &block.offset {
        builder.offset(x)?;
    }
    if let Some(x) = &block.bg {
        builder.bg(x)?;
    }
    if let Some(x) = &block.fg {
        builder.fg(x)?;
    }
    if let Some(x) = &block.un {
        builder.underline(x)?;
    }
    if let Some(x) = &block.font {
        builder.font(x)?;
    }
    for button in block.available_actions.iter() {
        builder.add_action(Action::new(block.alignment, index, monitor, button))?;
    }
    let body = block.last_run[monitor].trim_end_matches('\n');
    let body = if block.raw {
        if body.ends_with('%') {
            Cow::Owned(format!("{}%", body))
        } else {
            Cow::Borrowed(body)
        }
    } else if body.contains('%') {
        Cow::Owned(body.replace('%', "%%"))
    } else {
        Cow::Borrowed(body)
    };
    builder.text(&body)?;
    builder.finish()
}
