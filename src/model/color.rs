use crate::display::implementations::DisplayColor;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: Option<u8>,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0x18,
        g: 0x18,
        b: 0x18,
        a: None,
    };
    pub const RED: Color = Color {
        r: 0xf0,
        g: 0x71,
        b: 0x78,
        a: None,
    };
    pub const GREEN: Color = Color {
        r: 0xa1,
        g: 0xb5,
        b: 0x6c,
        a: None,
    };
    pub const YELLOW: Color = Color {
        r: 0xe3,
        g: 0xb9,
        b: 0x7d,
        a: None,
    };
    pub const BLUE: Color = Color {
        r: 0x7c,
        g: 0xaf,
        b: 0xc2,
        a: None,
    };
    pub const MAGENTA: Color = Color {
        r: 0xba,
        g: 0x8b,
        b: 0xaf,
        a: None,
    };
    pub const CYAN: Color = Color {
        r: 0x86,
        g: 0xc1,
        b: 0xb9,
        a: None,
    };
    pub const WHITE: Color = Color {
        r: 0xd8,
        g: 0xd8,
        b: 0xd8,
        a: None,
    };

    pub fn has_transparency(&self) -> bool {
        self.a.is_some()
    }

    pub fn to_code(&self) -> String {
        DisplayColor::lemonbar(*self).to_string()
    }

    pub fn to_hex(&self) -> String {
        DisplayColor::zelbar(*self).to_string()
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start_matches('#');
        if s.len() < 6 {
            Err(format!("{s:?} too short"))
        } else {
            let (r, g, b) = (&s[0..2], &s[2..4], &s[4..6]);
            Ok(Self {
                r: u8::from_str_radix(r, 16).map_err(|e| e.to_string())?,
                g: u8::from_str_radix(g, 16).map_err(|e| e.to_string())?,
                b: u8::from_str_radix(b, 16).map_err(|e| e.to_string())?,
                a: s.get(6..8)
                    .map(|a| u8::from_str_radix(a, 16).map_err(|e| e.to_string()))
                    .transpose()?,
            })
        }
    }
}
