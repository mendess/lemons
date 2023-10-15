use std::fmt;

use super::CmdlineArgBuilder;

pub struct Lemonbar<W> {
    sink: W,
    separator: Option<&'static str>,
    already_wrote_first_block_of_aligment: bool,
}

pub struct LemonArgs {
    height: u32,
    outputs: Vec<Geometry>,
    args: Vec<String>,
}

impl Default for LemonArgs {
    fn default() -> Self {
        Self {
            height: 22,
            outputs: vec![],
            args: vec![
                // #clicables should just be the max allowed
                "-a".into(),
                u8::MAX.to_string(),
                // Force docking without asking the window manager.
                // This is needed if the window manager isn't EWMH compliant.
                "-d".into(),
            ],
        }
    }
}

impl CmdlineArgBuilder for LemonArgs {
    fn output(&mut self, name: &str) {
        self.outputs
            .push(resolve_output_to_geometry(name).expect("failed to get information from xrandr"))
    }

    fn height(&mut self, height: u32) {
        self.height = height;
    }

    fn bottom(&mut self) {
        self.args.push("-b".into());
    }

    fn fonts<'s>(&mut self, fonts: impl Iterator<Item = &'s str>) {
        self.args
            .extend(fonts.flat_map(|font| ["-f".into(), font.into()]))
    }

    fn name(&mut self, name: &str) {
        self.args.extend(["-n".into(), name.into()])
    }

    fn underline_width(&mut self, width: u32) {
        self.args.extend(["-u".into(), width.to_string()])
    }

    fn underline_color(&mut self, color: &crate::model::Color<'_>) {
        self.args.extend(["-U".into(), color.to_string()])
    }

    fn background(&mut self, color: &crate::model::Color<'_>) {
        self.args.extend(["-B".into(), color.to_string()])
    }

    fn foreground(&mut self, color: &crate::model::Color<'_>) {
        self.args.extend(["-F".into(), color.to_string()])
    }

    fn finish(mut self) -> Vec<String> {
        for o in self.outputs {
            self.args.extend([
                "-g".into(),
                format!("{}x{}+{}+0", o.width, self.height, o.x_offset),
            ])
        }
        self.args
    }
}

impl<W: fmt::Write> super::Bar<W> for Lemonbar<W> {
    type BarBlockBuilder<'bar> = LemonDisplayBlock<'bar, W>
        where Self: 'bar;

    type CmdlineArgBuilder = LemonArgs;

    const PROGRAM: &'static str = "lemonbar";

    fn new(sink: W, separator: Option<&'static str>) -> Self {
        Self {
            sink,
            separator,
            already_wrote_first_block_of_aligment: false,
        }
    }

    fn cmdline_builder() -> Self::CmdlineArgBuilder {
        LemonArgs::default()
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

struct Geometry {
    width: i32,
    x_offset: i32,
}

fn resolve_output_to_geometry(name: &str) -> Result<Geometry, xrandr::XrandrError> {
    let mut handle = xrandr::XHandle::open()?;
    let monitor = handle
        .monitors()?
        .into_iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("there is no monitor with name {name}"));

    Ok(Geometry {
        width: monitor.width_px,
        x_offset: monitor.x,
    })
}
