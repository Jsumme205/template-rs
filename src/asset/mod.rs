use core::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ptr::{self, NonNull},
};

pub mod css;
pub mod set;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    error::Error,
    utils::{ConstTyId, MaybeArc},
    writer::Writer,
};

pub struct AssetContext<'a, 'path> {
    paths: &'a mut Vec<MaybeArc<'path, str>>,
    writer: &'a mut dyn Writer,
}

pub trait Asset {
    fn write_inline(&self, writer: &mut dyn Writer) -> Result<(), Error>;

    fn write_file(&self, writer: &mut AssetContext<'_, '_>) -> Result<(), Error>;
}

pub(crate) struct AssetVTable {
    type_id: ConstTyId,
    write_inline: unsafe fn(*const (), *const (), &mut dyn Writer) -> Result<(), Error>,
    write_file: unsafe fn(*const (), *const (), &mut AssetContext<'_, '_>) -> Result<(), Error>,
    hash_fn: unsafe fn(*const (), *const (), &mut dyn Hasher),
    eq_fn: unsafe fn(*const (), *const (), &RawDynAsset) -> bool,
    drop_fn: unsafe fn(*mut (), *mut ()),
}

struct RawDynAsset {
    data_ptr: NonNull<()>,
    metadata_ptr: NonNull<()>,
    vtable: &'static AssetVTable,
}

impl RawDynAsset {
    const unsafe fn from_ref<A>(r: &A) -> Self
    where
        A: ?Sized + Asset + Hash + Eq,
    {
        let [data_ptr, metadata_ptr]: [*mut (); 2] =
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                unsafe { mem::transmute_copy(&r) }
            } else {
                [r as *const _ as *mut (), ptr::dangling_mut()]
            };

        Self {
            data_ptr: unsafe { NonNull::new_unchecked(data_ptr) },
            metadata_ptr: unsafe { NonNull::new_unchecked(metadata_ptr) },
            vtable: const {
                &AssetVTable {
                    type_id: ConstTyId::of::<A>(),
                    write_file: imp::write_file_shim::<A>,
                    write_inline: imp::write_inline_shim::<A>,
                    hash_fn: imp::hash_shim::<A>,
                    eq_fn: imp::eq_shim::<A>,
                    drop_fn: |_, _| {},
                }
            },
        }
    }

    fn from_boxed<A>(asset: Box<A>) -> Self
    where
        A: ?Sized + Asset + Eq + Hash,
    {
        let r = Box::into_raw(asset);
        let [data_ptr, metadata_ptr]: [*mut (); 2] =
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                unsafe { mem::transmute_copy(&r) }
            } else {
                [r as *const _ as *mut (), ptr::dangling_mut()]
            };

        unsafe {
            Self {
                data_ptr: NonNull::new_unchecked(data_ptr),
                metadata_ptr: NonNull::new_unchecked(metadata_ptr),
                vtable: const {
                    &AssetVTable {
                        type_id: ConstTyId::of::<A>(),
                        write_file: imp::write_file_shim::<A>,
                        write_inline: imp::write_inline_shim::<A>,
                        drop_fn: imp::drop_shim_boxed::<A>,
                        hash_fn: imp::hash_shim::<A>,
                        eq_fn: imp::eq_shim::<A>,
                    }
                },
            }
        }
    }

    unsafe fn write_inline(&self, writer: &mut dyn Writer) -> Result<(), Error> {
        unsafe {
            (self.vtable.write_inline)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr(), writer)
        }
    }

    unsafe fn write_file(&self, ctx: &mut AssetContext<'_, '_>) -> Result<(), Error> {
        unsafe { (self.vtable.write_file)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr(), ctx) }
    }

    unsafe fn hash(&self, hasher: &mut dyn Hasher) {
        unsafe { (self.vtable.hash_fn)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr(), hasher) }
    }

    unsafe fn equals(&self, other: &Self) -> bool {
        unsafe { (self.vtable.eq_fn)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr(), other) }
    }

    fn is<T: ?Sized>(&self) -> bool {
        self.vtable.type_id == ConstTyId::of::<T>()
    }

    unsafe fn cast<T>(&self) -> *mut T
    where
        T: ?Sized,
    {
        debug_assert!(self.vtable.type_id.get() == ConstTyId::of::<T>().get());

        unsafe {
            // fat pointer
            if mem::size_of::<&T>() == 2 * mem::size_of::<usize>() {
                let dm = [self.data_ptr, self.metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const self.data_ptr;
                (ptr as *const *mut T).read()
            }
        }
    }
}

impl Drop for RawDynAsset {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop_fn)(self.data_ptr.as_ptr(), self.metadata_ptr.as_ptr()) }
    }
}

pub struct DynAsset<'asset> {
    raw: RawDynAsset,
    _marker: PhantomData<&'asset (dyn Asset + 'asset)>,
}

