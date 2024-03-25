use super::{parser::KeyValues, ParseError, Result};
use crate::{
    event_loop::Event,
    global_config,
    model::{
        block::{self, *},
        ActiveMonitors, Alignment, Indexes, Layer,
    },
    util::signal::valid_rt_signum,
};
use std::{
    convert::TryInto, num::NonZeroU8, result::Result as StdResult, str::FromStr, time::Duration,
};
use tokio::sync::{broadcast, mpsc};

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

enum BlockType {
    Static,
    Cmd,
    Persistent,
    Native,
}

// What does a block do
//
// - Produces a string after some time
// - Listens to events
//   - Force refresh
//   - Layer changed
//   - Mouse button clicked
//
impl Block<'static> {
    pub fn from_kvs(
        n_monitors: NonZeroU8,
        indexes: &mut Indexes,
        iter: KeyValues<'static, '_>,
        broadcast: &broadcast::Sender<Event>,
        responses: &mpsc::Sender<BlockUpdate>,
    ) -> Result<'static, Self> {
        let mut decorations_b = TextDecorations::default();
        let mut block_b = BlockBuilder::default();
        let mut actions: Actions<'static> = Default::default();
        let mut signal = Signal::None;
        // mandatory parameters
        let mut cmd = None;
        let mut interval = None;
        let gc = global_config::get();
        for kvl in iter {
            let (key, value, _) = kvl?;
            log::trace!("{}: {}", key, value);
            let color = || {
                gc.get_color(value).copied().ok_or(("", "")).or_else(|_| {
                    value
                        .try_into()
                        .map_err(|error| ParseError::Color { value, error })
                })
            };
            match key {
                "background" | "bg" => {
                    decorations_b.bg = Some(color()?);
                }
                "foreground" | "fg" => {
                    decorations_b.fg = Some(color()?);
                }
                "underline" | "un" => {
                    decorations_b.underline = Some(color()?);
                }
                "font" => {
                    block_b.font(
                        value
                            .try_into()
                            .map_err(|error| ParseError::InvalidFont { value, error })?,
                    );
                }
                "offset" => {
                    block_b.offset(
                        value
                            .try_into()
                            .map_err(|_| ParseError::InvalidOffset(value))?,
                    );
                }
                "left-click" => {
                    actions[0] = Some(value);
                }
                "middle-click" => {
                    actions[1] = Some(value);
                }
                "right-click" => {
                    actions[2] = Some(value);
                }
                "scroll-up" => {
                    actions[3] = Some(value);
                }
                "scroll-down" => {
                    actions[4] = Some(value);
                }
                "interval" => {
                    interval = Some(Duration::from_secs(
                        value
                            .parse::<u64>()
                            .map_err(|_| ParseError::InvalidDuration(value))?,
                    ));
                }
                "command" | "cmd" => {
                    cmd = Some((value, BlockType::Cmd));
                }
                "static" => {
                    cmd = Some((value, BlockType::Static));
                }
                "persistent" => {
                    cmd = Some((value, BlockType::Persistent));
                }
                "native" => {
                    cmd = Some((value, BlockType::Native));
                }
                "alignment" | "align" => {
                    block_b.alignment(
                        value
                            .parse()
                            .map_err(|_| ParseError::InvalidAlignment(value))?,
                    );
                }
                "signal" => {
                    signal = value
                        .parse::<bool>()
                        .ok()
                        .map(|_| Signal::Any)
                        .or_else(|| {
                            value
                                .parse()
                                .ok()
                                .filter(|s| valid_rt_signum(*s))
                                .map(Signal::Num)
                        })
                        .ok_or(ParseError::InvalidSignal(value))?;
                }
                "raw" => {
                    block_b.raw(
                        value
                            .parse()
                            .map_err(|_| ParseError::InvalidBoolean(value))?,
                    );
                }
                "multi_monitor" => {
                    block_b.active_in(
                        if value
                            .parse()
                            .map_err(|_| ParseError::InvalidBoolean(value))?
                        {
                            ActiveMonitors::MonitorCount(n_monitors)
                        } else {
                            ActiveMonitors::All
                        },
                    );
                }
                "layer" => {
                    block_b.layer(value.parse().map_err(|_| ParseError::InvalidLayer(value))?);
                }
                s => {
                    log::warn!("unrecognised option '{}', skipping", s);
                }
            };
        }
        if let Some((value, kind)) = cmd {
            block_b.decorations(decorations_b);
            let mut block = block_b
                .build()
                .map_err(|e| ParseError::MalformedBlock(e.to_string()))?;
            // monitors.resize_one_or_more(&mut block.last_run);
            block
                .available_actions
                .set_all(actions.iter().map(Option::is_some));

            let task: Box<dyn BlockTask> = match kind {
                BlockType::Static => Box::new(block::constant::Static),
                BlockType::Cmd if interval.is_some() => {
                    Box::new(block::timed::Timed(interval.unwrap()))
                }
                BlockType::Cmd if signal != Signal::None => {
                    Box::new(block::timed::Timed(Duration::from_secs(u64::MAX)))
                }
                BlockType::Cmd => {
                    return Err(ParseError::MalformedBlock(
                        "Missing either signal or interval".into(),
                    ))
                }
                BlockType::Persistent => Box::new(block::persistent::Persistent),
                BlockType::Native => match block::native::new(value) {
                    Some(b) => b,
                    None => return Err(ParseError::InvalidNative(value)),
                },
            };
            let bid = (block.alignment, indexes.get(block.alignment));
            log::info!("Starting task {:?}({}) {:?}", task, value, bid);
            task.start(
                broadcast,
                TaskData {
                    cmd: value,
                    updates: responses.into(),
                    actions,
                    bid,
                    activation_layer: block.layer,
                    monitors: block.active_in,
                    signal,
                },
            );
            Ok(block)
        } else {
            Err(ParseError::MalformedBlock(
                "Missing content (cmd, persistent, native, static)".into(),
            ))
        }
    }
}
