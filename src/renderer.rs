use core::{fmt::Display, hash};

use alloc::string::String;

use crate::{
    component::WriteContext, error::Error, module::IntoResult, page::Page, path::Path,
    utils::MaybeArc, writer::WriterRef,
};

pub struct Builder<'asset, C: ?Sized = dyn Display> {
    root: Option<Page<'asset, C>>,
}

pub struct Config<'a> {
    pub(crate) lang: MaybeArc<'a, str>,
}

pub(crate) fn in_html<F, R>(
    ctx: &mut WriteContext<'_, '_>,
    cfg: &Config<'_>,
    f: F,
) -> Result<(), Error>
where
    F: FnOnce(&mut WriteContext<'_, '_>) -> R,
    R: IntoResult<()>,
{
    ctx.writer.write_str("<!DOCTYPE html>\n")?;
    ctx.writer
        .write_fmt(format_args!("<html lang=\"{}\">\n", &*cfg.lang))?;

    let mut buf = String::new();

    let mut context = WriteContext {
        component_set: ctx.component_set,
        interner: ctx.interner,
        writer: WriterRef::from_ref(&mut buf),
    };

    // we can just discard the result, as it's just a unit type
    if let Err(e) = f(&mut context).into_result(&mut None) {
        ctx.writer.write_str("</html>")?;
        return Err(e);
    } else {
        ctx.writer.pull(&mut buf)?;
        Ok(())
    }
}
