use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{class_interner::ClassInterner, utils::BorrowCount};

pub struct ClassInternerCell<B> {
    inner: UnsafeCell<ClassInterner>,
    borrow_count: B,
}

pub struct Read<'a, B: BorrowCount> {
    inner: NonNull<ClassInterner>,
    count: &'a B,
}

pub struct Write<'a, B: BorrowCount> {
    inner: NonNull<ClassInterner>,
    count: &'a B,
    _marker: PhantomData<&'a mut ClassInterner>,
}

impl<B> ClassInternerCell<B>
where
    B: BorrowCount,
{
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(ClassInterner::new()),
            borrow_count: B::INIT,
        }
    }

    pub fn try_read(&self) -> Option<Read<'_, B>> {
        if self.borrow_count.increase_reads() {
            Some(Read {
                inner: unsafe { NonNull::new_unchecked(self.inner.get()) },
                count: &self.borrow_count,
            })
        } else {
            None
        }
    }

    pub fn try_write(&self) -> Option<Write<'_, B>> {
        if self.borrow_count.try_write() {
            Some(Write {
                inner: unsafe { NonNull::new_unchecked(self.inner.get()) },
                count: &self.borrow_count,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl<B: BorrowCount> Deref for Read<'_, B> {
    type Target = ClassInterner;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl<B: BorrowCount> Deref for Write<'_, B> {
    type Target = ClassInterner;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl<B: BorrowCount> DerefMut for Write<'_, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.inner.as_mut() }
    }
}

impl<B: BorrowCount> Drop for Read<'_, B> {
    fn drop(&mut self) {
        unsafe { self.count.decrease_reads() }
    }
}

impl<B: BorrowCount> Drop for Write<'_, B> {
    fn drop(&mut self) {
        unsafe { self.count.end_write() }
    }
}
