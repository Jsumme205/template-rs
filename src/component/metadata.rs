use crate::{
    class_interner::{Interner, Symbol},
    utils::MaybeArc,
};

pub struct ComponentMetadata<'meta> {
    id: Option<MaybeArc<'meta, str>>,
    class: Option<Symbol>,
}

impl ComponentMetadata<'_> {
    pub(crate) fn empty() -> Self {
        Self {
            id: None,
            class: None,
        }
    }
}

impl<'meta> ComponentMetadata<'meta> {
    pub const fn builder<'interner>(
        interner: &'interner mut Interner,
    ) -> MetadataBuilder<'meta, 'interner> {
        MetadataBuilder {
            id: None,
            class: None,
            interner,
        }
    }
}

pub struct MetadataBuilder<'meta, 'interner> {
    id: Option<MaybeArc<'meta, str>>,
    class: Option<Symbol>,
    interner: &'interner mut Interner,
}

impl<'meta, 'interner> MetadataBuilder<'meta, 'interner> {
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
        let sym = self.interner.intern(class.as_ref());
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

impl ComponentMetadata<'_> {
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn class(&self) -> Option<&str> {
        self.class.as_deref()
    }
}
