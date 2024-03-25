use std::{
    borrow::Cow,
    fmt,
    iter::{successors, zip},
    sync::OnceLock,
};

use regex::{Captures, Regex};

use crate::model::{Alignment, Color};

use super::{implementations::DisplayColor, CmdlineArgBuilder, DisplayBlock};

pub struct Zelbar<W> {
    sink: W,
    alignment: Alignment,
    separator: Option<&'static str>,
    already_wrote_first_block_of_aligment: bool,
}

#[derive(Default)]
pub struct ZelbarArgs {
    args: Vec<String>,
}

fn show_c(color: &Color) -> DisplayColor {
    DisplayColor::zelbar(*color)
}

impl CmdlineArgBuilder for ZelbarArgs {
    fn output(&mut self, name: &str) {
        self.args.extend(["-o".into(), name.into()]);
    }

    fn height(&mut self, height: u32) {
        self.args.extend(["-g".into(), format!("0:{height}")]);
    }

    fn bottom(&mut self) {
        self.args.push("-btm".into());
    }

    fn fonts<'s>(&mut self, fonts: impl Iterator<Item = &'s str>) {
        let mut buf = String::new();
        for (font, comma) in zip(fonts, successors(Some(false), |_| Some(true))) {
            if comma {
                buf.push(',');
            }
            buf.push_str(font);
        }
        self.args.extend(["-fn".into(), buf]);
    }

    fn name(&mut self, _name: &str) {}

    fn underline_width(&mut self, width: u32) {
        self.args.extend(["-lh".into(), width.to_string()]);
    }

    fn underline_color(&mut self, color: &Color) {
        self.args.extend(["-lc".into(), show_c(color).to_string()]);
    }

    fn background(&mut self, color: &Color) {
        self.args.extend(["-B".into(), show_c(color).to_string()]);
    }

    fn foreground(&mut self, color: &Color) {
        self.args.extend(["-F".into(), show_c(color).to_string()]);
    }

    fn finish(self) -> Vec<String> {
        self.args
    }
}

impl<W: fmt::Write> super::Bar<W> for Zelbar<W> {
    type BarBlockBuilder<'bar> = ZelbarDisplayBlock<'bar, W>
        where Self: 'bar;

    type CmdlineArgBuilder = ZelbarArgs;

    const PROGRAM: &'static str = "zelbar";

    fn new(sink: W, separator: Option<&'static str>) -> Self {
        Self {
            sink,
            separator,
            already_wrote_first_block_of_aligment: false,
            alignment: Alignment::Left,
        }
    }

    fn cmdline_builder() -> Self::CmdlineArgBuilder {
        ZelbarArgs::default()
    }

    fn set_alignment(&mut self, alignment: Alignment) -> fmt::Result {
        self.already_wrote_first_block_of_aligment = false;
        self.alignment = alignment;
        Ok(())
    }

    fn start_block(&mut self, delimit: bool) -> Result<Self::BarBlockBuilder<'_>, fmt::Error> {
        if self.already_wrote_first_block_of_aligment {
            if let Some(sep) = self.separator.filter(|_| delimit) {
                write!(self.sink, "{}", self.alignment)?;
                self.sink.write_str(sep)?;
            }
        } else {
            self.already_wrote_first_block_of_aligment = true;
        }
        write!(self.sink, "{}", self.alignment)?;
        Ok(ZelbarDisplayBlock::new(&mut self.sink))
    }

    fn into_inner(self) -> W {
        self.sink
    }
}

pub struct ZelbarDisplayBlock<'sink, W> {
    sink: &'sink mut W,
}

impl<'bar, W> ZelbarDisplayBlock<'bar, W> {
    fn new(sink: &'bar mut W) -> Self {
        Self { sink }
    }
}

impl<'bar, W> ZelbarDisplayBlock<'bar, W>
where
    W: fmt::Write,
{
    fn write<P, S>(&mut self, prefix: P, s: S) -> fmt::Result
    where
        P: fmt::Display,
        S: fmt::Display,
    {
        write!(self.sink, "%{{{}:{}}}", prefix, s)
    }
}

impl<'bar, W> DisplayBlock for ZelbarDisplayBlock<'bar, W>
where
    W: fmt::Write,
{
    fn offset(&mut self, offset: &crate::model::block::Offset<'_>) -> fmt::Result {
        self.write('X', offset.0)
    }

    fn bg(&mut self, color: &Color) -> fmt::Result {
        self.write('B', show_c(color))
    }

    fn fg(&mut self, color: &Color) -> fmt::Result {
        self.write('F', show_c(color))
    }

    fn underline(&mut self, color: &Color) -> fmt::Result {
        self.write('U', show_c(color))
    }

    fn font(&mut self, font: &crate::model::block::Font<'_>) -> fmt::Result {
        self.write('T', font.0)
    }

    fn add_action(&mut self, action: crate::event_loop::action_task::Action) -> fmt::Result {
        write!(self.sink, "%{{A{button}:{action}}}", button = action.button,)
    }

    fn text(&mut self, body: &str, raw: bool) -> fmt::Result {
        let body = if raw {
            Cow::Owned(compatibility_convert(body))
        } else {
            Cow::Borrowed(body)
        };
        self.sink.write_str(&body)
    }

    fn finish(self) -> fmt::Result {
        Ok(())
    }
}

fn compatibility_convert(body: &str) -> String {
    static COLORS: OnceLock<Regex> = OnceLock::new();
    static UNDERLINE: OnceLock<Regex> = OnceLock::new();
    static CLOSING: OnceLock<Regex> = OnceLock::new();
    let pat = COLORS.get_or_init(|| Regex::new(r"%\{([BFU])#([a-fA-F0-9]{6})\}").unwrap());
    let body = pat.replace_all(body, |captures: &Captures| {
        format!(
            "%{{{letter}:0x{color}}}",
            letter = &captures[1],
            color = &captures[2]
        )
    });
    let pat = UNDERLINE.get_or_init(|| Regex::new(r"%\{[-+]u\}").unwrap());
    let body = pat.replace_all(&body, "");
    let pat = CLOSING.get_or_init(|| Regex::new(r"%\{([BFU])-\}").unwrap());
    let body = pat.replace_all(&body, "");
    body.into_owned()
}
