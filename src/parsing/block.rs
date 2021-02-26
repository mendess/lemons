use super::{parse_key_value, ParseError};
use crate::{
    block::{Alignment, Block, Content, Layer},
    Color,
    GlobalConfig
};
use std::{str::FromStr, time::Duration};

impl FromStr for Alignment {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
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
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
    pub fn parse(
        block: &'a str,
        n_monitor: usize,
        gconfig: &GlobalConfig<'a>,
    ) -> Result<Self, ParseError<'a>> {
        let mut block_b = BlockBuilder::default();
        for opt in block.split('\n').skip(1).filter(|s| !s.trim().is_empty()) {
            let (key, value) = parse_key_value(opt)?;
            eprintln!("{}: {}", key, value);
            let color = || {
                gconfig
                    .get_color(value)
                    .map(|&c| c)
                    .ok_or(("", ""))
                    .or_else(|_| Color::from_str(value).map_err(|e| (opt, e)))
            };
            block_b = match key
                .trim()
                .trim_start_matches('*')
                .trim_start_matches('-')
                .trim()
            {
                "background" | "bg" => block_b.bg(color()?),
                "foreground" | "fg" => block_b.fg(color()?),
                "underline" | "un" => block_b.un(color()?),
                "font" => block_b.font(value).map_err(|e| (opt, e))?,
                "offset" => block_b.offset(value).map_err(|_| (opt, "invalid offset"))?,
                "left-click" => block_b.action(0, value),
                "middle-click" => block_b.action(1, value),
                "right-click" => block_b.action(2, value),
                "scroll-up" => block_b.action(3, value),
                "scroll-down" => block_b.action(4, value),
                "interval" => block_b.interval(Duration::from_secs(
                    value
                        .parse::<u64>()
                        .map_err(|_| (opt, "Invalid duration"))?,
                )),
                "command" | "cmd" => block_b.content_command(value),
                "static" => block_b.content_static(value),
                "persistent" => block_b.content_persistent(value),
                "alignment" | "align" => block_b.alignment(value.parse().map_err(|e| (opt, e))?),
                "signal" => block_b.signal(value.parse().map_err(|_| (opt, "Invalid boolean"))?),
                "raw" => block_b.raw(value.parse().map_err(|_| (opt, "Invalid boolean"))?),
                "multi_monitor" => {
                    block_b.multi_monitor(value.parse().map_err(|_| (opt, "Invalid boolean"))?)
                }
                "layer" => block_b.layer(value.parse().map_err(|e| (opt, e))?),
                s => {
                    eprintln!("Warning: unrecognised option '{}', skipping", s);
                    block_b
                }
            };
        }
        block_b
            .build(n_monitor)
            .map_err(|e| ("BLOCK DEFINITION", e))
    }
}

#[derive(Default)]
struct BlockBuilder<'a> {
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
    fn raw(self, r: bool) -> Self {
        Self { raw: r, ..self }
    }

    fn action(mut self, index: usize, action: &'a str) -> Self {
        self.actions[index] = Some(action);
        self
    }

    fn bg(self, c: Color<'a>) -> Self {
        Self {
            bg: Some(c),
            ..self
        }
    }

    fn fg(self, c: Color<'a>) -> Self {
        Self {
            fg: Some(c),
            ..self
        }
    }

    fn un(self, c: Color<'a>) -> Self {
        Self {
            un: Some(c),
            ..self
        }
    }

    fn font(self, font: &'a str) -> Result<Self, &'static str> {
        if font == "-" || font.parse::<u32>().map_err(|_| "Invalid font")? > 0 {
            Ok(Self {
                font: Some(font),
                ..self
            })
        } else {
            Err("Invalid index")
        }
    }

    fn offset(self, o: &'a str) -> Result<Self, <u32 as FromStr>::Err> {
        o.parse::<u32>()?;
        Ok(Self {
            offset: Some(o),
            ..self
        })
    }

    fn content_command(self, c: &'a str) -> Self {
        Self {
            content: Some(Content::Cmd {
                cmd: c,
                last_run: Default::default(),
            }),
            ..self
        }
    }

    fn content_static(self, c: &'a str) -> Self {
        Self {
            content: Some(Content::Static(c)),
            ..self
        }
    }

    fn content_persistent(self, c: &'a str) -> Self {
        Self {
            content: Some(Content::Persistent {
                cmd: c,
                last_run: Default::default(),
            }),
            ..self
        }
    }

    fn interval(self, i: Duration) -> Self {
        Self {
            interval: Some(i),
            ..self
        }
    }

    fn alignment(self, a: Alignment) -> Self {
        Self {
            alignment: Some(a),
            ..self
        }
    }

    fn signal(self, b: bool) -> Self {
        Self { signal: b, ..self }
    }

    fn multi_monitor(self, b: bool) -> Self {
        Self {
            multi_monitor: b,
            ..self
        }
    }

    fn layer(self, layer: Layer) -> Self {
        Self { layer, ..self }
    }

    fn build(self, n_monitor: usize) -> Result<Block<'a>, &'static str> {
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
