pub mod alignment;
pub mod block;
pub mod color;
pub mod global_config;
pub mod layer;
pub mod monitor;

pub use alignment::Alignment;
use block::Block;
pub use color::Color;
pub use layer::Layer;
pub use monitor::ActiveMonitors;
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct Config<'a>(pub [Vec<Block<'a>>; 3]);

impl<'a> Index<Alignment> for Config<'a> {
    type Output = Vec<Block<'a>>;

    fn index(&self, a: Alignment) -> &Self::Output {
        &self.0[a as usize]
    }
}

impl<'a> IndexMut<Alignment> for Config<'a> {
    fn index_mut(&mut self, a: Alignment) -> &mut Self::Output {
        &mut self.0[a as usize]
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Indexes([usize; 3]);

impl Indexes {
    pub fn get(&mut self, alignment: Alignment) -> usize {
        let i = self.0[alignment as usize];
        self.0[alignment as usize] += 1;
        i
    }
}
