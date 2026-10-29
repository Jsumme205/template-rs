use core::{alloc::Layout, marker::PhantomData, ptr::NonNull};

use alloc::vec::Vec;

use crate::{
    error::Error,
    utils::{ConstTyId, MaybeArc},
    writer::Writer,
};

pub struct AssetContext<'a, 'path> {
    paths: &'a mut Vec<MaybeArc<'path, str>>,
}

pub trait Asset {
    fn write_inline(&self, writer: &mut dyn Writer) -> Result<(), Error>;

    fn write_file(&self, writer: &mut AssetContext<'_, '_>) -> Result<(), Error>;
}

pub(crate) struct AssetVTable {
    layout: Layout,
    type_id: ConstTyId,
    write_inline: unsafe fn(*const (), *const (), &mut dyn Writer) -> Result<(), Error>,
    write_file: unsafe fn(*const (), *const (), &mut AssetContext<'_, '_>) -> Result<(), Error>,
    drop_fn: unsafe fn(*mut (), *mut (), Layout),
}

struct RawDynAsset {
    data_ptr: NonNull<()>,
    metadata_ptr: NonNull<()>,
    vtable: &'static AssetVTable,
}

impl RawDynAsset {

    unsafe fn write_inline(&self, writer: &mut dyn Writer) -> R

}

impl Drop for RawDynAsset {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop_fn)(
                self.data_ptr.as_ptr(),
                self.metadata_ptr.as_ptr(),
                self.vtable.layout,
            )
        }
    }
}

pub struct DynAsset<'asset> {
    _marker: PhantomData<&'asset (dyn Asset + 'asset)>,
}
