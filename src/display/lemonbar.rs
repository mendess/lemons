use std::fmt;

pub struct Lemonbar<W> {
    sink: W,
    separator: Option<&'static str>,
    already_wrote_first_block_of_aligment: bool,
}

impl<W: fmt::Write> super::Bar<W> for Lemonbar<W> {
    type BarBlockBuilder<'bar> = LemonDisplayBlock<'bar, W>
        where Self: 'bar;

    fn new(sink: W, separator: Option<&'static str>) -> Self {
        Self {
            sink,
            separator,
            already_wrote_first_block_of_aligment: false,
        }
    }

    fn set_alignment(&mut self, alignment: crate::model::Alignment) -> fmt::Result {
        self.already_wrote_first_block_of_aligment = false;
        write!(self.sink, "{alignment}")
    }

    fn start_block(&mut self) -> Result<Self::BarBlockBuilder<'_>, fmt::Error> {
        if self.already_wrote_first_block_of_aligment {
            if let Some(sep) = self.separator {
                self.sink.write_str(sep)?;
            }
        } else {
            self.already_wrote_first_block_of_aligment = true;
        }
        Ok(LemonDisplayBlock::new(self))
    }

    fn into_inner(self) -> W {
        self.sink
    }
}

// TODO: cut one lifetime
pub struct LemonDisplayBlock<'bar, W> {
    bar: &'bar mut Lemonbar<W>,
    offset: bool,
    bg: bool,
    fg: bool,
    underline: bool,
    font: bool,
    actions: u8,
}

impl<'bar, W> LemonDisplayBlock<'bar, W> {
    fn new(bar: &'bar mut Lemonbar<W>) -> Self {
        Self {
            bar,
            offset: false,
            bg: false,
            fg: false,
            underline: false,
            font: false,
            actions: 0,
        }
    }
}

impl<'bar, W> LemonDisplayBlock<'bar, W>
where
    W: fmt::Write,
{
    fn write<P, S>(&mut self, prefix: P, s: S) -> fmt::Result
    where
        P: fmt::Display,
        S: fmt::Display,
    {
        write!(self.bar.sink, "%{{{}{}}}", prefix, s)
    }
}

impl<'bar, W> super::DisplayBlock for LemonDisplayBlock<'bar, W>
where
    W: fmt::Write,
{
    fn offset(&mut self, offset: &crate::model::block::Offset<'_>) -> fmt::Result {
        self.offset = true;
        self.write('O', offset.0)
    }

    fn bg(&mut self, color: &crate::model::Color<'_>) -> fmt::Result {
        self.bg = true;
        self.write('B', color)
    }

    fn fg(&mut self, color: &crate::model::Color<'_>) -> fmt::Result {
        self.fg = true;
        self.write('F', color)
    }

    fn underline(&mut self, color: &crate::model::Color<'_>) -> fmt::Result {
        self.underline = true;
        self.write('U', color)?;
        self.bar.sink.write_str("%{+u}")
    }

    fn font(&mut self, font: &crate::model::block::Font<'_>) -> fmt::Result {
        self.font = true;
        self.write('T', font.0)
    }

    fn add_action(&mut self, action: crate::event_loop::action_task::Action) -> fmt::Result {
        self.actions += 1;
        write!(
            self.bar.sink,
            "%{{A{button}:{action}:}}",
            button = action.button,
        )
    }

    fn text(&mut self, body: &str) -> fmt::Result {
        self.bar.sink.write_str(body)
    }

    fn finish(mut self) -> fmt::Result {
        for _ in 0..self.actions {
            self.bar.sink.write_str("%{A}")?;
        }
        if self.font {
            self.write('T', '-')?;
        }
        if self.underline {
            self.write('U', "-")?;
            self.bar.sink.write_str("%{-u}")?;
        }
        if self.fg {
            self.write('F', '-')?;
        }
        if self.bg {
            self.write('B', '-')?;
        }
        if self.offset {
            self.write('O', '0')?;
        }
        Ok(())
    }
}
