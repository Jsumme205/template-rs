use core::sync::atomic::AtomicUsize;

use crate::{
    asset::set::AssetSet, class_interner::cell::ClassInternerCell,
    component::cell::ComponentSetCell, error::Error, head::Head, writer::Writer,
};

pub struct Config {}

pub struct Renderer<'asset, 'set, APP, B = AtomicUsize> {
    // registration set stuff
    asset_set: &'set AssetSet<'asset>,
    component_set: &'set ComponentSetCell<'asset, B>,
    interner: &'set ClassInternerCell<B>,

    config: Config,

    head: Head<'asset>,
    root: APP,
}

impl<'asset, 'set, B> Renderer<'asset, 'set, B> {
    pub fn render<W>(&mut self, writer: &mut W) -> Result<(), Error>
    where
        W: Writer + ?Sized,
    {
        Ok(())
    }
}
