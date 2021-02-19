use super::Lemonbar;
use crate::block::{Block, Content};
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct DisplayBlock<'a, 'b: 'a>(pub &'b Block<'a>, pub usize);

impl<'a, 'b> Display for DisplayBlock<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let DisplayBlock(b, mon) = self;
        if b.raw {
            return write!(f, "{}", DisplayContent(&b.content, *mon));
        }
        if let Some(x) = &b.offset {
            f.lemon('O', x)?;
        }
        if let Some(x) = &b.bg {
            f.lemon('B', x)?;
        }
        if let Some(x) = &b.fg {
            f.lemon('F', x)?;
        }
        if let Some(x) = &b.un {
            f.lemon('U', x)?;
            f.write_str("%{+u}")?;
        }
        if let Some(x) = &b.font {
            f.lemon('T', x)?;
        }
        let mut num_cmds = 0;
        for (i, a) in b
            .actions
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.map(|a| (i, a)))
        {
            write!(f, "%{{A{index}:{cmd}:}}", index = i + 1, cmd = a)?;
            num_cmds += 1;
        }
        write!(f, "{} ", DisplayContent(&b.content, *mon))?;
        (0..num_cmds).try_for_each(|_| f.write_str("%{A}"))?;
        if let Some(_) = &b.offset {
            f.lemon('O', "0")?;
        }
        if let Some(_) = &b.bg {
            f.lemon('B', "-")?;
        }
        if let Some(_) = &b.fg {
            f.lemon('F', "-")?;
        }
        if let Some(_) = &b.un {
            f.lemon('U', "-")?;
            f.write_str("%{-u}")?;
        }
        if let Some(_) = &b.font {
            f.lemon('T', "-")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DisplayContent<'a, 'b: 'a>(&'b Content<'a>, usize);

impl<'a, 'b> Display for DisplayContent<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Content::Static(s) => write!(f, "{}", s),
            Content::Cmd { last_run, .. } => write!(f, "{}", last_run[self.1].read().unwrap()),
            Content::Persistent { last_run, .. } => {
                write!(f, "{}", last_run[self.1].read().unwrap())
            }
        }
    }
}
