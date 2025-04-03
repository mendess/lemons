use super::{AffectedMonitor, Color};
use crate::{
    display::{Bar, CmdlineArgBuilder, Program},
    util::number_as_str,
};
use arc_swap::ArcSwap;
use clap::Args;
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
    iter::once,
    sync::Arc,
};

pub static GLOBAL_CONFIG: Lazy<ArcSwap<GlobalConfig>> =
    Lazy::new(|| ArcSwap::from_pointee(Default::default()));

pub fn set(g: GlobalConfig) {
    GLOBAL_CONFIG.store(Arc::new(g));
}

pub fn get() -> Arc<GlobalConfig> {
    GLOBAL_CONFIG.load_full()
}

#[derive(Default, Clone, Debug, Args)]
pub struct GlobalConfig {
    #[command(flatten)]
    pub file_config: FileConfig,
    // mandatory arguments
    #[command(flatten)]
    pub cmdline: CommandLineConfig,
    // can't be overriden
    #[arg(skip)]
    pub n_layers: u16,
}

#[derive(Default, Clone, Debug, Args)]
pub struct FileConfig {
    #[arg(long, alias = "height-override")]
    pub height: Option<u32>,
    #[arg(long)]
    pub bottom: bool,
    #[arg(long("font"))]
    pub fonts: Vec<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub underline_width: Option<u32>,
    #[arg(long)]
    pub background: Option<Color>,
    #[arg(long)]
    pub foreground: Option<Color>,
    #[arg(long)]
    pub underline: Option<Color>,
    #[arg(long)]
    pub separator: Option<String>,
    // hard to pass arguments
    #[arg(skip)]
    colors: HashMap<String, (String, Color)>,
}

#[derive(Default, Clone, Debug, Args)]
pub struct CommandLineConfig {
    /// The parameters to pass to each bar
    #[arg(short, long("output"))]
    pub outputs: Vec<String>,
    #[arg(long)]
    pub tray: bool,
    #[arg(short, long, default_value = "lemonbar")]
    pub program: Program,
}

impl GlobalConfig {
    pub fn new(file_config: FileConfig, overrides: Self) -> Self {
        Self {
            file_config: FileConfig {
                height: overrides.file_config.height.or(file_config.height),
                bottom: overrides.file_config.bottom || (file_config.bottom),
                fonts: if overrides.file_config.fonts.is_empty() {
                    file_config.fonts
                } else {
                    overrides.file_config.fonts
                },
                name: overrides.file_config.name.or(file_config.name),
                underline_width: overrides
                    .file_config
                    .underline_width
                    .or(file_config.underline_width),
                background: overrides.file_config.background.or(file_config.background),
                foreground: overrides.file_config.foreground.or(file_config.foreground),
                underline: overrides.file_config.underline.or(file_config.underline),
                separator: overrides.file_config.separator.or(file_config.separator),
                colors: file_config.colors,
            },
            cmdline: overrides.cmdline,
            n_layers: 0,
        }
    }

    pub fn to_arg_list<W, B>(&self, output: Option<&str>) -> Vec<String>
    where
        B: Bar<W>,
        W: std::fmt::Write,
    {
        let mut arg_builder = B::cmdline_builder();
        if let Some(h) = self.file_config.height {
            arg_builder.height(h)
        }
        if let Some(o) = output {
            arg_builder.output(o)
        }
        if self.file_config.bottom {
            arg_builder.bottom();
        }
        arg_builder.fonts(self.file_config.fonts.iter().map(|s| s.as_str()));
        if let Some(n) = &self.file_config.name {
            arg_builder.name(n);
        }
        if let Some(u) = self.file_config.underline_width {
            arg_builder.underline_width(u);
        }
        if let Some(bg) = &self.file_config.background {
            arg_builder.background(bg)
        }
        if let Some(fg) = &self.file_config.foreground {
            arg_builder.foreground(fg)
        }
        if let Some(un) = &self.file_config.underline {
            arg_builder.underline_color(un)
        }
        arg_builder.finish()
    }

    pub fn get_color<'s>(&'s self, name: &str) -> Option<&'s Color> {
        self.file_config.get_color(name)
    }

    pub fn as_env_vars(
        &self,
        monitor: AffectedMonitor,
        layer: u16,
    ) -> impl Iterator<Item = (&str, Cow<'_, OsStr>)> {
        let color = |c: &Option<Color>| {
            c.map(|c| Cow::Owned(OsString::from(c.to_code())))
                .unwrap_or(Cow::Borrowed(OsStr::new("")))
        };
        once(("LEMON_BG", color(&self.file_config.background)))
            .chain(once(("LEMON_FG", color(&self.file_config.foreground))))
            .chain(once(("LEMON_UN", color(&self.file_config.underline))))
            .chain(once((
                "LEMON_MONITOR",
                Cow::Borrowed(
                    match monitor {
                        AffectedMonitor::All => "all",
                        AffectedMonitor::Single(n) => number_as_str(n),
                    }
                    .as_ref(),
                ),
            )))
            .chain(once((
                "LEMON_LAYER",
                Cow::Borrowed(number_as_str(layer as u8).as_ref()),
            )))
            .chain(
                self.file_config
                    .colors
                    .iter()
                    .map(|(_, (k, v))| (k.as_str(), Cow::Owned(v.to_code().into()))),
            )
            .chain(once((
                "LEMON_PROGRAM",
                Cow::Owned(self.cmdline.program.as_str().into()),
            )))
    }
}

impl FileConfig {
    pub fn get_color<'s>(&'s self, name: &str) -> Option<&'s Color> {
        self.colors.get(name).map(|x| &x.1)
    }

    pub fn set_color(&mut self, name: &str, value: Color) -> Option<Color> {
        let env_var = format!("LEMON_{}", name.to_uppercase());
        self.colors
            .insert(name.to_string(), (env_var, value))
            .map(|x| x.1)
    }
}
