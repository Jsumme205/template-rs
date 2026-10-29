use core::fmt;

use crate::{error::Error, writer::Writer};

pub trait Renderable {
    fn render<W: ?Sized + crate::writer::Writer>(&self, writer: &mut W) -> Result<(), Error>;
}

impl<T: ?Sized + fmt::Display> Renderable for T {
    fn render<W: ?Sized + crate::writer::Writer>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_args(format_args!("{self}"))
    }
}

pub struct RenderFn<F>(F);

impl<F> Renderable for RenderFn<F>
where
    F: Fn(&mut dyn Writer) -> Result<(), Error>,
{
    fn render<W: ?Sized + crate::writer::Writer>(&self, mut writer: &mut W) -> Result<(), Error> {
        (self.0)(&mut writer)
    }
}
