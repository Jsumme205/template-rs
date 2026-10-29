use core::{fmt::Display, sync::atomic::AtomicUsize};

use alloc::{boxed::Box, string::String};

pub(crate) mod metadata;
pub(crate) mod set;

pub mod cell;

use crate::{
    class_interner::{ClassInterner, cell::ClassInternerCell},
    component::{
        cell::ComponentSetCell,
        metadata::{ComponentMetadata, HasComponentMetadata, MetadataBuilder},
        set::{ComponentRange, ComponentRangeBuilder, ComponentSet, DynComponent},
    },
    error::Error,
    renderable::Renderable,
    utils::{BorrowCount, Boxed, MaybeArc, MaybeBoxed},
    writer::Writer,
};

pub struct WriteContext<'a, 'r> {
    writer: &'a mut (dyn Writer + 'a),
    pub(crate) component_set: &'a ComponentSet<'r>,
    pub(crate) interner: &'a ClassInterner,
}

impl<'a, 'r> WriteContext<'a, 'r> {
    pub fn in_scope<'meta, T, F>(&mut self, component: &T, write: F) -> Result<(), Error>
    where
        F: FnOnce(&mut WriteContext<'_, 'r>) -> Result<(), Error>,
        T: HasComponentMetadata<'meta> + Tag,
    {
        let mut s = String::new();

        let mut tmp_buffer = WriteContext {
            writer: &mut s,
            component_set: self.component_set,
            interner: self.interner,
        };

        tmp_buffer.write_beginning_tag(T::TAG, component.metadata())?;

        if let Err(err) = write(&mut tmp_buffer) {
            return Err(err);
        } else {
            self.writer.pull(&mut s)?;
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

pub struct Div<'set, 'meta, Contents: ?Sized = dyn Display> {
    set: ComponentRange<'set>,
    contents: Option<MaybeBoxed<'set, Contents>>,
    metadata: ComponentMetadata<'meta>,
}

impl<'set, 'meta, Contents: ?Sized> HasComponentMetadata<'meta> for Div<'set, 'meta, Contents> {
    fn metadata(&self) -> &ComponentMetadata<'meta> {
        &self.metadata
    }
}

impl<'set, 'meta, Contents: ?Sized> Tag for Div<'set, 'meta, Contents> {
    const TAG: &'static str = "div";
}

impl<'set, 'meta, Contents: ?Sized> Component for Div<'meta, 'set, Contents>
where
    Contents: Renderable,
{
    fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        ctx.in_scope(self, |ctx| {
            if let Some(contents) = &self.contents {
                contents.render(ctx.writer)?;
            }
            self.set.render_components(ctx)?;
            Ok(())
        })
    }
}

pub struct DivBuilder<'range, 'set, 'meta, C: ?Sized = dyn Display, B = AtomicUsize> {
    range_builder: ComponentRangeBuilder<'range, 'set, B>,
    contents: Option<MaybeBoxed<'range, C>>,
    metadata: MetadataBuilder<'meta, 'set, B>,
}

impl<'range, 'meta> Div<'range, 'meta> {
    pub fn builder<'set, B: BorrowCount>(
        component_set: &'set ComponentSetCell<'range, B>,
        interner: &'set ClassInternerCell<B>,
    ) -> DivBuilder<'range, 'set, 'meta, dyn Display, B> {
        DivBuilder {
            range_builder: ComponentRange::builder(component_set),
            contents: None,
            metadata: ComponentMetadata::builder(interner),
        }
    }
}

impl<'range, 'set, 'meta> DivBuilder<'range, 'set, 'meta> {
    pub fn contents<C>(
        self,
        contents: impl Into<MaybeBoxed<'range, C>>,
    ) -> DivBuilder<'range, 'set, 'meta, C>
    where
        C: ?Sized,
    {
        DivBuilder {
            contents: Some(contents.into()),
            metadata: self.metadata,
            range_builder: self.range_builder,
        }
    }
}

impl<'range, 'set, 'meta, Cnt: ?Sized> DivBuilder<'range, 'set, 'meta, Cnt> {
    pub fn child<C>(&mut self, child: C) -> &mut Self
    where
        C: Into<DynComponent<'range>>,
    {
        let child = child.into();

        self.range_builder.register(child);
        self
    }

    pub fn id(&mut self, id: impl Into<MaybeArc<'meta, str>>) -> &mut Self {
        self.metadata.id(id);
        self
    }

    pub fn class(&mut self, class: impl AsRef<str>) -> &mut Self {
        self.metadata.class(class);
        self
    }

    pub fn finish(&mut self) -> Div<'range, 'meta, Cnt> {
        Div {
            set: self.range_builder.finish(),
            contents: self.contents.take(),
            metadata: self.metadata.finish(),
        }
    }
}

fn _f() {
    let component_set = ComponentSetCell::new();
    let interner = ClassInternerCell::new();

    Div::builder(&component_set, &interner)
        .child(Div::builder(&component_set, &interner).finish().boxed())
        .finish();
}
