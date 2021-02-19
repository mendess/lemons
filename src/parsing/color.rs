use crate::color::Color;

impl<'a> Color<'a> {
    pub fn from_str(s: &'a str) -> Result<Self, &'static str> {
        if !s.starts_with('#') {
            return Err("Invalid colour");
        }
        if s[1..]
            .bytes()
            .all(|b| ((b'0'..=b'F').contains(&b) && b != b'@') || (b'a'..=b'f').contains(&b))
        {
            Ok(Color(s))
        } else {
            Err("Invalid character in colour")
        }
    }
}
