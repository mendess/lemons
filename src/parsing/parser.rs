use super::{ParseError, Result};
use core::fmt;
use itertools::{Itertools, PeekingNext};
use once_cell::sync::Lazy;
use regex::Regex;
use std::{iter::Peekable, str::Lines};

static BULLET: Lazy<Regex> = Lazy::new(|| Regex::new("^(  )*[-*]").unwrap());
static TITLE: Lazy<Regex> = Lazy::new(|| Regex::new("^#+").unwrap());

#[derive(Clone, Debug)]
pub struct Parser<'a> {
    s: Peekable<Lines<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            s: s.lines().peekable(),
        }
    }

    pub fn next_section(&mut self) -> Result<'a, Option<(Title<'a>, KeyValues<'a, '_>)>> {
        let lines = &mut self.s;
        skip_empty_lines(lines);
        let title = match lines.next() {
            Some(t) => t,
            None => return Ok(None),
        };
        let title = if let Some(m) = TITLE.find(title) {
            let level = m.end() as u8;
            Title {
                level,
                title: title[m.end()..].trim(),
            }
        } else {
            return Err(ParseError::ExpectedTitle(title));
        };
        skip_empty_lines(lines);
        Ok(Some((title, KeyValues { parser: Some(self) })))
    }
}

fn key_value(s: &str) -> Result<(&str, &str)> {
    let colon = s.find(':').ok_or(ParseError::Colon(s))?;
    let (key, value) = s.split_at(colon);
    Ok((key.trim(), value[1..].trim().trim_matches('`')))
}

fn skip_empty_lines<'a, I: PeekingNext<Item = &'a str>>(s: &mut I) {
    s.peeking_take_while(|s| s.is_empty()).for_each(|_| {});
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Title<'a> {
    pub level: u8,
    pub title: &'a str,
}

impl fmt::Display for Title<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Debug)]
pub struct KeyValues<'a, 'parser> {
    parser: Option<&'parser mut Parser<'a>>,
}

impl<'a, 'parser> Iterator for KeyValues<'a, 'parser> {
    type Item = Result<'a, (&'a str, &'a str, u8)>;
    fn next(&mut self) -> Option<Self::Item> {
        self.parser
            .as_mut()?
            .s
            .next()
            .filter(|a| !a.trim().is_empty())
            .map(|attr| {
                if let Some(m) = BULLET.find(attr) {
                    let (k, v) = key_value(attr[m.end()..].trim())?;
                    let level = (m.end() / 2) + 1;
                    Ok((k, v, level as u8))
                } else {
                    return Err(ParseError::ExpectedAttribute(attr));
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple() {
        let md = r#"
# Bar
- bg: `#aa222222`
        "#;
        let mut p = Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("bg", "#aa222222", 1)));
    }

    #[test]
    fn two_blocks() {
        let md = r#"
# Bar
- bg: `#aa222222`

## Workspaces
- cmd: `echo ola`
        "#;
        let mut p = super::Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("bg", "#aa222222", 1)));
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is a second section");
        assert_eq!(
            title,
            Title {
                title: "Workspaces",
                level: 2
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("cmd", "echo ola", 1)));
    }

    #[test]
    fn nested_bullets() {
        let md = r#"
# Bar
- bg: `#aa222222`

## Workspaces
- cmd: `echo ola`
  - nested: `bullet`
        "#;
        let mut p = super::Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("bg", "#aa222222", 1)));
        let (title, kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Workspaces",
                level: 2
            }
        );
        assert_eq!(
            kvs.collect::<Result<'static, Vec<_>>>().unwrap(),
            vec![("cmd", "echo ola", 1), ("nested", "bullet", 2)]
        );
    }

    #[test]
    fn trailing_ws() {
        let md = r#"
# Bar
- bg: `#aa222222`


        "#;

        let mut p = super::Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("bg", "#aa222222", 1)));
    }

    #[test]
    fn leading_ws() {
        let md = r#"



# Bar
- bg: `#aa222222`"#;

        let mut p = super::Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("bg", "#aa222222", 1)));
    }

    #[test]
    fn three_blocks() {
        let md = r#"
# Bar
- a: b

## W
- c: d


## D
- e: f
        "#;
        let mut p = super::Parser::new(md);
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "Bar",
                level: 1
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("a", "b", 1)));
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "W",
                level: 2
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("c", "d", 1)));
        let (title, mut kvs) = p
            .next_section()
            .expect("No parse error")
            .expect("There is one section");
        assert_eq!(
            title,
            Title {
                title: "D",
                level: 2
            }
        );
        assert_eq!(kvs.next().map(Result::unwrap), Some(("e", "f", 1)));
    }
}
