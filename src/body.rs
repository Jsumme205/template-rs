use core::marker::PhantomData;

use crate::{component::WriteContext, error::Error};

pub struct Body<'asset> {
    _marker: PhantomData<&'asset ()>,
}

impl<'asset> Body<'asset> {
    pub fn render(
        &self,
        ctx: &mut WriteContext<'_, '_>,
        in_scope: impl FnOnce(&mut WriteContext<'_, '_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        todo!()
    }
}
