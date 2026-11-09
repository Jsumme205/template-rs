use crate::error::Error;

pub trait Module<Args: ChangeRef + ?Sized> {
    type Output;

    fn build(self, args: Args::Mut<'_>) -> Self::Output;
}

pub trait ChangeRef {
    type Mut<'a>
    where
        Self: 'a;
}

impl ChangeRef for () {
    type Mut<'a>
        = ()
    where
        Self: 'a;
}

impl<F, R> Module<()> for F
where
    F: FnOnce() -> R,
{
    type Output = R;

    fn build(self, _args: <() as ChangeRef>::Mut<'_>) -> Self::Output {
        self()
    }
}

macro_rules! impl_change_ref {
    () => {};
    ($head:ident $(, $tail:ident)* $(,)?) => {
        impl<$head, $($tail),*> ChangeRef for ($head, $($tail,)*) {
            type Mut<'a>
                = (&'a mut $head, $(&'a mut $tail,)*)
            where
                Self: 'a;
        }

        #[automatically_derived]
        impl<F, R, $head, $($tail),*> Module<($head, $($tail,)*)> for F
        where
            F: FnOnce(&mut $head, $(&mut $tail,)*) -> R
        {
            type Output = R;

            #[allow(non_snake_case)]
            fn build(self, ($head, $($tail,)*) :<($head, $($tail,)*) as ChangeRef>::Mut<'_>) -> Self::Output {
                self($head, $($tail,)*)
            }
        }

        // Recurse on the tail: generates the impl for the next smaller arity.
        impl_change_ref!($($tail),*);
    };
}

impl_change_ref!(
    T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
);

pub trait IntoResult<T> {
    fn into_result(self, slot: &mut Option<T>) -> Result<(), Error>;
}

impl<T> IntoResult<T> for T {
    fn into_result(self, v: &mut Option<T>) -> Result<(), Error> {
        *v = Some(self);
        Ok(())
    }
}

impl<T, E> IntoResult<T> for Result<T, E>
where
    E: Into<Error>,
{
    fn into_result(self, slot: &mut Option<T>) -> Result<(), Error> {
        match self {
            Self::Ok(v) => {
                *slot = Some(v);
                Ok(())
            }
            Self::Err(e) => Err(e.into()),
        }
    }
}
