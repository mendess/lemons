use super::{parse_key_value, ParseError};
use crate::{Color, GlobalConfig};
use std::convert::TryFrom;

impl<'a> TryFrom<&'a str> for GlobalConfig<'a> {
    type Error = ParseError<'a>;
    fn try_from(globals: &'a str) -> Result<Self, Self::Error> {
        let mut global_config = Self::default();
        let mut opts = globals.split('\n').filter(|s| !s.trim().is_empty());
        while let Some(opt) = opts.next() {
            let (key, value) = parse_key_value(opt)?;
            eprintln!("{}: {}", key, value);
            let color = || Color::from_str(value).map_err(|e| (opt, e));
            match key
                .trim()
                .trim_start_matches('*')
                .trim_start_matches('-')
                .trim()
            {
                "background" | "bg" | "B" => global_config.background = Some(color()?),
                "foreground" | "fg" | "F" => global_config.foreground = Some(color()?),
                "underline" | "un" | "U" => global_config.underline = Some(color()?),
                "font" | "f" => global_config.font = Some(value),
                "bottom" | "b" => {
                    global_config.bottom = value
                        .trim()
                        .parse()
                        .map_err(|_| (opt, "Not a valid boolean"))?
                }
                "n_clickables" | "a" => {
                    global_config.n_clickbles = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| (opt, "Not a valid number"))?,
                    )
                }
                "underline_width" | "u" => {
                    global_config.underline_width = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| (opt, "Not a valid number"))?,
                    )
                }
                "separator" => global_config.separator = Some(value),
                "geometry" | "g" => global_config.base_geometry = Some(value.into()),
                "name" | "n" => global_config.name = Some(value),
                "colors" | "colours" | "c" if value.trim() == "{" => {
                    let mut failed = true;
                    while let Some(color) = opts.next().map(str::trim) {
                        if color == "}" {
                            failed = false;
                            break;
                        }
                        let (key, value) = parse_key_value(color)?;
                        eprintln!("{}: {}", key, value);
                        global_config
                            .colors
                            .insert(key, Color::from_str(value).map_err(|e| (value, e))?);
                    }
                    if failed {
                        return Err(("", "expected a '}' at the end of the colors definition"));
                    }
                }
                s => {
                    eprintln!("Warning: unrecognised option '{}', skipping", s);
                }
            }
        }
        Ok(global_config)
    }
}
