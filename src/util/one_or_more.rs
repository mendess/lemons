use std::mem::replace;
use std::ops::{Index, IndexMut};

#[derive(Debug)]
pub enum OneOrMore<T> {
    One(T),
    More(Vec<T>),
}

impl<T: Default> Default for OneOrMore<T> {
    fn default() -> Self {
        Self::One(Default::default())
    }
}

macro_rules! impl_index {
    ($($t:ty)+) => {
        $(
        impl<T> Index<$t> for OneOrMore<T> {
            type Output = T;
            fn index(&self, i: $t) -> &Self::Output {
                match self {
                    Self::One(t) => t,
                    Self::More(m) => &m[i as usize],
                }
            }
        }

        impl<T> IndexMut<$t> for OneOrMore<T> {
            fn index_mut(&mut self, i: $t) -> &mut T {
                match self {
                    Self::One(t) => t,
                    Self::More(m) => &mut m[i as usize],
                }
            }
        }
        )*
    };
}

impl_index!(u8 u16 u32 u64 usize);

impl<T> OneOrMore<T> {
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::More(m) => m.len(),
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        match self {
            Self::One(t) => Iter::One(Some(t)),
            Self::More(m) => Iter::More(m.iter()),
        }
    }

    pub fn resize_with<F>(&mut self, new_len: usize, f: F)
    where
        F: FnMut() -> T,
    {
        if new_len > 1 {
            let mut to_resize = match replace(self, OneOrMore::More(vec![])) {
                Self::One(o) => vec![o],
                Self::More(m) => m,
            };
            to_resize.resize_with(new_len, f);
            *self = Self::More(to_resize);
        }
    }

    pub fn push(&mut self, t: T) {
        match replace(self, OneOrMore::More(vec![])) {
            OneOrMore::One(o) => *self = OneOrMore::More(vec![o, t]),
            OneOrMore::More(mut v) => v.push(t),
        }
    }
}

#[derive(Debug)]
pub enum Iter<'a, T> {
    One(Option<&'a T>),
    More(std::slice::Iter<'a, T>),
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(t) => t.take(),
            Self::More(m) => m.next(),
        }
    }
}
