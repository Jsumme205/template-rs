use core::any::TypeId;
use core::borrow::Borrow;
use core::hash::Hash;
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop, MaybeUninit};
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::{ptr, slice};

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

impl<'a> Clone for MaybeArc<'a, str> {
    fn clone(&self) -> Self {
        match self {
            Self::Borrowed(b) => Self::Borrowed(*b),
            Self::Owned(o) => MaybeBorrowed::Owned(Arc::clone(o)),
        }
    }
}

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

impl<Ptr> Hash for MaybeBorrowed<'_, Ptr>
where
    Ptr: Deref,
    Ptr: Borrow<Ptr::Target>,
    Ptr::Target: Hash,
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<Ptr> PartialEq for MaybeBorrowed<'_, Ptr>
where
    Ptr: Deref,
    Ptr: Borrow<Ptr::Target>,
    Ptr::Target: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<Ptr> Eq for MaybeBorrowed<'_, Ptr>
where
    Ptr: Deref,
    Ptr: Borrow<Ptr::Target>,
    Ptr::Target: Eq,
{
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

impl PartialEq for ConstTyId {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for ConstTyId {}

impl Hash for ConstTyId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl PartialEq<TypeId> for ConstTyId {
    fn eq(&self, other: &TypeId) -> bool {
        self.get() == *other
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

pub struct PushBuffer<'a, T> {
    buffer: &'a mut [MaybeUninit<T>],
    len: usize,
}

impl<'a, T> PushBuffer<'a, T> {
    pub const fn new(buffer: &'a mut [MaybeUninit<T>]) -> Self {
        Self { buffer, len: 0 }
    }

    pub const fn push(&mut self, val: T) -> Result<(), T> {
        if self.len == self.buffer.len() {
            return Err(val);
        } else {
            unsafe {
                self.buffer
                    .as_mut_ptr()
                    .add(self.len)
                    .write(MaybeUninit::new(val));
                self.len += 1;
            }
            Ok(())
        }
    }

    pub fn drop_initialized(&mut self) {
        let old_len = self.len;
        self.len = 0;

        if old_len == 0 {
            return;
        }

        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                self.buffer.as_mut_ptr(),
                old_len,
            ));
        }
    }

    pub const fn into_mut_slice(self) -> &'a mut [T] {
        unsafe { slice::from_raw_parts_mut(self.buffer.as_mut_ptr().cast(), self.len) }
    }
}

const FAT_PTR_SIZE: usize = mem::size_of::<usize>() * 2;
const THIN_PTR_SIZE: usize = mem::size_of::<usize>();

pub unsafe trait Pointer {
    type Thin: ThinPointer;
    type Pointee: ?Sized;
}

pub unsafe trait ThinPointer: Clone + Copy {
    type ChangePointee<T: ?Sized>: Pointer;

    const DANGLING: Self;
}

unsafe impl ThinPointer for *const () {
    type ChangePointee<T: ?Sized> = *const T;
    const DANGLING: Self = ptr::dangling();
}

unsafe impl ThinPointer for *mut () {
    type ChangePointee<T: ?Sized> = *mut T;
    const DANGLING: Self = ptr::dangling_mut();
}

unsafe impl ThinPointer for NonNull<()> {
    type ChangePointee<T: ?Sized> = NonNull<T>;
    const DANGLING: Self = NonNull::dangling();
}

unsafe impl<T: ?Sized> Pointer for *const T {
    type Thin = *const ();

    type Pointee = T;
}

unsafe impl<T: ?Sized> Pointer for &T {
    type Thin = NonNull<()>;

    type Pointee = T;
}

unsafe impl<T: ?Sized> Pointer for &mut T {
    type Thin = NonNull<()>;

    type Pointee = T;
}

unsafe impl<T: ?Sized> Pointer for *mut T {
    type Thin = *mut ();
    type Pointee = T;
}

unsafe impl<T: ?Sized> Pointer for NonNull<T> {
    type Thin = NonNull<()>;
    type Pointee = T;
}

pub const fn deconstruct<Ptr>(ptr: Ptr) -> (Ptr::Thin, Ptr::Thin)
where
    Ptr: Pointer,
{
    // FIXME: const-hack. the "destructor" of ptr cannot be evaluated at compile time but there logically isn't any constructor
    // because it's always a non-owning reference
    let ptr = ManuallyDrop::new(ptr);

    match mem::size_of::<Ptr>() {
        FAT_PTR_SIZE => {
            let [data, metadata]: [Ptr::Thin; 2] = unsafe { mem::transmute_copy(&ptr) };
            (data, metadata)
        }
        THIN_PTR_SIZE => {
            let data: Ptr::Thin = unsafe { mem::transmute_copy(&ptr) };
            (data, Ptr::Thin::DANGLING)
        }
        _ => unsafe { core::hint::unreachable_unchecked() },
    }
}

pub const fn reconstruct<P, T>(data: P, metadata: P) -> P::ChangePointee<T>
where
    P: ThinPointer,
    T: ?Sized,
{
    match mem::size_of::<*const T>() {
        FAT_PTR_SIZE => {
            let arr = [data, metadata];
            unsafe { mem::transmute_copy(&arr) }
        }
        THIN_PTR_SIZE => {
            let ptr = &raw const data;

            unsafe { (ptr as *const P::ChangePointee<T>).read() }
        }
        _ => unsafe { core::hint::unreachable_unchecked() },
    }
}
