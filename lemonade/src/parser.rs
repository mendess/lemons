#![allow(unstable_name_collisions)] // split_at_checked

trait StrExt {
    fn split_at_checked(self, at: usize) -> Option<(Self, Self)>
    where
        Self: Sized;
}

impl<'s> StrExt for &'s str {
    fn split_at_checked(self, at: usize) -> Option<(Self, Self)> {
        Some((self.get(0..at)?, self.get(at..)?))
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: Option<u8>,
}

impl Color {
    pub fn parse(s: &str) -> Option<(Self, &str)> {
        fn p(s: &str) -> Option<(u8, u8, u8, Option<u8>, &str)> {
            let (h0, s) = s.split_at_checked(2)?;
            let h0 = u8::from_str_radix(h0, 16).ok()?;
            let (h1, s) = s.split_at_checked(2)?;
            let h1 = u8::from_str_radix(h1, 16).ok()?;
            let (h2, s) = s.split_at_checked(2)?;
            let h2 = u8::from_str_radix(h2, 16).ok()?;
            if let Some((h3, s)) = s.split_at_checked(2) {
                if let Ok(h3) = u8::from_str_radix(h3, 16) {
                    return Some((h0, h1, h2, Some(h3), s));
                }
            }
            Some((h0, h1, h2, None, s))
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

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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

    let (action, s) = s.split_at_checked(s.find('}')?)?;

    let s = s.strip_prefix('}')?;

    Some((idx, action, s))
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

impl<'s> Block<'s> {
    fn parse(mut s: &'s str, default_alignment: Alignment) -> Option<(Self, &'s str)> {
        let mut block = Block {
            alignment: default_alignment,
            ..Default::default()
        };
        #[cfg(any(test, fuzzing))]
        let mut i = 0;
        #[cfg(any(test, fuzzing))]
        let original_str = s;
        loop {
            #[cfg(any(test, fuzzing))]
            {
                assert_ne!(
                    i, 4096,
                    "infinite loop detected.
input was {original_str:?}
looping body was {:?}",
                    s
                );
                i += 1;
            }
            if let Some((attr, rest)) = Attribute::parse(s) {
                s = rest;
                match attr {
                    Attribute::Alignment(al) => block.alignment = al,
                    Attribute::Fg(fg) => block.fg = Some(fg),
                    Attribute::Bg(bg) => block.bg = Some(bg),
                    Attribute::Ul(ul) => block.ul = Some(ul),
                    Attribute::Action { index, action } => block.actions[index - 1] = Some(action),
                }
            } else {
                (block.t, s) = (0..s.len())
                    .filter_map(|mid| Some((s.get(0..mid)?, s.get(mid..)?)))
                    .find(|(_, rest)| Attribute::parse(rest).is_some())
                    .unwrap_or((s, ""));
                break;
            }
        }
        Some((block, s))
    }
}

enum Attribute<'s> {
    Alignment(Alignment),
    Fg(Color),
    Bg(Color),
    Ul(Color),
    Action { index: usize, action: &'s str },
}

impl<'s> Attribute<'s> {
    fn parse(s: &'s str) -> Option<(Self, &'s str)> {
        if let Some((al, rest)) = Alignment::parse(s) {
            Some((Self::Alignment(al), rest))
        } else if let Some((fg, rest)) = parse_color_attr(s, "F") {
            Some((Self::Fg(fg), rest))
        } else if let Some((bg, rest)) = parse_color_attr(s, "B") {
            Some((Self::Bg(bg), rest))
        } else if let Some((ul, rest)) = parse_color_attr(s, "U") {
            Some((Self::Ul(ul), rest))
        } else if let Some((index, action, rest)) = parse_action(s) {
            Some((Self::Action { index, action }, rest))
        } else {
            None
        }
    }
}

struct Parser<'s> {
    last_alignment: Alignment,
    s: &'s str,
    #[cfg(any(test, fuzzing))]
    elements_yielded: u16,
}

#[derive(Debug, Default)]
pub struct ParserError;

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ParserError {}

impl<'s> Iterator for Parser<'s> {
    type Item = Result<Block<'s>, ParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        (!self.s.is_empty()).then(|| {
            let (block, s) = Block::parse(self.s, self.last_alignment).ok_or(ParserError)?;
            #[cfg(any(test, fuzzing))]
            {
                match self.elements_yielded.checked_add(1) {
                    Some(i) => self.elements_yielded = i,
                    None => panic!("infinite iterator detected. Input str was {:?}", self.s),
                }
            }
            self.s = s;
            self.last_alignment = block.alignment;
            Ok(block)
        })
    }
}

pub fn parse(s: &str) -> impl Iterator<Item = Result<Block<'_>, ParserError>> {
    Parser {
        last_alignment: Default::default(),
        s,
        #[cfg(any(test, fuzzing))]
        elements_yielded: 0,
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
    fn permissive_color() {
        let (c0, _) = parse_color_attr("%{F:0xaabbcc55}", "F").unwrap();
        let (c1, _) = parse_color_attr("%{F0xaabbcc55}", "F").unwrap();
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

    #[test]
    fn alignment_conservation() {
        let [left0, left1, right] =
            parse_to_array("%{l}%{F:0xFffFFf}left%{U:0xdeadbeef}also left%{r}right");
        assert_eq!(
            left0,
            Block {
                alignment: Alignment::Left,
                t: "left",
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
            left1,
            Block {
                alignment: Alignment::Left,
                t: "also left",
                ul: Some(Color {
                    r: 0xde,
                    g: 0xad,
                    b: 0xbe,
                    a: Some(0xef),
                }),
                ..Default::default()
            }
        );
        assert_eq!(
            right,
            Block {
                alignment: Alignment::Right,
                t: "right",
                ..Default::default()
            }
        );
    }

    #[test]
    fn gibberish_text_with_percents() {
        let [b] = parse_to_array("%{F:0xFffFFf}The t% ext %{F}");
        assert_eq!(
            b,
            Block {
                t: "The t% ext %{F}",
                fg: Some(Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                    a: None
                }),
                ..Default::default()
            }
        );
    }

    #[test]
    fn multiple_actions() {
        let [b] = parse_to_array("%{U:0xE3B97D}%{A1:1-0-0-1}%{A2:1-0-0-2}%{A3:1-0-0-3}%{A4:1-0-0-4}%{A5:1-0-0-5}[0] music name > 84%");
        assert_eq!(
            b,
            Block {
                t: "[0] music name > 84%",
                ul: Some(Color {
                    r: 0xe3,
                    g: 0xb9,
                    b: 0x7d,
                    a: None,
                }),
                actions: [
                    Some("1-0-0-1"),
                    Some("1-0-0-2"),
                    Some("1-0-0-3"),
                    Some("1-0-0-4"),
                    Some("1-0-0-5"),
                ],
                ..Default::default()
            }
        )
    }

    #[test]
    fn fuzzed_input() {
        parse(r#"k{c%{"#).for_each(|_| {});
        parse(r#"%{\u{5}"#).for_each(|_| {});
        parse(r#"%{"#).for_each(|_| {});
        parse(r#"%{c}"#).for_each(|_| {});
        parse(r#"%%{l}"#).for_each(|_| {});
        parse(r#"%`%{F0x}"#).for_each(|_| {});
        parse(r#"L}{%{U0x"#).for_each(|_| {});
        parse(r#"%%%{A5:5{A555{5{A%{ll}6""#).for_each(|_| {});
    }
}
