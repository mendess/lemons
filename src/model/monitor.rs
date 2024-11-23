use std::num::{NonZeroU8, NonZeroUsize};

use crate::util::one_or_more::OneOrMore;

use super::AffectedMonitor;

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

    pub fn iter(&self) -> impl Iterator<Item = AffectedMonitor> {
        let u8_range = match self {
            Self::All => u8::MAX..=u8::MAX,
            Self::MonitorCount(m) => 0..=(m.get() - 1),
        };
        u8_range.map(|m| {
            if m == u8::MAX {
                AffectedMonitor::All
            } else {
                AffectedMonitor::Single(m)
            }
        })
    }

    pub fn len(&self) -> NonZeroUsize {
        match self {
            Self::All => NonZeroUsize::new(1).unwrap(),
            Self::MonitorCount(n) => NonZeroUsize::from(*n),
        }
    }
}
