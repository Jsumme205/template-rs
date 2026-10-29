use core::{alloc::Layout, marker::PhantomData, mem, ptr::NonNull, slice};

use alloc::{boxed::Box, vec::Vec};

use crate::{
    component::{Component, WriteContext, cell::ComponentSetCell},
    error::Error,
    utils::{BorrowCount, ConstTyId},
};

#[repr(C)]
pub(crate) struct ComponentVTable {
    layout: Layout,
    typeid: ConstTyId,
    render: unsafe fn(*const (), ctx: &mut WriteContext<'_, '_>) -> Result<(), Error>,
    drop: unsafe fn(*mut (), layout: Layout),
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
    vtable: &'static ComponentVTable,
}

impl Drop for RawComponent {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(self.data_ptr.as_ptr(), self.vtable.layout) }
    }
}

impl<'a> DynComponent<'a> {
    pub const fn from_ref<C>(component: &'a C) -> Self
    where
        C: Component + 'a,
    {
        let vtable: &'static ComponentVTable = const {
            &ComponentVTable {
                layout: Layout::new::<C>(),
                typeid: ConstTyId::of::<C>(),
                render: imp::component_render_shim::<C>,
                drop: imp::noop_drop_shim,
            }
        };

        Self {
            raw: RawComponent {
                data_ptr: NonNull::from_ref(component).cast(),
                vtable,
            },
            _marker: PhantomData,
        }
    }

    pub fn from_owned<C>(component: C) -> Self
    where
        C: Component + 'a,
    {
        Self::from_boxed(component.into())
    }

    pub fn from_boxed<C>(component: Box<C>) -> Self
    where
        C: Component + 'a,
    {
        let ptr = Box::into_non_null(component);
        let vtable = const {
            &ComponentVTable {
                layout: Layout::new::<C>(),
                typeid: ConstTyId::of::<C>(),
                render: imp::component_render_shim::<C>,
                drop: imp::drop_shim::<C>,
            }
        };

        Self {
            raw: RawComponent {
                data_ptr: ptr.cast(),
                vtable,
            },
            _marker: PhantomData,
        }
    }

    pub fn render(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        unsafe { (self.raw.vtable.render)(self.raw.data_ptr.as_ptr(), ctx) }
    }

    pub fn is_same_as<C>(&self) -> bool {
        self.raw.vtable.typeid.get() == ConstTyId::of::<C>().get()
    }
}

impl<'a, C> From<&'a C> for DynComponent<'a>
where
    C: Component + 'a,
{
    fn from(value: &'a C) -> Self {
        Self::from_ref(value)
    }
}

impl<'a, C> From<Box<C>> for DynComponent<'a>
where
    C: Component + 'a,
{
    fn from(value: Box<C>) -> Self {
        Self::from_boxed(value)
    }
}

mod imp {
    use core::{alloc::Layout, ptr};

    use crate::{
        component::{Component, WriteContext},
        error::Error,
    };

    pub(super) unsafe fn component_render_shim<C: Component>(
        ptr: *const (),
        ctx: &mut WriteContext<'_, '_>,
    ) -> Result<(), Error> {
        unsafe { C::render(&*(ptr as *const C), ctx) }
    }

    pub(super) unsafe fn drop_shim<T>(ptr: *mut (), layout: Layout) {
        unsafe {
            ptr::drop_in_place(ptr as *mut T);
            alloc::alloc::dealloc(ptr as *mut u8, layout);
        }
    }

    pub(super) unsafe fn noop_drop_shim(_ptr: *mut (), _layout: Layout) {}
}

pub struct ComponentSet<'a> {
    inner: Vec<DynComponent<'a>>,
}

impl<'a> ComponentSet<'a> {
    pub(crate) const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn register<C>(&mut self, component: C, range: &mut ComponentRange<'a>)
    where
        C: Into<DynComponent<'a>>,
    {
        let index = self.inner.len();
        self.inner.push(component.into());
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
    pub(crate) fn builder<'set, B: BorrowCount>(
        component_set: &'set ComponentSetCell<'range, B>,
    ) -> ComponentRangeBuilder<'range, 'set, B> {
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

pub(crate) struct ComponentRangeBuilder<'range, 'set, B> {
    component_set: &'set ComponentSetCell<'range, B>,
    range: ComponentRange<'range>,
}

impl<'range, 'set, B> ComponentRangeBuilder<'range, 'set, B>
where
    B: BorrowCount,
{
    pub(crate) fn register<C>(&mut self, component: C) -> &mut Self
    where
        C: Into<DynComponent<'range>>,
    {
        let component = component.into();
        self.component_set
            .try_write()
            .expect("exclusive borrow failed")
            .register(component, &mut self.range);

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
