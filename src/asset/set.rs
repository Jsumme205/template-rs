use alloc::vec::Vec;

use crate::{
    asset::{DynAsset, LinkageVTable},
    component::WriteContext,
    error::Error,
};

#[derive(Default)]
pub struct AssetSet<'asset> {
    inner: Vec<(DynAsset<'asset>, &'static LinkageVTable)>,
}

impl<'asset> AssetSet<'asset> {
    pub(crate) fn link(&self, ctx: &mut WriteContext<'_, '_>) -> Result<(), Error> {
        self.inner
            .iter()
            .try_for_each(|(asset, vtable)| asset.write(&mut ctx.writer, *vtable))
    }
}
