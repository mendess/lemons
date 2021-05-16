use crate::model::color::Color;
use std::convert::TryFrom;

impl<'a> TryFrom<&'a str> for Color<'a> {
    type Error = &'static str;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
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
