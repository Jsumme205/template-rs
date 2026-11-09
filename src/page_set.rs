use core::fmt::Display;

use crate::{page::Page, path::Path};

enum Inner<'asset, C: ?Sized = dyn Display> {
    Erased(hashbrown::HashMap<&'asset Path, Page<'asset>>),
    Typed(hashbrown::HashMap<&'asset Path, Page<'asset, C>>),
}

pub struct PageSet<'asset, C: ?Sized = dyn Display> {
    inner: Inner<'asset, C>,
}

impl<'asset, C: ?Sized> PageSet<'asset, C> {
    pub(crate) fn erase<F>(self, mut eraser: F) -> PageSet<'asset>
    where
        F: FnMut(Page<'asset, C>) -> Page<'asset>,
    {
        let inner = match self.inner {
            Inner::Erased(e) => Inner::Erased(e),
            Inner::Typed(t) => {
                let mut map = hashbrown::HashMap::with_capacity(t.len());

                for (key, value) in t {
                    unsafe { map.insert_unique_unchecked(key, eraser(value)) };
                }

                Inner::Erased(map)
            }
        };

        PageSet { inner }
    }
}
