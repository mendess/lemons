mod block;
mod color;
mod global_config;

use crate::{block::Layer, Block, Config, GlobalConfig};
use std::convert::TryFrom;

pub type ParseError<'a> = (&'a str, &'a str);

pub fn parse_key_value(s: &str) -> Result<(&str, &str), ParseError<'_>> {
    let (a, b) = s.split_at(s.find(':').ok_or((s, "missing :"))?);
    Ok((a, b[1..].trim().trim_matches('\'')))
}

pub fn parse(config: &'static str, bars: Vec<String>, tray: bool) -> Result<Config, ParseError> {
    let mut blocks = Config::with_capacity(3);
    let mut blocks_iter = config.split("\n>");
    let mut global_config = blocks_iter
        .next()
        .map(GlobalConfig::try_from)
        .unwrap_or_else(|| Ok(Default::default()))?;
    for block in blocks_iter {
        let b = Block::parse(block, bars.len(), &global_config.colors)?;
        if let Layer::L(n) = b.layer {
            global_config.n_layers = global_config.n_layers.max(n);
        }
        blocks.entry(b.alignment).or_default().push(b);
    }
    global_config.n_layers += 1;
    global_config.tray = tray;
    global_config.bars_geometries = bars;
    crate::global_config::set(global_config);
    Ok(blocks)
}
