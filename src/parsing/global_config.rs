use super::{ParseError, Result, parser::KeyValues};
use crate::global_config::GlobalConfig;
use std::convert::TryInto;

impl<'a> GlobalConfig<'a> {
    pub fn from_kvs(iter: KeyValues<'a, '_>) -> Result<'a, Self> {
        let mut global_config = GlobalConfig::default();
        let mut in_colors = false;
        for kvl in iter {
            let (key, value, level) = kvl?;
            in_colors = in_colors && level > 1;
            log::trace!(
                "{}{}: {}",
                " ".repeat(level.saturating_sub(1) as _),
                key,
                value
            );
            let color = || {
                value
                    .try_into()
                    .map_err(|error| ParseError::Color { value, error })
            };
            match key {
                "background" | "bg" | "B" => global_config.background = Some(color()?),
                "foreground" | "fg" | "F" => global_config.foreground = Some(color()?),
                "underline" | "un" | "U" => global_config.underline = Some(color()?),
                "font" | "f" => global_config.fonts.push(value),
                "bottom" | "b" => {
                    global_config.bottom = value
                        .trim()
                        .parse()
                        .map_err(|_| ParseError::InvalidBoolean(value))?
                }
                "underline_width" | "u" => {
                    global_config.underline_width = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| ParseError::InvalidInteger(value))?,
                    )
                }
                "separator" => global_config.separator = Some(value),
                "height" | "h" => {
                    global_config.height = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| ParseError::InvalidInteger(value))?,
                    )
                }
                "name" | "n" => global_config.name = Some(value),
                "colors" | "colours" | "c" => in_colors = true,
                key if level == 2 && in_colors => {
                    global_config.set_color(key, color()?);
                }
                s => {
                    log::warn!("Warning: unrecognised option '{}', skipping", s);
                }
            };
        }
        Ok(global_config)
    }
}
