use core::sync::atomic::AtomicUsize;

use alloc::string::String;

use crate::{
    body::Body,
    class_interner::cell::ClassInternerCell,
    component::{Component, WriteContext, cell::ComponentSetCell},
    error::Error,
    head::Head,
    utils::{BorrowCount, MaybeArc},
    writer::Writer,
};

pub struct Builder<'asset, 'set, B> {
    component_set: &'set ComponentSetCell<'asset, B>,
    interner: &'set ClassInternerCell<B>,

    config: Config<'asset>,
}

pub struct Config<'a> {
    pub output_dir: MaybeArc<'a, str>,
}

pub struct Renderer<'asset, 'set, APP, B = AtomicUsize> {
    // registration set stuff
    component_set: &'set ComponentSetCell<'asset, B>,
    interner: &'set ClassInternerCell<B>,

    config: Config<'asset>,

    head: Head<'asset>,
    body: Body<'asset>,
    root: APP,
}

impl<'asset, 'set, APP, B> Renderer<'asset, 'set, APP, B>
where
    B: BorrowCount,
    APP: Component,
{
    pub fn render<W>(&mut self, writer: &mut W) -> Result<(), Error>
    where
        W: Writer + ?Sized,
    {
        write_prefix(writer)?;

        let mut tmp = String::new();

        let mut context = WriteContext {
            writer: &mut tmp,
            component_set: &*self
                .component_set
                .try_read()
                .expect("this should never fail"),
            interner: &*self.interner.try_read().expect("this should never fail"),
        };

        self.head.render(&mut context, |ctx| Ok(()))?;

        self.body
            .render(&mut context, |ctx| self.root.render(ctx))?;

        write_postfix(writer)?;

        writer.pull(&mut tmp)?;

        Ok(())
    }

    pub fn bundle<W, IO>(&mut self, writer: &mut W, io: &IO) -> Result<(), Error>
    where
        W: Writer + ?Sized,
        IO: Io + ?Sized,
        IO::Error: Into<Error>,
        str: AsRef<IO::Path>,
    {
        todo!()
    }
}

fn write_prefix<W>(writer: &mut W) -> Result<(), Error>
where
    W: ?Sized + Writer,
{
    const BEGINNING_TEMPLATE_STRING: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n";
    writer.write_str(BEGINNING_TEMPLATE_STRING)
}

fn write_postfix<W>(writer: &mut W) -> Result<(), Error>
where
    W: ?Sized + Writer,
{
    writer.write_str("</html>")
}

pub trait Io: 'static {
    type Error;
    type Path: ?Sized;

    fn read_file(&self, path: &Self::Path) -> Result<String, Self::Error>;

    fn copy(&self, src: &Self::Path, dst: &Self::Path) -> Result<(), Self::Error>;
}
