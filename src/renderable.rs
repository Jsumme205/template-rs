use core::fmt;

use crate::{
    error::Error,
    writer::{Writer, WriterRef},
};

pub trait Renderable {
    fn render(&self, writer: &mut WriterRef<'_>) -> Result<(), Error>;
}

impl<T: ?Sized + fmt::Display> Renderable for T {
    fn render(&self, writer: &mut WriterRef<'_>) -> Result<(), Error> {
        write!(writer, "{self}")
    }
}

pub struct RenderFn<F>(F);

impl<F> Renderable for RenderFn<F>
where
    F: Fn(&mut WriterRef<'_>) -> Result<(), Error>,
{
    fn render(&self, mut writer: &mut WriterRef<'_>) -> Result<(), Error> {
        (self.0)(&mut writer)
    }
}

pub fn render_fn<F>(f: F) -> RenderFn<F>
where
    F: Fn(&mut WriterRef<'_>) -> Result<(), Error>,
{
    fn assert_renderable<T: Renderable>(v: T) -> T {
        v
    }

    assert_renderable(RenderFn(f))
}
