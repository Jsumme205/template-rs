use core::{fmt::Display, sync::atomic::AtomicUsize};

use alloc::string::String;

pub(crate) mod metadata;
pub(crate) mod set;

pub mod cell;

use crate::{
    class_interner::ClassInterner,
    component::{
        metadata::{ComponentMetadata, HasComponentMetadata},
        set::{ComponentRange, ComponentSet},
    },
    error::Error,
    renderable::Renderable,
    utils::{BorrowCount, MaybeArc, MaybeBoxed},
    writer::Writer,
};

pub struct WriteContext<'a, 'r> {
    pub(crate) writer: &'a mut (dyn Writer + 'a),
    pub(crate) component_set: &'a ComponentSet<'r>,
    pub(crate) interner: &'a ClassInterner,
}

impl<'a, 'r> WriteContext<'a, 'r> {
    pub fn in_scope<'meta, T, F>(&mut self, component: &T, write: F) -> Result<(), Error>
    where
        F: FnOnce(&mut WriteContext<'_, 'r>) -> Result<(), Error>,
        T: HasComponentMetadata<'meta> + Tag,
    {
        self.in_temporary_buffer(None, |tmp_buffer| {
            tmp_buffer.write_beginning_tag(T::TAG, component.metadata())?;

            if let Err(err) = write(tmp_buffer) {
                return Err(err);
            } else {
                if T::HAS_END_TAG {
                    tmp_buffer.write_end_tag(T::TAG)?;
                }
                Ok(())
            }
        })
    }

    pub fn in_temporary_buffer<F>(
        &mut self,
        size_hint: impl Into<Option<usize>>,
        write: F,
    ) -> Result<(), Error>
    where
        F: FnOnce(&mut WriteContext<'_, 'r>) -> Result<(), Error>,
    {
        let mut s = String::with_capacity(size_hint.into().unwrap_or(0));

        let mut tmp_buffer = WriteContext {
            writer: &mut s,
            component_set: self.component_set,
            interner: self.interner,
        };

        if let Err(e) = write(&mut tmp_buffer) {
            return Err(e);
        } else {
            self.writer.pull(&mut s)?;
            Ok(())
        }
    }

    pub fn in_tag<T, F>(&mut self, write: F) -> Result<(), Error>
    where
        F: FnOnce(&mut WriteContext<'_, 'r>) -> Result<(), Error>,
        T: Tag,
    {
        let mut buf = String::new();

        let mut ctx = WriteContext {
            writer: &mut buf,
            component_set: self.component_set,
            interner: self.interner,
        };

        ctx.write_beginning_tag(T::TAG, &ComponentMetadata::empty())?;

        if let Err(e) = write(&mut ctx) {
            return Err(e);
        } else {
            self.writer.pull(&mut buf)?;
            if T::HAS_END_TAG {
                self.write_end_tag(T::TAG)?;
            }
            Ok(())
        }
    }

    fn write_beginning_tag(
        &mut self,
        tag: &str,
        metadata: &ComponentMetadata<'_>,
    ) -> Result<(), Error> {
        self.writer
            .write_args(format_args!("<{} {}>\n", tag, metadata))
    }

    fn write_end_tag(&mut self, tag: &str) -> Result<(), Error> {
        self.writer.write_args(format_args!("</{}>\n", tag))
    }
}

pub trait Tag {
    const HAS_END_TAG: bool = true;

    const TAG: &'static str;
}

pub trait Component {
    fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error>;
}

pub struct Div<'asset, C: ?Sized = dyn Display> {
    meta: ComponentMetadata<'asset>,
    range: ComponentRange<'asset>,
    contents: Option<MaybeBoxed<'asset, C>>,
}

impl<'asset, C> Tag for Div<'asset, C>
where
    C: ?Sized,
{
    const TAG: &'static str = "div";
}

impl<'asset, C> HasComponentMetadata<'asset> for Div<'asset, C>
where
    C: ?Sized,
{
    fn metadata(&self) -> &ComponentMetadata<'asset> {
        &self.meta
    }
}

impl<'asset, C> Component for Div<'asset, C>
where
    C: ?Sized + Renderable,
{
    fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        ctx.in_scope(self, |ctx| {
            if let Some(contents) = &self.contents {
                contents.render(ctx.writer)?;
            }

            self.range.render_components(ctx)?;
            Ok(())
        })?;

        Ok(())
    }
}
