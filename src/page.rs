use core::{fmt::Display, hash::BuildHasher, mem};

use alloc::boxed::Box;

use crate::{
    asset::set::AssetSet,
    body::Body,
    class_interner::{Interner, Symbol},
    component::{
        OwnedComponent, WriteContext,
        set::{ComponentSet, DynComponent},
    },
    error::Error,
    head::Head,
    module::{ChangeRef, IntoResult, Module},
    path::Path,
    renderable::Renderable,
    renderer,
    router::Router,
};

macro_rules! break_if_erred {
    ($self_:ident) => {
        if $self_.result.is_err() {
            return $self_;
        }
    };
}

#[derive(PartialEq, Eq, Hash)]
pub struct PageId {
    hash: u64,
    name: Symbol,
}

impl PageId {
    pub fn new(name: &str, interner: &mut Interner) -> Self {
        let symbol = interner.intern(name);

        let hash = hashbrown::DefaultHashBuilder::default().hash_one(name);

        Self { hash, name: symbol }
    }
}

pub struct Page<'asset, C: ?Sized = dyn Display> {
    pageid: PageId,
    asset_set: AssetSet<'asset>,
    head: Head<'asset, C>,
    body: Body<'asset>,
    main: DynComponent<'asset>,
}

impl<'asset, C> Page<'asset, C>
where
    C: ?Sized + Renderable,
{
    pub(crate) fn render(
        &self,
        cfg: &renderer::Config<'_>,
        ctx: &mut WriteContext<'_, '_>,
    ) -> Result<(), Error> {
        renderer::in_html(ctx, cfg, |ctx| -> Result<(), Error> {
            self.head.render(ctx, |ctx| self.asset_set.link(ctx))?;
            self.body.render(ctx, |ctx| self.main.render(ctx))?;
            Ok(())
        })
    }
}

impl<'asset, C: ?Sized> Page<'asset, C> {
    pub fn builder<'set>(
        component_set: &'set mut ComponentSet<'asset>,
        interner: &'set mut Interner,
        router: Router<'set, 'asset>,
    ) -> Builder<'asset, 'set> {
        Builder {
            asset_set: AssetSet::default(),
            router,
            component_set,
            interner,
            main: None,
            head: None,
            body: None,
            result: Ok(()),
        }
    }
}

pub struct Builder<'asset, 'set, T = (), C: ?Sized = dyn Display> {
    asset_set: AssetSet<'asset>,
    router: Router<'set, 'asset>,
    component_set: &'set mut ComponentSet<'asset>,
    interner: &'set mut Interner,
    main: Option<T>,
    head: Option<Head<'asset, C>>,
    body: Option<Body<'asset>>,
    result: Result<(), Error>,
}

impl<'asset, 'set> Builder<'asset, 'set> {
    pub fn head<Args, M, R, C>(mut self, module: M) -> Builder<'asset, 'set, (), C>
    where
        M: Module<Args, Output = R>,
        R: IntoResult<Head<'asset, C>>,
        Args: ChangeRef,
        Self: Get<Args>,
        C: ?Sized,
    {
        let (result, head) = match self.result {
            Ok(_) => {
                let mut head = None;
                let result = module.build(self.get()).into_result(&mut head);

                (result, head)
            }
            Err(e) => (Err(e), None),
        };

        Builder {
            asset_set: self.asset_set,
            component_set: self.component_set,
            interner: self.interner,
            main: self.main,
            head,
            body: self.body,
            router: self.router,
            result,
        }
    }
}

impl<'asset, 'set, C> Builder<'asset, 'set, (), C>
where
    C: ?Sized,
{
    pub fn main<Args, M, R, T>(mut self, module: M) -> Builder<'asset, 'set, T, C>
    where
        M: Module<Args, Output = R>,
        R: IntoResult<T>,
        Self: Get<Args>,
        Args: ChangeRef,
    {
        let (result, main) = match self.result {
            Ok(_) => {
                let mut main = None;
                let result = module.build(self.get()).into_result(&mut main);
                (result, main)
            }
            Err(e) => (Err(e), None),
        };

        Builder {
            asset_set: self.asset_set,
            component_set: self.component_set,
            interner: self.interner,
            main,
            head: self.head,
            body: self.body,
            router: self.router,
            result,
        }
    }
}

