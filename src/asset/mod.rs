use core::{
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ptr::NonNull,
};

use alloc::boxed::Box;

use crate::{
    asset::set::AssetSet,
    error::Error,
    utils::{self, ConstTyId, MaybeArc},
    writer::WriterRef,
};

pub mod css;
pub mod set;

pub trait Asset {
    type Linkage: Linkage;

    fn create_linkage(&self) -> Self::Linkage;

    fn write(&self, writer: &mut WriterRef<'_>) -> Result<(), Error>;
}

impl<T> Asset for &T
where
    T: ?Sized + Asset,
{
    type Linkage = T::Linkage;

    fn create_linkage(&self) -> Self::Linkage {
        T::create_linkage(*self)
    }

    fn write(&self, writer: &mut WriterRef<'_>) -> Result<(), Error> {
        T::write(*self, writer)
    }
}

pub unsafe trait OwnedAsset: Asset {}

unsafe impl<T: ?Sized + Asset + 'static> OwnedAsset for T {}

pub mod sealed {
    use crate::asset::{Either, ExternLink, InlineLink, sealed};

    pub trait Sealed {}

    impl Sealed for InlineLink {}
    impl Sealed for ExternLink<'_> {}

    impl<T, U> sealed::Sealed for Either<T, U> {}
}

pub trait Linkage: Sized + sealed::Sealed {
    fn write_contents(
        &self,
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error>;
}

pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T, U> Linkage for Either<T, U>
where
    T: Linkage,
    U: Linkage,
{
    fn write_contents(
        &self,
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error> {
        match self {
            Self::Left(l) => l.write_contents(data, writer),
            Self::Right(r) => r.write_contents(data, writer),
        }
    }
}

pub struct AssetData<'a, 'b> {
    // pub file_name: Option<&'a str>,
    pub asset: &'a DynAsset<'b>,
}

pub enum TagKind {
    Script,
    Stylesheet,
    Image,
}

// <script> | <style>
//
// contents
//
// </script> | </style>
pub struct InlineLink {
    kind: TagKind,
}

impl Linkage for InlineLink {
    fn write_contents(
        &self,
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error> {
        todo!()
    }
}

// <script src="filename"> | <style src="">
pub struct ExternLink<'a> {
    filename: MaybeArc<'a, str>,
    kind: TagKind,
}

impl Linkage for ExternLink<'_> {
    fn write_contents(
        &self,
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error> {
        todo!()
    }
}

// sanity checks. fails to compile if any of these aren't met
const _: () = {
    assert!(mem::size_of::<ExternLink<'static>>() >= mem::size_of::<InlineLink>());
    assert!(mem::align_of::<ExternLink<'static>>() >= mem::align_of::<InlineLink>());
    assert!(mem::size_of::<LinkageSlot>() >= mem::size_of::<ExternLink<'static>>());
    assert!(mem::align_of::<LinkageSlot>() >= mem::align_of::<ExternLink<'static>>());
};

#[repr(align(8))]
pub struct LinkageSlot {
    data: [MaybeUninit<u8>; mem::size_of::<Either<InlineLink, ExternLink<'static>>>()],
    initialized: bool,
    linkage_vtable: &'static LinkageVTable,
}

impl LinkageSlot {
    pub const fn new_for<T: Linkage>() -> Self {
        Self {
            data: [const { MaybeUninit::uninit() }; _],
            initialized: false,
            linkage_vtable: const {
                &LinkageVTable {
                    type_id: ConstTyId::of::<T>(),
                    write_contents: imp::write_contents_shim::<T>,
                    drop: imp::drop_sized_shim::<T>,
                }
            },
        }
    }

    pub const fn as_mut_ptr(&mut self) -> *mut () {
        self.data.as_mut_ptr().cast()
    }

    pub const unsafe fn set_initialized(&mut self) {
        self.initialized = true;
    }

    pub fn write_contents(
        &self,
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error> {
        if !self.initialized {
            return Ok(());
        }

        unsafe {
            (self.linkage_vtable.write_contents)(self.data.as_ptr() as *const _, data, writer)
        }
    }
}

impl Drop for LinkageSlot {
    fn drop(&mut self) {
        if !self.initialized {
            return;
        }

        unsafe { (self.linkage_vtable.drop)(self.as_mut_ptr()) }
    }
}

pub struct AssetVTable {
    type_id: ConstTyId,
    linkage_id: ConstTyId,
    create_linkage:
        unsafe fn(data_ptr: *const (), metadata_ptr: *const (), linkage_slot: &mut LinkageSlot),
    write_fn: unsafe fn(
        data_ptr: *const (),
        metadata_ptr: *const (),
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error>,
    drop_fn: unsafe fn(*mut (), *mut ()),
    //hash: unsafe fn(*mut (), *mut (), &mut dyn core::hash::Hasher),
}

pub struct LinkageVTable {
    type_id: ConstTyId,
    write_contents:
        unsafe fn(*const (), AssetData<'_, '_>, &mut WriterRef<'_>) -> Result<(), Error>,
    drop: unsafe fn(*mut ()),
}

struct RawAsset {
    data_ptr: NonNull<()>,
    metadata_ptr: NonNull<()>,
    vtable: &'static AssetVTable,
}

pub struct DynAsset<'a> {
    raw: RawAsset,
    _marker: PhantomData<&'a ()>,
}

impl<'a> DynAsset<'a> {
    pub const fn from_ref<T>(asset: &'a T) -> Self
    where
        T: ?Sized + Asset,
    {
        unsafe {
            Self {
                raw: RawAsset::from_ref(asset),
                _marker: PhantomData,
            }
        }
    }

    pub fn owned<T>(asset: Box<T>) -> Self
    where
        T: ?Sized + OwnedAsset,
    {
        unsafe {
            Self {
                raw: RawAsset::owned(asset),
                _marker: PhantomData,
            }
        }
    }

    pub fn write(
        &self,
        writer: &mut WriterRef<'_>,
        linker_vtable: &'static LinkageVTable,
    ) -> Result<(), Error> {
        let mut linkage_slot = LinkageSlot {
            data: [MaybeUninit::uninit(); _],
            initialized: false,
            linkage_vtable: linker_vtable,
        };

        self.raw.create_linkage(&mut linkage_slot);

        linkage_slot.write_contents(AssetData { asset: self }, writer)
    }

    pub fn write_contents(&self, writer: &mut WriterRef<'_>) -> Result<(), Error> {
        unsafe {
            (self.raw.vtable.write_fn)(
                self.raw.data_ptr.as_ptr(),
                self.raw.metadata_ptr.as_ptr(),
                writer,
            )
        }
    }
}

impl RawAsset {
    const unsafe fn from_ref<T>(asset: &T) -> Self
    where
        T: ?Sized + Asset,
    {
        let (data_ptr, metadata_ptr) = utils::deconstruct(asset);

        Self {
            data_ptr,
            metadata_ptr,
            vtable: const {
                &AssetVTable {
                    type_id: ConstTyId::of::<T>(),
                    linkage_id: ConstTyId::of::<T::Linkage>(),
                    create_linkage: imp::create_linkage::<T>,
                    write_fn: imp::asset_write_shim::<T>,
                    drop_fn: imp::drop_ref_shim,
                }
            },
        }
    }

    fn create_linkage(&self, linkage_slot: &mut LinkageSlot) {
        unsafe {
            (self.vtable.create_linkage)(
                self.data_ptr.as_ptr(),
                self.metadata_ptr.as_ptr(),
                linkage_slot,
            )
        }
    }

    unsafe fn owned<T>(v: Box<T>) -> Self
    where
        T: ?Sized + Asset,
    {
        todo!()
    }
}

mod imp {
    use core::ptr;

    use crate::{
        asset::{Asset, AssetData, Linkage, LinkageSlot},
        error::Error,
        utils::{self, ConstTyId},
        writer::WriterRef,
    };

    pub(super) unsafe fn drop_sized_shim<T>(ptr: *mut ()) {
        unsafe {
            ptr::drop_in_place(ptr as *mut T);
        }
    }

    pub(super) unsafe fn write_contents_shim<T>(
        ptr: *const (),
        data: AssetData<'_, '_>,
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error>
    where
        T: Linkage,
    {
        unsafe { T::write_contents(&*(ptr as *const T), data, writer) }
    }

    pub(super) unsafe fn asset_write_shim<T>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        writer: &mut WriterRef<'_>,
    ) -> Result<(), Error>
    where
        T: ?Sized + Asset,
    {
        unsafe { T::write(&*utils::reconstruct(data_ptr, metadata_ptr), writer) }
    }

    pub(super) unsafe fn create_linkage<T>(
        data_ptr: *const (),
        metadata_ptr: *const (),
        linkage_slot: &mut LinkageSlot,
    ) where
        T: ?Sized + Asset,
    {
        unsafe {
            debug_assert!(
                linkage_slot.linkage_vtable.type_id == ConstTyId::of::<T::Linkage>(),
                "ERROR: mismatched type ids"
            );

            let ptr = utils::reconstruct(data_ptr, metadata_ptr);

            let linkage = T::create_linkage(&*ptr);

            linkage_slot
                .as_mut_ptr()
                .cast::<T::Linkage>()
                .write(linkage);
            linkage_slot.set_initialized();
        }
    }

    pub(super) unsafe fn drop_ref_shim(_: *mut (), _: *mut ()) {}
}

fn _f<L: Linkage>(a: &dyn Asset<Linkage = L>, writer: &mut WriterRef<'_>) {
    let linker = a.create_linkage();

    let asset_ref = DynAsset::from_ref(a);

    let data = AssetData {
        // file_name: None,
        asset: &asset_ref,
    };

    let _ = linker.write_contents(data, writer);
}
