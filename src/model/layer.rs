#[derive(Debug, Clone, Copy)]
pub enum Layer {
    All,
    L(u16),
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::All, _) => true,
            (_, Self::All) => true,
            (Self::L(l1), Self::L(l2)) => l1 == l2,
        }
    }
}

impl PartialEq<u16> for Layer {
    fn eq(&self, other: &u16) -> bool {
        self == &Layer::L(*other)
    }
}

impl Layer {
    pub fn next(&mut self, bound: u16) {
        *self = match self {
            Self::L(n) => Self::L((*n + 1) % bound),
            Self::All => panic!("Can't next an all layer"),
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::All
    }
}
