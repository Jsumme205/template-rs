use core::{marker::PhantomData, mem, ptr::NonNull, slice};

use alloc::{boxed::Box, vec::Vec};

use crate::{
    component::{Component, OwnedComponent, WriteContext},
    error::Error,
    utils::{self, BorrowCount, ConstTyId},
};

#[repr(C)]
pub(crate) struct ComponentVTable {
    typeid: ConstTyId,
    render: unsafe fn(*const (), *const (), ctx: &mut WriteContext<'_, '_>) -> Result<(), Error>,
    drop: unsafe fn(*mut (), *mut ()),
}

pub struct DynComponent<'a> {
    /// this is either an &'a dyn Component or a Box<dyn Component>
    //
    // reasoning for the raw component:
    // until #[may_dangle] is stablized, this causes rejections in the borrow checkr for programs that are safe.
    raw: RawComponent,
    _marker: PhantomData<&'a (dyn Component + 'a)>,
}

struct RawComponent {
    data_ptr: NonNull<()>,
    metadata_ptr: NonNull<()>,
    vtable: &'static ComponentVTable,
}

impl RawComponent {
    const unsafe fn from_ref<C>(component: &C) -> Self
    where
        C: ?Sized + Component,
    {
        let vtable: &'static ComponentVTable = const {
            &ComponentVTable {
                typeid: ConstTyId::of::<C>(),
                render: imp::component_render_shim::<C>,
                drop: imp::noop_drop_shim,
            }
        };

        let (data_ptr, metadata_ptr) = utils::deconstruct(component);

        Self {
            data_ptr,
            metadata_ptr,
            vtable,
        }
    }

    fn from_owned<C>(component: Box<C>) -> Self
    where
        C: ?Sized + OwnedComponent,
    {
        let ptr = Box::into_non_null(component);

        let vtable: &'static ComponentVTable = const {
            &ComponentVTable {
                typeid: ConstTyId::of::<C>(),
                render: imp::component_render_shim::<C>,
                drop: imp::drop_shim::<C>,
            }
        };

        let (data_ptr, metadata_ptr) = utils::deconstruct(ptr);

        Self {
            data_ptr,
            metadata_ptr,
            vtable,
        }
    }
}

impl Drop for RawComponent {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr()) }
    }
}

impl<'a> DynComponent<'a> {
    pub const fn from_ref<C>(component: &'a C) -> Self
    where
        C: Component + 'a,
    {
        unsafe {
            Self {
                raw: RawComponent::from_ref(component),
                _marker: PhantomData,
            }
        }
    }

    pub fn from_owned<C>(component: C) -> Self
    where
        C: OwnedComponent + 'a,
    {
        Self::from_boxed(component.into())
    }

    pub fn from_boxed<C>(component: Box<C>) -> Self
    where
        C: OwnedComponent + 'a,
    {
        Self {
            raw: RawComponent::from_owned(component),
            _marker: PhantomData,
        }
    }

    pub fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        unsafe {
            (self.raw.vtable.render)(
                self.raw.data_ptr.as_ptr(),
                self.raw.metadata_ptr.as_ptr(),
                ctx,
            )
        }
    }

    pub fn is_same_as<C>(&self) -> bool {
        self.raw.vtable.typeid.get() == ConstTyId::of::<C>().get()
    }
}

pub trait IntoDynComponent<'c, M> {
    type Component: Component;

    fn into_dyn_component(self) -> DynComponent<'c>;
}

pub struct Ref;
pub struct Val;

impl<'component, C> IntoDynComponent<'component, Ref> for &'component C
where
    C: Component,
{
    type Component = C;

    fn into_dyn_component(self) -> DynComponent<'component> {
        DynComponent::from_ref(self)
    }
}

impl<'c, C> IntoDynComponent<'c, Val> for C
where
    C: OwnedComponent + 'c,
{
    type Component = C;

    fn into_dyn_component(self) -> DynComponent<'c> {
        DynComponent::from_owned(self)
    }
}

mod imp {
    use core::{alloc::Layout, ptr};

    use crate::{
        component::{Component, WriteContext},
        error::Error,
        utils,
    };

    pub(super) unsafe fn component_render_shim<C: Component + ?Sized>(
        ptr: *const (),
        metadata_ptr: *const (),
        ctx: &mut WriteContext<'_, '_>,
    ) -> Result<(), Error> {
        let ptr = utils::reconstruct(ptr, metadata_ptr);

        unsafe { C::render(&*ptr, ctx) }
    }

