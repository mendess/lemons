use crate::model::{Alignment, Color};
use std::fmt::{self, Display};

impl<'a> Display for Color<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.0)
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
