use crate::{
    class_interner::{Symbol, cell::ClassInternerCell},
    utils::{BorrowCount, MaybeArc},
};

pub struct ComponentMetadata<'meta> {
    id: Option<MaybeArc<'meta, str>>,
    class: Option<Symbol>,
}

impl<'meta> ComponentMetadata<'meta> {
    pub const fn builder<'interner, B>(
        interner: &'interner ClassInternerCell<B>,
    ) -> MetadataBuilder<'meta, 'interner, B> {
        MetadataBuilder {
            id: None,
            class: None,
            interner,
        }
    }
}

pub struct MetadataBuilder<'meta, 'interner, B> {
    id: Option<MaybeArc<'meta, str>>,
    class: Option<Symbol>,
    interner: &'interner ClassInternerCell<B>,
}

impl<'meta, 'interner, B> MetadataBuilder<'meta, 'interner, B>
where
    B: BorrowCount,
{
    pub fn finish(&mut self) -> ComponentMetadata<'meta> {
        ComponentMetadata {
            id: self.id.take(),
            class: self.class.take(),
        }
    }

    pub fn id(&mut self, id: impl Into<MaybeArc<'meta, str>>) -> &mut Self {
        self.id = Some(id.into());
        self
    }

    pub fn class(&mut self, class: impl AsRef<str>) -> &mut Self {
        let sym = self.interner.try_write().unwrap().intern(class.as_ref());
        self.class = Some(sym);
        self
    }
}

pub trait HasComponentMetadata<'meta> {
    fn metadata(&self) -> &ComponentMetadata<'meta>;
}

impl core::fmt::Display for ComponentMetadata<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(id) = &self.id {
            f.write_fmt(format_args!("id=\"{}\"", &**id))?;
        }

        if let Some(class) = &self.class {
            f.write_fmt(format_args!(" class=\"{}\"", &**class))?;
        }

        Ok(())
    }
}