    pub(super) unsafe fn drop_shim<T: ?Sized>(ptr: *mut (), metadata_ptr: *mut ()) {
        unsafe {
            let ptr = utils::reconstruct(ptr, metadata_ptr);
            let layout = Layout::for_value_raw(ptr);
            ptr::drop_in_place(ptr as *mut T);
            alloc::alloc::dealloc(ptr as *mut u8, layout);
        }
    }

    pub(super) unsafe fn noop_drop_shim(_ptr: *mut (), _meta: *mut ()) {}
}

pub struct ComponentSet<'a> {
    inner: Vec<DynComponent<'a>>,
}

impl<'a> ComponentSet<'a> {
    pub(crate) const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn register(&mut self, component: DynComponent<'a>, range: &mut ComponentRange<'a>) {
        let index = self.inner.len();
        self.inner.push(component);
        match &mut range.repr {
            ComponentRangeRepr::IndicesSet { set } => set.push(index),
            ComponentRangeRepr::SingleRange { range: r } => {
                if r.end.checked_sub(1).is_none_or(|v| v == index) {
                    r.end += 1;
                }
                range.repr = ComponentRangeRepr::IndicesSet {
                    set: {
                        let mut c = r.collect::<Vec<_>>();
                        c.push(index);
                        c
                    },
                }
            }
        }
    }

    pub fn components_in_set<C>(&self) -> ComponentsInSet<'_, 'a, C> {
        ComponentsInSet {
            iter: self.inner.iter(),
            _marker: PhantomData,
        }
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }
}

pub struct ComponentsInSet<'a, 'set, C: ?Sized> {
    iter: slice::Iter<'a, DynComponent<'set>>,
    _marker: PhantomData<fn() -> &'a C>,
}

impl<'a, 'set, C> Iterator for ComponentsInSet<'a, 'set, C> {
    type Item = &'a C;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.iter.next()?;
            if next.is_same_as::<C>() {
                return Some(unsafe { next.raw.data_ptr.cast::<C>().as_ref() });
            }
        }
    }
}

pub struct ComponentRange<'a> {
    repr: ComponentRangeRepr,
    _marker: PhantomData<&'a ()>,
}

impl<'a> ComponentRange<'a> {
    pub fn render_components(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        let mut error = None;

        match &self.repr {
            ComponentRangeRepr::IndicesSet { set: indices } => {
                indices.iter().for_each(|&component_index| {
                    let component = ctx
                        .component_set
                        .inner
                        .get(component_index)
                        .expect("this shouldn't be none");
                    error = component.render(ctx).err();
                });
            }
            ComponentRangeRepr::SingleRange { range } => {
                let slice = ctx
                    .component_set
                    .inner
                    .get(range.clone())
                    .expect("this shouldn't be none");

                slice.iter().for_each(|component| {
                    error = component.render(ctx).err();
                });
            }
        }

        if let Some(error) = error {
            return Err(error);
        } else {
            Ok(())
        }
    }
}

impl<'range> ComponentRange<'range> {
    pub(crate) fn builder<'set>(
        component_set: &'set mut ComponentSet<'range>,
    ) -> ComponentRangeBuilder<'range, 'set> {
        ComponentRangeBuilder {
            range: ComponentRange {
                repr: ComponentRangeRepr::SingleRange {
                    range: component_set.len()..component_set.len(),
                },
                _marker: PhantomData,
            },
            component_set,
        }
    }
}

pub(crate) struct ComponentRangeBuilder<'range, 'set> {
    component_set: &'set mut ComponentSet<'range>,
    range: ComponentRange<'range>,
}

impl<'range, 'set> ComponentRangeBuilder<'range, 'set> {
    pub(crate) fn register(&mut self, component: DynComponent<'range>) -> &mut Self {
        let component = component;
        self.component_set.register(component, &mut self.range);

        self
    }

    pub(crate) fn finish(&mut self) -> ComponentRange<'range> {
        mem::replace(
            &mut self.range,
            ComponentRange {
                repr: ComponentRangeRepr::IndicesSet { set: alloc::vec![] },
                _marker: PhantomData,
            },
        )
    }
}

enum ComponentRangeRepr {
    SingleRange { range: core::ops::Range<usize> },
    IndicesSet { set: alloc::vec::Vec<usize> },
}
