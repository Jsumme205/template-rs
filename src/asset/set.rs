use core::{cell::LazyCell, marker::PhantomData};

use hashbrown::HashSet;

use crate::asset::IntoDynAsset;

pub struct AssetSet<'asset> {
    inner: LazyCell<HashSet<super::DynAsset<'asset>>>,
}

impl<'asset> AssetSet<'asset> {
    pub const fn new() -> Self {
        Self {
            inner: LazyCell::new(Default::default),
        }
    }

    pub fn register<A, M>(&mut self, asset: A)
    where
        A: IntoDynAsset<'asset, M>,
    {
        let asset = asset.into_dyn_asset();

        self.inner.entry(asset).or_insert();
    }

    pub fn assets_of<A: ?Sized>(&self) -> AssetsOf<'_, 'asset, A> {
        AssetsOf {
            asset_set: self.inner.iter(),
            _marker: PhantomData,
        }
    }
}

pub struct AssetsOf<'set, 'asset, A: ?Sized> {
    asset_set: hashbrown::hash_set::Iter<'set, super::DynAsset<'asset>>,
    _marker: PhantomData<fn() -> &'set A>,
}

impl<'set, 'asset, A> Iterator for AssetsOf<'set, 'asset, A>
where
    A: ?Sized,
{
    type Item = &'set A;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.asset_set.next()?;
            if next.is::<A>() {
                return Some(unsafe { next.cast() });
            }
        }
    }
}
