mod block;
mod color;
mod global_config;
pub mod parser;

use std::num::NonZeroU8;

use crate::{
    event_loop::Event,
    global_config::GlobalConfig,
    model::Layer,
    model::{
        block::{Block, BlockUpdate},
        Indexes,
    },
    Config,
};
use tokio::sync::{broadcast, mpsc};

#[derive(Debug)]
pub enum ParseError<'a> {
    Colon(&'a str),
    ExpectedTitle(&'a str),
    ExpectedAttribute(&'a str),
    Color { value: &'a str, error: &'static str },
    InvalidBoolean(&'a str),
    InvalidInteger(&'a str),
    InvalidDuration(&'a str),
    InvalidOffset(&'a str),
    InvalidFont { value: &'a str, error: &'static str },
    InvalidAlignment(&'a str),
    InvalidLayer(&'a str),
    InvalidSignal(&'a str),
    InvalidNative(&'a str),
    MalformedBlock(String),
    MissingAlignment,
    MissingContent,
    TooManyBarSpecs { got: usize, max: u8 },
    NeedAtLeastOneBarSpec,
}

pub type Result<'a, T> = std::result::Result<T, ParseError<'a>>;

pub fn parse(
    config: &'static str,
    bars: Vec<String>,
    tray: bool,
    broadcast: &broadcast::Sender<Event>,
    responses: &mpsc::Sender<BlockUpdate>,
) -> Result<'static, Config<'static>> {
    let mut parser = parser::Parser::new(config);
    let mut global_config = parser
        .next_section()?
        .map(|(_, kvs)| kvs)
        .map(GlobalConfig::from_kvs)
        .unwrap_or_else(|| Ok(Default::default()))?;

    let mut blocks = Config::default();
    let mut indexes = Indexes::default();
    crate::global_config::set(global_config.clone());
    while let Some((_, kvs)) = parser.next_section()? {
        let block = Block::from_kvs(
            NonZeroU8::new(
                bars.len()
                    .try_into()
                    .map_err(|_| ParseError::TooManyBarSpecs {
                        got: bars.len(),
                        max: u8::MAX,
                    })?,
            )
            .ok_or(ParseError::NeedAtLeastOneBarSpec)?,
            &mut indexes,
            kvs,
            broadcast,
            responses,
        )?;
        if let Layer::L(l) = block.layer {
            global_config.n_layers = u16::max(global_config.n_layers, l);
        }
        blocks[block.alignment].push(block);
    }
    global_config.n_layers += 1;
    global_config.tray = tray;
    global_config.bars_geometries = bars;
    crate::global_config::set(global_config);
    Ok(blocks)
}
