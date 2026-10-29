use core::any::TypeId;
use core::marker::PhantomData;
use core::mem;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{boxed::Box, rc::Rc, string::String, sync::Arc};

pub enum MaybeBorrowed<'a, Ptr>
where
    Ptr: Deref,
{
    Borrowed(&'a Ptr::Target),
    Owned(Ptr),
}

pub type MaybeBoxed<'a, T> = MaybeBorrowed<'a, Box<T>>;
pub type MaybeArc<'a, T> = MaybeBorrowed<'a, Arc<T>>;

impl<'a, Ptr> From<&'a Ptr::Target> for MaybeBorrowed<'a, Ptr>
where
    Ptr: Deref,
{
    fn from(value: &'a Ptr::Target) -> Self {
        Self::Borrowed(value)
    }
}

impl<Ptr> From<String> for MaybeBorrowed<'_, Ptr>
where
    Ptr: From<String> + Deref<Target = str>,
{
    fn from(value: String) -> Self {
        Self::Owned(Ptr::from(value))
    }
}

impl<Ptr> Deref for MaybeBorrowed<'_, Ptr>
where
    Ptr: Deref,
{
    type Target = Ptr::Target;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(o) => o.deref(),
        }
    }
}

fn _f() {
    let _v = MaybeBoxed::from(String::new());
    let _v = MaybeArc::from(String::new());
    let _v = MaybeBorrowed::<'_, Rc<str>>::from(String::new());
}

#[derive(Clone, Copy)]
pub(crate) struct ConstTyId {
    get: fn() -> TypeId,
}

impl ConstTyId {
    pub const fn of<T: ?Sized>() -> Self {
        Self { get: self::of::<T> }
    }

    pub fn get(&self) -> TypeId {
        (self.get)()
    }
}

fn of<T>() -> TypeId
where
    T: ?Sized,
{
    trait NonStaticAny {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static;
    }

    impl<T: ?Sized> NonStaticAny for PhantomData<T> {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static,
        {
            TypeId::of::<T>()
        }
    }

    let ph = PhantomData::<T>;
    NonStaticAny::get_type_id(unsafe {
        mem::transmute::<&dyn NonStaticAny, &(dyn NonStaticAny + 'static)>(&ph)
    })
}

pub trait BorrowCount {
    const INIT: Self;

    fn increase_reads(&self) -> bool;

    unsafe fn decrease_reads(&self);

    fn try_write(&self) -> bool;

    unsafe fn end_write(&self);
}

impl BorrowCount for AtomicUsize {
    const INIT: Self = Self::new(0);

    unsafe fn decrease_reads(&self) {
        self.fetch_sub(1, Ordering::Release);
    }

    fn increase_reads(&self) -> bool {
        self.fetch_add(1, Ordering::Acquire) != (usize::MAX - 1)
    }

    fn try_write(&self) -> bool {
        self.compare_exchange(0, usize::MAX, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    unsafe fn end_write(&self) {
        self.store(0, Ordering::Release);
    }
}

pub trait Boxed: Sized {
    fn boxed(self) -> Box<Self>;
}

impl<T> Boxed for T {
    fn boxed(self) -> Box<Self> {
        self.into()
    }
}
