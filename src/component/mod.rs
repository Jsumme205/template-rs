use core::{fmt::Display, sync::atomic::AtomicUsize};

use alloc::string::String;

pub(crate) mod metadata;
pub(crate) mod set;

pub mod cell;

use crate::{
    class_interner::{ClassInterner, cell::ClassInternerCell},
    component::{
        cell::ComponentSetCell,
        metadata::{ComponentMetadata, HasComponentMetadata, MetadataBuilder},
        set::{ComponentRange, ComponentRangeBuilder, ComponentSet, IntoDynComponent},
    },
    error::Error,
    renderable::Renderable,
    utils::{BorrowCount, MaybeArc, MaybeBoxed},
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

pub struct ComponentBuilder<'asset, 'set, C: ?Sized = dyn Display, B = AtomicUsize> {
    component_metadata: MetadataBuilder<'asset, 'set, B>,
    component_range: ComponentRangeBuilder<'asset, 'set, B>,
    contents: Option<MaybeBoxed<'asset, C>>,
}

impl<'asset, 'set, B> ComponentBuilder<'asset, 'set, dyn Display, B>
where
    B: BorrowCount,
{
    pub fn new(
        component_set: &'set ComponentSetCell<'asset, B>,
        interner: &'set ClassInternerCell<B>,
    ) -> Self {
        Self {
            component_metadata: ComponentMetadata::builder(interner),
            component_range: ComponentRange::builder(component_set),
            contents: None,
        }
    }

    pub fn with_contents<C>(
        self,
        contents: impl Into<MaybeBoxed<'asset, C>>,
    ) -> ComponentBuilder<'asset, 'set, C, B>
    where
        C: ?Sized,
    {
        ComponentBuilder {
            component_metadata: self.component_metadata,
            component_range: self.component_range,
            contents: Some(contents.into()),
        }
    }
}

impl<'asset, 'set, C, B> ComponentBuilder<'asset, 'set, C, B>
where
    C: ?Sized,
    B: BorrowCount,
{
    pub fn finish<Component>(&mut self) -> Component
    where
        Component: FromComponentBuilder<'asset, C>,
    {
        Component::from_component_parts(
            self.component_metadata.finish(),
            self.component_range.finish(),
            self.contents.take(),
        )
    }

    pub fn child<Child, M>(&mut self, child: Child) -> &mut Self
    where
        Child: IntoDynComponent<'asset, M>,
    {
        let component = child.into_dyn_component();
        self.component_range.register(component);
        self
    }

    pub fn id(&mut self, id: impl Into<MaybeArc<'asset, str>>) -> &mut Self {
        self.component_metadata.id(id);
        self
    }

    pub fn class(&mut self, class: impl AsRef<str>) -> &mut Self {
        self.component_metadata.class(class);
        self
    }
}

pub trait FromComponentBuilder<'asset, C: ?Sized> {
    fn from_component_parts(
        component_metadata: ComponentMetadata<'asset>,
        component_range: ComponentRange<'asset>,
        contents: Option<MaybeBoxed<'asset, C>>,
    ) -> Self;
}

pub fn builder<'asset, 'set, B>(
    component_set: &'set ComponentSetCell<'asset, B>,
    interner: &'set ClassInternerCell<B>,
) -> ComponentBuilder<'asset, 'set, dyn Display, B>
where
    B: BorrowCount,
{
    ComponentBuilder::new(component_set, interner)
}

pub struct Div<'asset, C: ?Sized = dyn Display> {
    meta: ComponentMetadata<'asset>,
    range: ComponentRange<'asset>,
    contents: Option<MaybeBoxed<'asset, C>>,
}

impl<'asset, C> FromComponentBuilder<'asset, C> for Div<'asset, C>
where
    C: ?Sized,
{
    fn from_component_parts(
        component_metadata: ComponentMetadata<'asset>,
        component_range: ComponentRange<'asset>,
        contents: Option<MaybeBoxed<'asset, C>>,
    ) -> Self {
        Self {
            meta: component_metadata,
            range: component_range,
            contents,
        }
    }
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
        })
    }
}
