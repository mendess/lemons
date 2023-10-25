use crate::model::{Alignment, Color};
use std::fmt::{self, Display};

use super::Program;

pub struct DisplayColor(Color, Program);

impl DisplayColor {
    pub fn new(color: Color, program: Program) -> Self {
        Self(color, program)
    }

    pub fn lemonbar(color: Color) -> Self {
        Self(color, Program::Lemonbar)
    }
}

impl Display for DisplayColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Color { r, g, b, a } = self.0;
        match (self.1, a) {
            (Program::Lemonbar, Some(a)) => write!(f, "#{a:02X}{r:02X}{g:02X}{b:02X}"),
            (Program::Lemonbar, None) => write!(f, "#{r:02X}{g:02X}{b:02X}"),
        }
    }
}

impl Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Alignment::*;
        let s = match self {
            Left => "%{l}",
            Middle => "%{c}",
            Right => "%{r}",
        };
        f.write_str(s)
    }
}
