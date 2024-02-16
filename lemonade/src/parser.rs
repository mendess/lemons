#![allow(dead_code)]

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: Option<u8>,
}

impl Color {
    fn parse(s: &str) -> Option<(Self, &str)> {
        fn p(s: &str) -> Option<(u8, u8, u8, Option<u8>, &str)> {
            let (h0, rest) = s.split_at(2);
            let h0 = u8::from_str_radix(h0, 16).ok()?;
            let (h1, rest) = rest.split_at(2);
            let h1 = u8::from_str_radix(h1, 16).ok()?;
            let (h2, rest) = rest.split_at(2);
            let h2 = u8::from_str_radix(h2, 16).ok()?;
            if rest.len() > 1 {
                let (h3, rest_) = rest.split_at(2);
                if let Ok(h3) = u8::from_str_radix(h3, 16) {
                    return Some((h0, h1, h2, Some(h3), rest_));
                }
            }
            Some((h0, h1, h2, None, rest))
        }
        if let Some(s) = s.strip_prefix('#') {
            // lemonbar
            let (h0, h1, h2, h3, s) = p(s)?;
            let c = if let Some(h3) = h3 {
                Color {
                    a: Some(h0),
                    r: h1,
                    g: h2,
                    b: h3,
                }
            } else {
                Color {
                    a: None,
                    r: h0,
                    g: h1,
                    b: h2,
                }
            };

            Some((c, s))
        } else if let Some(s) = s.strip_prefix("0x") {
            // zelbar
            let (r, g, b, a, s) = p(s)?;
            Some((Color { r, g, b, a }, s))
        } else {
            return None;
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

impl Alignment {
    fn parse(s: &str) -> Option<(Self, &str)> {
        let al = match s.get(0..4)? {
            "%{l}" => Self::Left,
            "%{c}" => Self::Center,
            "%{r}" => Self::Right,
            _ => return None,
        };
        Some((al, s.get(4..)?))
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Block<'s> {
    pub alignment: Alignment,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub ul: Option<Color>,
    pub actions: [Option<&'s str>; 5],
    pub t: &'s str,
}

struct Parser<'s> {
    last_alignment: Alignment,
    s: &'s str,
}

#[derive(Debug, Default)]
pub struct ParserError;

fn parse_color_attr<'s>(s: &'s str, attr: &'static str) -> Option<(Color, &'s str)> {
    let s = s.strip_prefix("%{")?;
    let s = s.strip_prefix(attr)?;
    let s = s.strip_prefix(':').unwrap_or(s);
    let (c, s) = Color::parse(s)?;
    let s = s.strip_prefix('}')?;
    Some((c, s))
}

fn parse_action(s: &str) -> Option<(usize, &str, &str)> {
    let s = s.strip_prefix("%{A")?;

    let (idx, s) = {
        let idx = s
            .get(0..1)?
            .parse()
            .ok()
            .filter(|idx| (1..=5).contains(idx))?;

        (idx, s.get(1..).unwrap_or_default())
    };

    let s = s.strip_prefix(':')?;

    let (action, s) = s.split_at(s.find('}')?);

    Some((idx, action, s))
}

impl<'s> Iterator for Parser<'s> {
    type Item = Result<Block<'s>, ParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.s.is_empty() {
            return None;
        }
        let mut block = Block::default();
        while block.t.is_empty() {
            let current_len = self.s.len();
            if let Some((al, rest)) = Alignment::parse(self.s) {
                self.s = rest;
                block.alignment = al;
            }
            if let Some((fg, rest)) = parse_color_attr(self.s, "F") {
                self.s = rest;
                block.fg = Some(fg);
            }
            if let Some((bg, rest)) = parse_color_attr(self.s, "B") {
                self.s = rest;
                block.bg = Some(bg);
            }
            if let Some((ul, rest)) = parse_color_attr(self.s, "U") {
                self.s = rest;
                block.ul = Some(ul);
            }
            if let Some((idx, action, rest)) = parse_action(self.s) {
                self.s = rest;
                block.actions[idx] = Some(action);
            }
            if self.s.len() == current_len {
                let end = self.s.find("%{").unwrap_or(self.s.len());
                (block.t, self.s) = self.s.split_at(end);
            }
        }
        Some(Ok(block))
    }
}

pub fn parse(s: &str) -> impl Iterator<Item = Result<Block<'_>, ParserError>> {
    Parser {
        last_alignment: Default::default(),
        s,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn opaque_colors() {
        let (c0, _) = Color::parse("#aabbcc").unwrap();
        let (c1, _) = Color::parse("0xaabbcc").unwrap();
        let expect = Color {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
            a: None,
        };

        assert_eq!(c0, c1);
        assert_eq!(c0, expect);
        assert_eq!(c1, expect);
    }

    #[test]
    fn transparent_colors() {
        let (c0, _) = Color::parse("#55aabbcc").unwrap();
        let (c1, _) = Color::parse("0xaabbcc55").unwrap();
        let expect = Color {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
            a: Some(0x55),
        };

        assert_eq!(c0, c1);
        assert_eq!(c0, expect);
        assert_eq!(c1, expect);
    }

    fn parse_to_array<const N: usize>(s: &str) -> [Block; N] {
        let a: [_; N] = parse(s).collect::<Vec<_>>().try_into().unwrap();
        a.map(|b| b.unwrap())
    }

    #[test]
    fn plain_block() {
        let [b] = parse_to_array("Text");
        assert_eq!(
            b,
            Block {
                t: "Text",
                ..Default::default()
            }
        );
    }

    #[test]
    fn fg_block() {
        let [b] = parse_to_array("%{F:0xaabbcc}Text");
        assert_eq!(
            b,
            Block {
                t: "Text",
                fg: Some(Color {
                    r: 0xaa,
                    g: 0xbb,
                    b: 0xcc,
                    a: None
                }),
                ..Default::default()
            }
        )
    }

    #[test]
    fn two_fg_blocks() {
        let [b0, b1] = parse_to_array("%{F:0xffffff}Black%{F:0x000000}White");
        assert_eq!(
            b0,
            Block {
                t: "Black",
                fg: Some(Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                    a: None
                }),
                ..Default::default()
            }
        );
        assert_eq!(
            b1,
            Block {
                t: "White",
                fg: Some(Color {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00,
                    a: None
                }),
                ..Default::default()
            }
        )
    }
}
