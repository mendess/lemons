use crate::display::implementations::DisplayColor;

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
