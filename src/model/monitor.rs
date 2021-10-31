use crate::util::one_or_more::OneOrMore;

#[derive(Debug, Clone, Copy)]
pub enum ActiveMonitors {
    All,
    M(u8),
}

impl Default for ActiveMonitors {
    fn default() -> Self {
        Self::All
    }
}

impl ActiveMonitors {
    pub fn resize_one_or_more<T: Default>(self, one_or_more: &mut OneOrMore<T>) {
        if let Self::M(m) = self {
            one_or_more.resize_with(m as usize, Default::default)
        }
    }

    pub fn iter(self) -> impl Iterator<Item = u8> {
        match self {
            Self::All => (u8::MAX..=u8::MAX),
            Self::M(m) => (0..=(m - 1)),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ActiveMonitors::All => 1,
            ActiveMonitors::M(m) => *m as _,
        }
    }
}
