use core::{fmt::Display, mem};

use alloc::{sync::Arc, vec::Vec};

use crate::{
    component::{
        Component, WriteContext,
        set::{ComponentRange, ComponentRangeBuilder, ComponentSet},
    },
    error::Error,
    renderable::Renderable,
    utils::{BorrowCount, MaybeArc},
    writer::WriterRef,
};

pub struct Head<'asset, C: ?Sized = dyn Display> {
    metas: HeadMetadata<'asset>,
    contents: Option<MaybeArc<'asset, C>>,
    lang: MaybeArc<'asset, str>,
    head_components: ComponentRange<'asset>,
    // _marker: PhantomData<&'asset ()>,
}

pub struct HeadBuilder<'set, 'asset, C: ?Sized = dyn Display> {
    metas: HeadMetadataBuilder<'asset>,
    contents: Option<MaybeArc<'asset, C>>,
    lang: Option<MaybeArc<'asset, str>>,
    components: ComponentRangeBuilder<'asset, 'set>,
}

impl<'asset> Head<'asset> {
    pub fn builder<'set, B>(
        component_set: &'set mut ComponentSet<'asset>,
    ) -> HeadBuilder<'set, 'asset, B>
    where
        B: BorrowCount,
    {
        HeadBuilder {
            metas: HeadMetadata::builder(),
            contents: None,
            lang: None,
            components: ComponentRange::builder(component_set),
        }
    }
}

impl<'asset, C> Head<'asset, C>
where
    C: ?Sized + Renderable,
{
    pub fn render(
        &self,
        ctx: &mut WriteContext<'_, '_>,
        in_scope: impl FnOnce(&mut WriteContext<'_, '_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.metas.write(&mut ctx.writer)
    }
}

pub struct HeadMetadataBuilder<'a> {
    inner: Vec<(Key<'a>, Option<Value<'a>>)>,
}

pub struct HeadMetadata<'a> {
    inner: Arc<[(Key<'a>, Option<Value<'a>>)]>,
}

impl<'a> HeadMetadata<'a> {
    pub const fn builder() -> HeadMetadataBuilder<'a> {
        HeadMetadataBuilder { inner: Vec::new() }
    }

    fn write(&self, writer: &mut WriterRef<'_>) -> Result<(), Error> {
        todo!()
    }
}

impl<'a> HeadMetadataBuilder<'a> {
    pub fn insert_pair(
        &mut self,
        key: impl Into<Key<'a>>,
        value: impl Into<Value<'a>>,
    ) -> &mut Self {
        self.inner.push((key.into(), Some(value.into())));
        self
    }

    pub fn insert(&mut self, key: impl Into<Key<'a>>) -> &mut Self {
        self.inner.push((key.into(), None));
        self
    }

    pub fn finish(&mut self) -> HeadMetadata<'a> {
        HeadMetadata {
            inner: Arc::from(mem::take(&mut self.inner)),
        }
    }
}

#[derive(Hash, PartialEq, Eq)]
pub struct Key<'a> {
    key: MaybeArc<'a, str>,
    value: MaybeArc<'a, str>,
}

impl<'a, K, V> From<(K, V)> for Key<'a>
where
    K: Into<MaybeArc<'a, str>>,
    V: Into<MaybeArc<'a, str>>,
{
    fn from((key, value): (K, V)) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

pub struct Value<'a> {
    key_of_value: MaybeArc<'a, str>,
    value_of_values: Values<'a>,
}

enum Values<'a> {
    Single(MaybeArc<'a, str>, MaybeArc<'a, str>),
    Pair {
        k1: MaybeArc<'a, str>,
        v1: MaybeArc<'a, str>,
        k2: MaybeArc<'a, str>,
        v2: MaybeArc<'a, str>,
    },
    Many(Vec<(MaybeArc<'a, str>, MaybeArc<'a, str>)>),
}

impl<'a, KK, K, V, const N: usize> From<(KK, [(K, V); N])> for Value<'a>
where
    KK: Into<MaybeArc<'a, str>>,
    K: Into<MaybeArc<'a, str>>,
    V: Into<MaybeArc<'a, str>>,
{
    fn from((key, values): (KK, [(K, V); N])) -> Self {
        let mut values = values.into_iter();

        let values = match N {
            1 => {
                // SAFETY: array length is 1, so this always succeeds
                let (k, v) = unsafe { values.next().unwrap_unchecked() };
                Values::Single(k.into(), v.into())
            }
            2 => {
                // SAFETY: array length is 2. we can safely unwrap this no matter what
                let (k1, v1) = unsafe { values.next().unwrap_unchecked() };
                let (k2, v2) = unsafe { values.next().unwrap_unchecked() };
                Values::Pair {
                    k1: k1.into(),
                    v1: v1.into(),
                    k2: k2.into(),
                    v2: v2.into(),
                }
            }
            _ => Values::Many(values.map(|(k, v)| (k.into(), v.into())).collect()),
        };

        Self {
            key_of_value: key.into(),
            value_of_values: values,
        }
    }
}

fn _f() {
    let m = HeadMetadata::builder()
        .insert(("charset", "UTF-8"))
        .insert_pair(
            ("name", "viewport"),
            (
                "content",
                [("width", "device-width"), ("initial-scale", "1.0")],
            ),
        )
        .finish();
}

pub trait HeadComponent: Component {}
