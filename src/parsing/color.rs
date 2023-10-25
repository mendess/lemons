use crate::model::color::Color;
use std::convert::TryFrom;

impl TryFrom<&str> for Color {
    type Error = &'static str;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if !s.starts_with('#') {
            return Err("Invalid colour");
        }

        fn p(s: &str) -> Result<u8, &'static str> {
            let n = u8::from_str_radix(s, 16).map_err(|_| "Invalid character in colour")?;

            Ok(n << if s.len() == 1 { 4 } else { 0 })
        }
        let s = &s[1..];
        match s.len() {
            3 => Ok(Color {
                r: p(&s[0..1])?,
                g: p(&s[1..2])?,
                b: p(&s[2..3])?,
                a: None,
            }),
            4 => Ok(Color {
                r: p(&s[0..1])?,
                g: p(&s[1..2])?,
                b: p(&s[2..3])?,
                a: Some(p(&s[3..4])?),
            }),
            6 => Ok(Color {
                r: p(&s[0..2])?,
                g: p(&s[2..4])?,
                b: p(&s[4..6])?,
                a: None,
            }),
            8 => Ok(Color {
                r: p(&s[0..2])?,
                g: p(&s[2..4])?,
                b: p(&s[4..6])?,
                a: Some(p(&s[6..8])?),
            }),
            _ => Err("Invalid color length"),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::model::Color;

    #[test]
    fn parse_short_color() {
        assert_eq!(
            Color::try_from("#123").unwrap(),
            Color {
                r: 0x10,
                g: 0x20,
                b: 0x30,
                a: None,
            }
        )
    }

    #[test]
    fn parse_short_color_with_alpha() {
        assert_eq!(
            Color::try_from("#123a").unwrap(),
            Color {
                r: 0x10,
                g: 0x20,
                b: 0x30,
                a: Some(0xa0),
            }
        )
    }

    #[test]
    fn parse_long_color() {
        assert_eq!(
            Color::try_from("#d1d2d3").unwrap(),
            Color {
                r: 0xd1,
                g: 0xd2,
                b: 0xd3,
                a: None,
            }
        )
    }

    #[test]
    fn parse_long_color_with_alpha() {
        assert_eq!(
            Color::try_from("#d1d2d3ab").unwrap(),
            Color {
                r: 0xd1,
                g: 0xd2,
                b: 0xd3,
                a: Some(0xab),
            }
        )
    }
}
