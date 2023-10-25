use crate::display::implementations::DisplayColor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: Option<u8>,
}

impl Color {
    pub fn has_transparency(&self) -> bool {
        self.a.is_some()
    }

    pub fn to_code(&self) -> String {
        DisplayColor::lemonbar(*self).to_string()
    }
}
