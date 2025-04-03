mod block;
mod color;
mod global_config;
pub mod parser;

use std::num::NonZeroU8;

use crate::{
    Config,
    global_config::{FileConfig, GlobalConfig},
    model::{ActivationLayer, block::Block},
};

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

pub fn parse(config: &'static str, overrides: GlobalConfig) -> Result<'static, Config<'static>> {
    let mut parser = parser::Parser::new(config);
    let file_config = parser
        .next_section()?
        .map(|(_, kvs)| kvs)
        .map(FileConfig::from_kvs)
        .unwrap_or_else(|| Ok(Default::default()))?;
    let mut global_config = GlobalConfig::new(file_config, overrides);

    let mut blocks = Config::default();
    crate::global_config::set(global_config.clone());
    let bar_spec_count = global_config
        .cmdline
        .outputs
        .len()
        .try_into()
        .map_err(|_| ParseError::TooManyBarSpecs {
            got: global_config.cmdline.outputs.len(),
            max: u8::MAX,
        })
        .map(NonZeroU8::new)?
        .unwrap_or_else(|| NonZeroU8::new(1).unwrap());
    while let Some((title, kvs)) = parser.next_section()? {
        let block = Block::from_kvs(
            title,
            bar_spec_count,
            // &mut indexes,
            kvs,
            // broadcast,
            // responses,
        )?;
        if let ActivationLayer::L(l) = block.layer {
            global_config.n_layers = u16::max(global_config.n_layers, l);
        }
        blocks[block.alignment].push(block);
    }
    global_config.n_layers += 1;
    log::debug!("global config loaded: {global_config:?}");
    crate::global_config::set(global_config);
    Ok(blocks)
}
