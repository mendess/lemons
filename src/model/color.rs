#[derive(Clone, Copy, Debug)]
pub struct Color<'a>(pub &'a str);

impl Default for Color<'static> {
    fn default() -> Self {
        Color("-")
    }
}

impl<'a> Color<'a> {
    pub fn has_transparency(&self) -> bool {
        self.0.len() == "#aaFFFFFF".len()
    }

    pub fn tint(&self) -> &str {
        let start = self.has_transparency() as usize * 2 + 1;
        &self.0[start..]
    }

    pub fn transparency(&self) -> Option<u8> {
        self.has_transparency()
            .then(|| u8::from_str_radix(&self.0[1..3], 16).unwrap())
    }
}
