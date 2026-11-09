use core::fmt::Display;

use alloc::string::String;

pub(crate) mod metadata;
pub(crate) mod set;

use crate::{
    class_interner::Interner,
    component::{
        metadata::{ComponentMetadata, HasComponentMetadata},
        set::{ComponentRange, ComponentSet},
    },
    error::Error,
    renderable::Renderable,
    utils::MaybeBoxed,
    writer::{Writer, WriterRef},
};

pub struct WriteContext<'a, 'r> {
    pub(crate) writer: WriterRef<'a>,
    pub(crate) component_set: &'a ComponentSet<'r>,
    pub(crate) interner: &'a Interner,
}

impl<'a, 'r> WriteContext<'a, 'r> {
    pub const fn from_writer_ref<W>(
        component_set: &'a ComponentSet<'r>,
        interner: &'a Interner,
        writer: &'a mut W,
    ) -> Self
    where
        W: ?Sized + Writer,
    {
        todo!()
    }

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

        let mut tmp_buffer =
            WriteContext::from_writer_ref(self.component_set, self.interner, &mut s);
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

        let mut ctx = WriteContext::from_writer_ref(self.component_set, self.interner, &mut buf);

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
            .write_fmt(format_args!("<{} {}>\n", tag, metadata))
    }

    fn write_end_tag(&mut self, tag: &str) -> Result<(), Error> {
        self.writer.write_fmt(format_args!("</{}>\n", tag))
    }
}

pub trait Tag {
    const HAS_END_TAG: bool = true;

    const TAG: &'static str;
}

pub trait Component {
    fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error>;
}

impl Component for () {
    fn render(&self, _ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        // noop
        Ok(())
    }
}

const fn assert_owned_component<C: OwnedComponent + ?Sized>(_: &C) {}

const _: () = assert_owned_component(&());

impl<T> Component for &T
where
    T: ?Sized + Component,
{
    fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        T::render(*self, ctx)
    }
}

/// Marker trait for components that [`DynComponent::from_owned`] and
/// [`DynComponent::from_boxed`] may take ownership of.
///
/// # Why this exists
///
/// `DynComponent<'a>` erases the component's type, so the borrow checker cannot see
/// that dropping it runs the component's destructor. A `DynComponent<'a>` may be
/// dropped after some data borrowed for `'a` has already been freed, for example
/// when a `ComponentSet<'a>` is declared before the values it borrows. Dropping
/// the component must therefore not touch that data.
///
/// # Safety
///
/// By implementing this trait you promise that dropping a value of `Self` (its
/// `Drop::drop` impl, if any, and the drop glue of all its fields, recursively)
/// never reads, writes, or calls through any reference, pointer, or other handle
/// to data that is borrowed for a lifetime shorter than `'static`, even if that
/// data has already been dropped by that time.
///
/// This is satisfied if at least one of the following holds:
/// - `Self: 'static`, so there is no borrowed data to dangle.
/// - `Self` has no drop glue (`!needs_drop::<Self>()`). Plain references, `&str`
///   and `Copy` data are fine; they are only held, never accessed, when dropped.
/// - `Self` has drop glue, but none of it accesses borrowed data. A type that only
///   frees its own heap allocations (`String`, `Vec<u8>`, `Box<[u8]>`) is fine
///   even if it also stores `&'a` references that it never touches on drop.
///
/// The following do **not** satisfy the contract unless the specific instance
/// meets the conditions above:
/// - A `Drop` impl that dereferences a stored `&'a T`, `*const T`, or `Rc`/`Arc`
///   pointing to borrowed data (for example, logging a field on drop).
/// - A field holding a closure or trait object that borrows data
///   (`Box<dyn FnOnce() + 'a>`, `Box<dyn Trait + 'a>`), because its destructor is
///   opaque and may access that data.
/// - Guards that release a borrowed resource on drop (`MutexGuard<'a, _>`,
///   `Ref<'a, _>`, `RefMut<'a, _>`, `Drain<'a, _>`).
/// - Generic containers `Vec<T>`, `Option<T>`, etc. where `T` does not itself
///   uphold this contract.
///
/// Fields of type `DynComponent<'a>` are fine, because that type upholds the
/// contract through this trait.
pub unsafe trait OwnedComponent: Component {}

// SAFETY:
// `T: 'static` means that `T` contains no references or other lifetime-bearing
// data that can expire before the end of the program. Everything `T` can reach
// through its type is either owned by `T` itself (and dropped together with it) or
// valid for `'static`. The destructor of `T` therefore cannot observe data that
// has already been freed, however late the owning `DynComponent` is dropped.
//
// This relies on the usual Rust rule that safe code cannot make a `'static` type
// reach shorter-lived data. A type that smuggles such data past the lifetime
// system (for example, a raw pointer to stack data that its `Drop` impl
// dereferences) has already broken the contract of its own `unsafe` code, not
// this one.
unsafe impl<T> OwnedComponent for T where T: ?Sized + Component + 'static {}

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
                contents.render(&mut ctx.writer)?;
            }

            self.range.render_components(ctx)?;
            Ok(())
        })?;

        Ok(())
    }
}
