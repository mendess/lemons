mod block;
mod color;
mod global_config;
mod parser;

use crate::{block::Layer, Block, Config, GlobalConfig};

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
    MalformedBlock(&'static str),
}

pub type Result<'a, T> = std::result::Result<T, ParseError<'a>>;

const BLOCKS_INIT: Config = [Vec::new(), Vec::new(), Vec::new()];

pub fn parse(config: &'static str, bars: Vec<String>, tray: bool) -> Result<Config> {
    let mut parser = parser::Parser::new(config);
    let mut global_config = parser
        .next_section()?
        .map(|(_, kvs)| kvs)
        .map(GlobalConfig::from_kvs)
        .unwrap_or_else(|| Ok(Default::default()))?;

    let mut blocks = BLOCKS_INIT;
    while let Some(section) = parser.next_section()? {
        let block = Block::from_kvs(&global_config, section.1, bars.len())?;
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
