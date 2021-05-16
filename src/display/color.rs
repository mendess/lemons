use crate::model::Color;
use std::fmt::{self, Display};

impl<'a> Display for Color<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.0)
    }
}
