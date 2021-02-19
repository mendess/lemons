mod block;
mod color;
pub use block::{DisplayBlock, DisplayContent};

use std::fmt::{self, Display, Write};

pub trait Lemonbar: Write {
    fn lemon<P, S>(&mut self, prefix: P, s: S) -> fmt::Result
    where
        P: Display,
        S: Display,
    {
        write!(self, "%{{{}{}}}", prefix, s)
    }
}

impl<'a> Lemonbar for fmt::Formatter<'a> {}
impl<'a> Lemonbar for String {}
