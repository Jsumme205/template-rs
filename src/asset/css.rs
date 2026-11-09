use crate::{
    asset::{Asset, Either, ExternLink, InlineLink},
    class_interner::Symbol,
    error::Error,
    utils::MaybeArc,
    writer::WriterRef,
};

pub struct Css<'a> {
    inner: Inner<'a>,
}

impl<'a> Asset for Css<'a> {
    type Linkage = Either<InlineLink, ExternLink<'a>>;

    fn create_linkage(&self) -> Self::Linkage {
        match &self.inner {
            Inner::Inlined { .. } => Either::Left(InlineLink {
                kind: super::TagKind::Stylesheet,
            }),
            Inner::Extern { file_name } => Either::Right(ExternLink {
                filename: MaybeArc::clone(&file_name),
                kind: super::TagKind::Stylesheet,
            }),
        }
    }

    fn write(&self, writer: &mut crate::writer::WriterRef<'_>) -> Result<(), crate::error::Error> {
        self.inner.write(writer)
    }
}

enum Inner<'a> {
    Inlined { data: Data, selector: Selector<'a> },
    Extern { file_name: MaybeArc<'a, str> },
}

impl Inner<'_> {
    fn write(&self, writer: &mut WriterRef<'_>) -> Result<(), Error> {
        match self {
            // nothing to write.
            Inner::Extern { .. } => Ok(()),
            Inner::Inlined { data, selector } => todo!(),
        }
    }
}

struct Data {}

pub enum Selector<'a> {
    ById(MaybeArc<'a, str>),
    ByClass(Symbol),
    ByElement(Element),
}

pub enum Element {
    H1,
    H2,
    // etc...
}