#[doc(hidden)]
pub trait Get<Args: ChangeRef> {
    fn get(&mut self) -> Args::Mut<'_>;
}

// DISCLAIMER: this portion of code (the macro) was written by claude sonnet 5.5

macro_rules! impl_builder_get {
    // Entry point: generate an impl for every ordered, non-empty selection
    // of the markers below.
    () => {
        impl_builder_get!(@pick [] [] [asset component interner router]);
    };

    // @pick [chosen] [skipped] [remaining]
    // Nothing remaining: done at this level.
    (@pick [$($c:ident)*] [$($b:ident)*] []) => {};

    (@pick [$($c:ident)*] [$($b:ident)*] [$head:ident $($tail:ident)*]) => {
        // 1. Emit an impl for chosen + head.
        impl_builder_get!(@emit $($c,)* $head);
        // 2. Extend the selection with head; everything else stays available.
        impl_builder_get!(@pick [$($c)* $head] [] [$($b)* $($tail)*]);
        // 3. Move on to the next candidate at this level.
        impl_builder_get!(@pick [$($c)*] [$($b)* $head] [$($tail)*]);
    };

    (@emit $($m:ident),+) => {
        impl<'asset, 'set, T, C> Get<($(impl_builder_get!(@ty $m),)+)>
            for Builder<'asset, 'set, T, C>
        where
            C: ?Sized,
        {
            fn get(
                &mut self,
            ) -> <($(impl_builder_get!(@ty $m),)+) as ChangeRef>::Mut<'_> {
                // Direct field accesses keep the borrows disjoint.
                ($(impl_builder_get!(@field self, $m),)+)
            }
        }
    };

    // marker -> type
    (@ty asset)     => { AssetSet<'asset> };
    (@ty component) => { ComponentSet<'asset> };
    (@ty interner)  => { Interner };
    (@ty router)    => { Router<'set, 'asset> };

    // marker -> `&mut` borrow of the matching field
    (@field $s:ident, asset)     => { &mut $s.asset_set };
    (@field $s:ident, component) => { &mut *$s.component_set };
    (@field $s:ident, interner)  => { &mut *$s.interner };
    (@field $s:ident, router)    => { &mut $s.router };
}

// END AI GENERATED CODE

impl_builder_get!();

impl<'asset, 'set, T, C> Builder<'asset, 'set, T, C>
where
    C: ?Sized,
{
    pub fn asset<M, R, Args>(&mut self, module: M) -> &mut Self
    where
        M: Module<Args, Output = R>,
        Self: Get<Args>,
        Args: ChangeRef,
        R: IntoResult<()>,
    {
        break_if_erred!(self);

        let args = self.get();
        let result = module.build(args).into_result(&mut None);

        self.result = result;
        self
    }

    pub fn page<M, R, Args>(&mut self, page: M) -> &mut Self
    where
        M: Module<Args, Output = R>,
        Self: Get<Args>,
        Args: ChangeRef,
        R: IntoResult<(Page<'asset>, &'asset Path)>,
    {
        break_if_erred!(self);
        let args = self.get();
        let mut p = None;

        let result = page.build(args).into_result(&mut p);

        self.result = result;

        match p {
            Some((page, path)) => self.router.route(path, page),
            None => {}
        }
        self
    }

    pub fn build<S>(&mut self, name: S) -> Result<Page<'asset, C>, Error>
    where
        T: OwnedComponent + 'asset,
        S: AsRef<str>,
    {
        let result = mem::replace(&mut self.result, Ok(()));

        match result {
            Ok(_) => Ok(Page {
                asset_set: mem::take(&mut self.asset_set),
                head: self.head.take().ok_or(Error::const_new(
                    crate::error::ErrorKind::NotFound,
                    "Head element not built",
                ))?,
                body: self.body.take().ok_or(Error::const_new(
                    crate::error::ErrorKind::NotFound,
                    "Body element not found",
                ))?,
                main: DynComponent::from_boxed(Box::new(self.main.take().ok_or(
                    Error::const_new(crate::error::ErrorKind::NotFound, "Main not found"),
                )?)),
                pageid: PageId::new(name.as_ref(), self.interner),
            }),
            Err(e) => Err(e),
        }
    }
}