impl<'asset> DynAsset<'asset> {
    pub const fn from_ref<A>(asset: &'asset A) -> Self
    where
        A: ?Sized + Asset + Hash + Eq,
    {
        unsafe {
            Self {
                raw: RawDynAsset::from_ref(asset),
                _marker: PhantomData,
            }
        }
    }

    pub fn from_boxed<A>(asset: Box<A>) -> Self
    where
        A: ?Sized + Asset + Eq + Hash,
    {
        Self {
            raw: RawDynAsset::from_boxed(asset),
            _marker: PhantomData,
        }
    }

    pub fn write_inline(&self, writer: &mut dyn Writer) -> Result<(), Error> {
        unsafe { self.raw.write_inline(writer) }
    }

    pub fn write_file(&self, ctx: &mut AssetContext<'_, '_>) -> Result<(), Error> {
        unsafe { self.raw.write_file(ctx) }
    }

    pub fn is<T: ?Sized>(&self) -> bool {
        self.raw.is::<T>()
    }

    pub(crate) unsafe fn cast<T: ?Sized>(&self) -> &T {
        unsafe { &*self.raw.cast() }
    }
}

impl PartialEq for DynAsset<'_> {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.raw.equals(&other.raw) }
    }
}

impl Hash for DynAsset<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe {
            self.raw.hash(state);
        }
    }
}

impl Eq for DynAsset<'_> {}

pub trait IntoDynAsset<'asset, __M = ()> {
    fn into_dyn_asset(self) -> DynAsset<'asset>;
}

impl<'asset> IntoDynAsset<'asset> for DynAsset<'asset> {
    fn into_dyn_asset(self) -> DynAsset<'asset> {
        self
    }
}

pub struct Ref;
pub struct Value;
pub struct Boxed;

impl<'asset, A> IntoDynAsset<'asset, Ref> for &'asset A
where
    A: ?Sized + Asset + Hash + Eq,
{
    fn into_dyn_asset(self) -> DynAsset<'asset> {
        DynAsset::from_ref(self)
    }
}

impl<'asset, A> IntoDynAsset<'asset, Value> for A
where
    A: Asset + Eq + Hash,
{
    fn into_dyn_asset(self) -> DynAsset<'asset> {
        DynAsset::from_boxed(Box::new(self))
    }
}

impl<'asset, A> IntoDynAsset<'asset, Boxed> for Box<A>
where
    A: Asset + Eq + Hash + ?Sized,
{
    fn into_dyn_asset(self) -> DynAsset<'asset> {
        DynAsset::from_boxed(self)
    }
}

mod imp {
    use core::{
        hash::{Hash, Hasher},
        mem,
    };

    use alloc::boxed::Box;

    use crate::{
        asset::{Asset, AssetContext},
        error::Error,
        utils::ConstTyId,
        writer::Writer,
    };

    pub(super) unsafe fn write_inline_shim<A>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        writer: &mut dyn Writer,
    ) -> Result<(), Error>
    where
        A: ?Sized + Asset,
    {
        let ptr: *const A = unsafe {
            // fat pointer
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                let dm = [data_ptr, metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const data_ptr;
                (ptr as *const *const A).read()
            }
        };

        unsafe { A::write_inline(&*ptr, writer) }
    }

    pub(super) unsafe fn write_file_shim<A>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        ctx: &mut AssetContext<'_, '_>,
    ) -> Result<(), Error>
    where
        A: ?Sized + Asset,
    {
        let ptr: *const A = unsafe {
            // fat pointer
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                let dm = [data_ptr, metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const data_ptr;
                (ptr as *const *const A).read()
            }
        };

        unsafe { A::write_file(&*ptr, ctx) }
    }

    pub(super) unsafe fn drop_shim_boxed<A>(data_ptr: *mut (), metadata_ptr: *mut ())
    where
        A: ?Sized,
    {
        let ptr: *mut A = unsafe {
            // fat pointer
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                let dm = [data_ptr, metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const data_ptr;
                (ptr as *const *mut A).read()
            }
        };

        drop(unsafe { Box::from_raw(ptr) });
    }

    pub(super) unsafe fn eq_shim<A>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        other: &super::RawDynAsset,
    ) -> bool
    where
        A: ?Sized + Eq,
    {
        if ConstTyId::of::<A>() != other.vtable.type_id {
            return false;
        }

        let ptr: *const A = unsafe {
            // fat pointer
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                let dm = [data_ptr, metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const data_ptr;
                (ptr as *const *mut A).read()
            }
        };

        unsafe { A::eq(&*ptr, &*other.cast::<A>()) }
    }

    pub(super) unsafe fn hash_shim<A>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        mut hasher: &mut dyn Hasher,
    ) where
        A: ?Sized + Hash,
    {
        let ptr: *const A = unsafe {
            // fat pointer
            if mem::size_of::<&A>() == 2 * mem::size_of::<usize>() {
                let dm = [data_ptr, metadata_ptr];
                mem::transmute_copy(&dm)
            } else {
                let ptr = &raw const data_ptr;
                (ptr as *const *mut A).read()
            }
        };

        unsafe {
            A::hash(&*ptr, &mut hasher);
        }
    }
}
