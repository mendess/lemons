use super::Lemonbar;
use crate::{event_loop::action_task::Action, model::block::Block};
use std::{
    borrow::Cow,
    fmt::{self, Display},
};

#[derive(Debug)]
pub struct DisplayBlock<'a, 'b: 'a>(pub &'b Block<'a>, pub usize, pub u8);

impl<'a, 'b> Display for DisplayBlock<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let DisplayBlock(b, index, mon) = self;
        let body = b.last_run[*mon].trim_end_matches('\n');
        if let Some(x) = &b.offset {
            f.lemon('O', x.0)?;
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
            f.lemon('T', x.0)?;
        }
        let mut num_cmds = 0;
        for i in b.available_actions.iter() {
            write!(
                f,
                "%{{A{mouse}:{action}:}}",
                mouse = i + 1,
                action = Action::new(b.alignment, *index, *mon, i),
            )?;
            num_cmds += 1;
        }
        let body = if b.raw {
            log::info!("Processing raw block");
            if body.ends_with('%') {
                Cow::Owned(format!("{}%", body))
            } else {
                Cow::Borrowed(body)
            }
        } else if body.contains('%') {
            Cow::Owned(body.replace('%', "%%"))
        } else {
            Cow::Borrowed(body)
        };
        write!(f, "{}", body)?;
        (0..num_cmds).try_for_each(|_| f.write_str("%{A}"))?;
        if b.font.is_some() {
            f.lemon('T', "-")?;
        }
        if b.un.is_some() {
            f.lemon('U', "-")?;
            f.write_str("%{-u}")?;
        }
        if b.fg.is_some() {
            f.lemon('F', "-")?;
        }
        if b.bg.is_some() {
            f.lemon('B', "-")?;
        }
        if b.offset.is_some() {
            f.lemon('O', "0")?;
        }
        Ok(())
    }
}
