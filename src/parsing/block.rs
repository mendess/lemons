use super::{parser::KeyValues, ParseError, Result};
use crate::{
    block::{Alignment, Block, Content, Layer},
    Color, GlobalConfig,
};
use std::{result::Result as StdResult, str::FromStr, time::Duration};

impl FromStr for Alignment {
    type Err = &'static str;
    fn from_str(s: &str) -> StdResult<Self, <Self as FromStr>::Err> {
        match s {
            "left" | "Left" => Ok(Self::Left),
            "middle" | "Middle" => Ok(Self::Middle),
            "right" | "Right" => Ok(Self::Right),
            _ => Err("Invalid alignment"),
        }
    }
}

impl FromStr for Layer {
    type Err = &'static str;
    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        match s {
            "all" | "All" => Ok(Self::All),
            s => match s.parse::<u16>() {
                Ok(n) => Ok(Self::L(n)),
                _ => Err("Invalid layer"),
            },
        }
    }
}

impl<'a> Block<'a> {
    pub fn from_kvs<'parser>(
        gconfig: &GlobalConfig<'a>,
        iter: KeyValues<'a, 'parser>,
        n_monitor: usize,
    ) -> Result<'a, Self> {
        let mut block_b = BlockBuilder::default();
        for kvl in iter {
            let (key, value, _) = kvl?;
            eprintln!("{}: {}", key, value);
            let color = || {
                gconfig
                    .get_color(value)
                    .map(|&c| c)
                    .ok_or(("", ""))
                    .or_else(|_| {
                        Color::from_str(value).map_err(|error| ParseError::Color { value, error })
                    })
            };
            match key {
                "background" | "bg" => block_b.bg(color()?),
                "foreground" | "fg" => block_b.fg(color()?),
                "underline" | "un" => block_b.un(color()?),
                "font" => block_b
                    .font(value)
                    .map_err(|error| ParseError::InvalidFont { value, error })?,
                "offset" => block_b
                    .offset(value)
                    .map_err(|_| ParseError::InvalidOffset(value))?,
                "left-click" => block_b.action(0, value),
                "middle-click" => block_b.action(1, value),
                "right-click" => block_b.action(2, value),
                "scroll-up" => block_b.action(3, value),
                "scroll-down" => block_b.action(4, value),
                "interval" => block_b.interval(Duration::from_secs(
                    value
                        .parse::<u64>()
                        .map_err(|_| ParseError::InvalidDuration(value))?,
                )),
                "command" | "cmd" => block_b.content_command(value),
                "static" => block_b.content_static(value),
                "persistent" => block_b.content_persistent(value),
                "alignment" | "align" => block_b.alignment(
                    value
                        .parse()
                        .map_err(|_| ParseError::InvalidAlignment(value))?,
                ),
                "signal" => block_b.signal(
                    value
                        .parse()
                        .map_err(|_| ParseError::InvalidBoolean(value))?,
                ),
                "raw" => block_b.raw(
                    value
                        .parse()
                        .map_err(|_| ParseError::InvalidBoolean(value))?,
                ),
                "multi_monitor" => block_b.multi_monitor(
                    value
                        .parse()
                        .map_err(|_| ParseError::InvalidBoolean(value))?,
                ),
                "layer" => {
                    block_b.layer(value.parse().map_err(|_| ParseError::InvalidLayer(value))?)
                }
                s => {
                    eprintln!("Warning: unrecognised option '{}', skipping", s);
                    &mut block_b
                }
            };
        }
        Ok(block_b
            .build(n_monitor)
            .map_err(|e| ParseError::MalformedBlock(e))?)
    }
}

#[derive(Default)]
pub struct BlockBuilder<'a> {
    bg: Option<Color<'a>>,
    fg: Option<Color<'a>>,
    un: Option<Color<'a>>,
    font: Option<&'a str>,   // 1-infinity index or '-'
    offset: Option<&'a str>, // u32
    actions: [Option<&'a str>; 5],
    content: Option<Content<'a>>,
    interval: Option<Duration>,
    alignment: Option<Alignment>,
    raw: bool,
    signal: bool,
    multi_monitor: bool,
    layer: Layer,
}

impl<'a> BlockBuilder<'a> {
    fn raw(&mut self, r: bool) -> &mut Self {
        self.raw = r;
        self
    }

    fn action(&mut self, index: usize, action: &'a str) -> &mut Self {
        self.actions[index] = Some(action);
        self
    }

    fn bg(&mut self, c: Color<'a>) -> &mut Self {
        self.bg = Some(c);
        self
    }

    fn fg(&mut self, c: Color<'a>) -> &mut Self {
        self.fg = Some(c);
        self
    }

    fn un(&mut self, c: Color<'a>) -> &mut Self {
        self.un = Some(c);
        self
    }

    fn font(&mut self, font: &'a str) -> StdResult<&mut Self, &'static str> {
        if font == "-" || font.parse::<u32>().map_err(|_| "Invalid font")? > 0 {
            self.font = Some(font);
            Ok(self)
        } else {
            Err("Invalid index")
        }
    }

    fn offset(&mut self, o: &'a str) -> StdResult<&mut Self, <u32 as FromStr>::Err> {
        o.parse::<u32>()?;
        self.offset = Some(o);
        Ok(self)
    }

    fn content_command(&mut self, c: &'a str) -> &mut Self {
        self.content = Some(Content::Cmd {
            cmd: c,
            last_run: Default::default(),
        });
        self
    }

    fn content_static(&mut self, c: &'a str) -> &mut Self {
        self.content = Some(Content::Static(c));
        self
    }

    fn content_persistent(&mut self, c: &'a str) -> &mut Self {
        self.content = Some(Content::Persistent {
            cmd: c,
            last_run: Default::default(),
        });
        self
    }

    fn interval(&mut self, i: Duration) -> &mut Self {
        self.interval = Some(i);
        self
    }

    fn alignment(&mut self, a: Alignment) -> &mut Self {
        self.alignment = Some(a);
        self
    }

    fn signal(&mut self, b: bool) -> &mut Self {
        self.signal = b;
        self
    }

    fn multi_monitor(&mut self, b: bool) -> &mut Self {
        self.multi_monitor = b;
        self
    }

    fn layer(&mut self, layer: Layer) -> &mut Self {
        self.layer = layer;
        self
    }

    pub fn build(self, n_monitor: usize) -> StdResult<Block<'a>, &'static str> {
        let n_monitor = if self.multi_monitor { n_monitor } else { 1 };
        if let Some(content) = self.content {
            if let Some(alignment) = self.alignment {
                Ok(Block {
                    bg: self.bg,
                    fg: self.fg,
                    un: self.un,
                    font: self.font,
                    offset: self.offset,
                    content: content.replicate_to_mon(n_monitor),
                    interval: self.interval.map(|i| (i, Default::default())),
                    actions: self.actions,
                    alignment,
                    raw: self.raw,
                    signal: self.signal,
                    layer: self.layer,
                })
            } else {
                Err("No alignment defined")
            }
        } else {
            Err("No content defined")
        }
    }
}
