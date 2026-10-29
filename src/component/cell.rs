use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::utils::BorrowCount;

pub struct ComponentSetCell<'a, B> {
    inner: UnsafeCell<super::ComponentSet<'a>>,
    borrow_count: B,
}

pub struct Read<'r, 'a, B: BorrowCount> {
    cell: &'r ComponentSetCell<'a, B>,
}

pub struct Write<'r, 'a, B: BorrowCount> {
    cell: &'r ComponentSetCell<'a, B>,
    _marker: PhantomData<&'a mut super::ComponentSet<'a>>,
}

impl<'a, B> ComponentSetCell<'a, B>
where
    B: BorrowCount,
{
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(super::ComponentSet::new()),
            borrow_count: B::INIT,
        }
    }

    pub fn try_read<'r>(&'r self) -> Option<Read<'r, 'a, B>> {
        if self.borrow_count.increase_reads() {
            Some(Read { cell: self })
        } else {
            None
        }
    }

    pub fn try_write<'r>(&'r self) -> Option<Write<'r, 'a, B>> {
        if self.borrow_count.try_write() {
            Some(Write {
                cell: self,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.try_read().unwrap().len()
    }
}

impl<'a, B> Deref for Read<'_, 'a, B>
where
    B: BorrowCount,
{
    type Target = super::ComponentSet<'a>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.cell.inner.get() }
    }
}

impl<'a, B> Deref for Write<'_, 'a, B>
where
    B: BorrowCount,
{
    type Target = super::ComponentSet<'a>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.cell.inner.get() }
    }
}

impl<'a, B> DerefMut for Write<'_, 'a, B>
where
    B: BorrowCount,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.cell.inner.get() }
    }
}

impl<'a, B: BorrowCount> Drop for Read<'_, 'a, B> {
    fn drop(&mut self) {
        unsafe { self.cell.borrow_count.decrease_reads() }
    }
}

impl<'a, B: BorrowCount> Drop for Write<'_, 'a, B> {
    fn drop(&mut self) {
        unsafe { self.cell.borrow_count.end_write() }
    }
}
