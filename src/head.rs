use core::marker::PhantomData;

pub struct Head<'asset> {
    _marker: PhantomData<&'asset ()>,
}
