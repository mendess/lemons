use std::num::NonZeroU8;

use crate::util::one_or_more::OneOrMore;

#[derive(Debug, Clone, Copy)]
pub enum ActiveMonitors {
    All,
    MonitorCount(NonZeroU8),
}

impl Default for ActiveMonitors {
    fn default() -> Self {
        Self::All
    }
}

impl ActiveMonitors {
    pub fn resize_one_or_more<T: Default>(self, one_or_more: &mut OneOrMore<T>) {
        if let Self::MonitorCount(m) = self {
            one_or_more.resize_with(m.get() as usize, Default::default)
        }
    }

    pub fn iter(self) -> impl Iterator<Item = u8> {
        match self {
            Self::All => u8::MAX..=u8::MAX,
            Self::MonitorCount(m) => 0..=(m.get() - 1),
        }
    }
}
