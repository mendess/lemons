use crate::color::Color;
use arc_swap::ArcSwap;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};

pub static GLOBAL_CONFIG: Lazy<ArcSwap<GlobalConfig<'static>>> =
    Lazy::new(|| ArcSwap::from_pointee(Default::default()));

pub fn set(g: GlobalConfig<'static>) {
    GLOBAL_CONFIG.store(Arc::new(g));
}

pub fn get() -> Arc<GlobalConfig<'static>> {
    GLOBAL_CONFIG.load_full()
}

#[derive(Default)]
pub struct GlobalConfig<'a> {
    pub base_geometry: Option<&'a str>,
    pub bars_geometries: Vec<String>,
    pub bottom: bool,
    pub font: Option<&'a str>,
    pub n_clickbles: Option<u32>,
    pub name: Option<&'a str>,
    pub underline_width: Option<u32>,
    pub background: Option<Color<'a>>,
    pub foreground: Option<Color<'a>>,
    pub underline: Option<Color<'a>>,
    pub separator: Option<&'a str>,
    pub tray: bool,
    pub n_layers: u16,
    colors: HashMap<String, Color<'a>>,
}

impl<'a> GlobalConfig<'a> {
    pub fn to_arg_list(&self, extra_geomtery: Option<&str>) -> Vec<String> {
        let mut vector: Vec<String> = vec![];
        if let Some(g) = &self.base_geometry {
            vector.extend_from_slice(&[
                "-g".into(),
                extra_geomtery
                    .map(|e| merge_geometries(g, e))
                    .unwrap_or_else(|| g.to_string()),
            ]);
        }
        if self.bottom {
            vector.extend_from_slice(&["-b".into()]);
        }
        if let Some(f) = self.font {
            vector.extend_from_slice(&["-f".into(), f.into()]);
        }
        if let Some(n) = self.n_clickbles {
            vector.extend_from_slice(&["-a".into(), n.to_string()]);
        }
        if let Some(n) = self.name {
            vector.extend_from_slice(&["-n".into(), n.to_string()]);
        }
        if let Some(u) = self.underline_width {
            vector.extend_from_slice(&["-u".into(), u.to_string()]);
        }
        if let Some(bg) = &self.background {
            vector.extend_from_slice(&["-B".into(), bg.to_string()]);
        }
        if let Some(fg) = &self.foreground {
            vector.extend_from_slice(&["-F".into(), fg.to_string()]);
        }
        if let Some(un) = &self.underline {
            vector.extend_from_slice(&["-U".into(), un.to_string()]);
        }
        vector.extend_from_slice(&["-d".into()]);
        vector
    }

    pub fn get_color<'s>(&'s self, name: &str) -> Option<&'s Color<'a>> {
        let k = format!("LEMON_{}", name.to_uppercase());
        self.colors.get(&k)
    }

    pub fn set_color(&mut self, name: &str, value: Color<'a>) -> Option<Color<'a>> {
        self.colors
            .insert(format!("LEMON_{}", name.to_uppercase()), value.into())
    }

    pub fn colors<'s>(&'s self) -> impl Iterator<Item = (&'s str, &'s Color<'a>)> {
        self.colors.iter().map(|(s, c)| (s.as_str(), c))
    }
}

fn merge_geometries(geo1: &str, geo2: &str) -> String {
    if geo1.is_empty() {
        return geo2.into();
    }
    if geo2.is_empty() {
        return geo1.into();
    }
    let parse = |geo: &str| {
        let (geow, geo) = geo.split_at(geo.find('x').unwrap_or(0));
        let geo = geo.get(1..).unwrap_or("");
        let (geoh, geo) = geo.split_at(geo.find('+').unwrap_or(geo.len()));
        let (geox, geoy) = geo
            .get(1..)
            .and_then(|s| s.find('+').map(|i| s.split_at(i)))
            .unwrap_or(("", ""));
        (
            geow.parse::<i32>().unwrap_or(0),
            geoh.parse::<i32>().unwrap_or(0),
            geox.parse::<i32>().unwrap_or(0),
            geoy.parse::<i32>().unwrap_or(0),
        )
    };

    let (geo1w, geo1h, geo1x, geo1y) = parse(geo1);
    let (geo2w, geo2h, geo2x, geo2y) = parse(geo2);

    format!(
        "{}x{}+{}+{}",
        geo1w + geo2w,
        geo1h + geo2h,
        geo1x + geo2x,
        geo1y + geo2y
    )
}
