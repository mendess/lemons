use super::{AffectedMonitor, Color};
use crate::{
    display::{Bar, CmdlineArgBuilder, Program},
    util::number_as_str,
};
use arc_swap::ArcSwap;
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
    iter::once,
    sync::Arc,
};

pub static GLOBAL_CONFIG: Lazy<ArcSwap<GlobalConfig<'static>>> =
    Lazy::new(|| ArcSwap::from_pointee(Default::default()));

pub fn set(g: GlobalConfig<'static>) {
    GLOBAL_CONFIG.store(Arc::new(g));
}

pub fn get() -> Arc<GlobalConfig<'static>> {
    GLOBAL_CONFIG.load_full()
}

#[derive(Default, Clone, Debug)]
pub struct GlobalConfig<'a> {
    pub height: Option<u32>,
    pub outputs: Vec<String>,
    pub bottom: bool,
    pub fonts: Vec<&'a str>,
    pub name: Option<&'a str>,
    pub underline_width: Option<u32>,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub underline: Option<Color>,
    pub separator: Option<&'a str>,
    pub tray: bool,
    pub n_layers: u16,
    colors: HashMap<&'a str, (String, Color)>,
    pub program: Program,
}

impl<'a> GlobalConfig<'a> {
    pub fn to_arg_list<W, B>(&self, output: Option<&str>) -> Vec<String>
    where
        B: Bar<W>,
        W: std::fmt::Write,
    {
        let mut arg_builder = B::cmdline_builder();
        if let Some(h) = self.height {
            arg_builder.height(h)
        }
        if let Some(o) = output {
            arg_builder.output(o)
        }
        if self.bottom {
            arg_builder.bottom();
        }
        arg_builder.fonts(self.fonts.iter().copied());
        if let Some(n) = self.name {
            arg_builder.name(n);
        }
        if let Some(u) = self.underline_width {
            arg_builder.underline_width(u);
        }
        if let Some(bg) = &self.background {
            arg_builder.background(bg)
        }
        if let Some(fg) = &self.foreground {
            arg_builder.foreground(fg)
        }
        if let Some(un) = &self.underline {
            arg_builder.underline_color(un)
        }
        arg_builder.finish()
    }

    pub fn get_color<'s>(&'s self, name: &str) -> Option<&'s Color> {
        self.colors.get(name).map(|x| &x.1)
    }

    pub fn set_color(&mut self, name: &'a str, value: Color) -> Option<Color> {
        let env_var = format!("LEMON_{}", name.to_uppercase());
        self.colors.insert(name, (env_var, value)).map(|x| x.1)
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
        once(("LEMON_BG", color(&self.background)))
            .chain(once(("LEMON_FG", color(&self.foreground))))
            .chain(once(("LEMON_UN", color(&self.underline))))
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
                self.colors
                    .iter()
                    .map(|(_, (k, v))| (k.as_str(), Cow::Owned(v.to_code().into()))),
            )
            .chain(once((
                "LEMON_PROGRAM",
                Cow::Owned(self.program.as_str().into()),
            )))
    }
}
