use enum_iterator::IntoEnumIterator;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, IntoEnumIterator)]
pub enum Alignment {
    Left = 0,
    Middle = 1,
    Right = 2,
}

impl From<u8> for Alignment {
    fn from(x: u8) -> Self {
        use Alignment::*;
        match x {
            0 => Left,
            1 => Middle,
            2 => Right,
            _ => unreachable!(),
        }
    }
}
