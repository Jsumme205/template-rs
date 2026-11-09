use core::fmt::Display;

use crate::{page::Page, path::Path};

pub struct Router<'map, 'asset, C: ?Sized = dyn Display> {
    inner: &'map mut hashbrown::HashMap<&'asset Path, Page<'asset, C>>,
}

impl<'map, 'asset, C: ?Sized> Router<'map, 'asset, C> {
    pub(crate) const fn new(
        inner: &'map mut hashbrown::HashMap<&'asset Path, Page<'asset, C>>,
    ) -> Self {
        Self { inner }
    }

    pub fn route(&mut self, path: &'asset Path, page: Page<'asset, C>) {
        self.inner.insert(path, page);
    }
}
