use core::{
    fmt::{self, Write},
    marker::PhantomData,
    ptr::NonNull,
};

use alloc::{boxed::Box, string::String};

use crate::{
    error::Error,
    utils::{self, ConstTyId},
};

pub trait Writer {
    fn write_str(&mut self, s: &str) -> Result<(), crate::error::Error>;

    fn pull(&mut self, s: &mut String) -> Result<(), crate::error::Error> {
        self.write_str(s.as_str())?;
        s.clear();
        Ok(())
    }

    fn write_args(&mut self, args: fmt::Arguments<'_>) -> Result<(), crate::error::Error>;
}

impl<W: ?Sized> Writer for &mut W
where
    W: Writer,
{
    fn write_args(&mut self, args: fmt::Arguments<'_>) -> Result<(), crate::error::Error> {
        W::write_args(self, args)
    }

    fn write_str(&mut self, s: &str) -> Result<(), crate::error::Error> {
        W::write_str(self, s)
    }

    fn pull(&mut self, s: &mut String) -> Result<(), crate::error::Error> {
        W::pull(self, s)
    }
}

impl<W> Writer for Box<W>
where
    W: ?Sized + Writer,
{
    fn write_args(&mut self, args: fmt::Arguments<'_>) -> Result<(), crate::error::Error> {
        W::write_args(self, args)
    }

    fn write_str(&mut self, s: &str) -> Result<(), crate::error::Error> {
        W::write_str(self, s)
    }

    fn pull(&mut self, s: &mut String) -> Result<(), crate::error::Error> {
        W::pull(self, s)
    }
}

impl Writer for alloc::string::String {
    fn write_str(&mut self, s: &str) -> Result<(), crate::error::Error> {
        self.push_str(s);
        Ok(())
    }

    fn write_args(&mut self, args: fmt::Arguments<'_>) -> Result<(), crate::error::Error> {
        self.write_fmt(args)?;
        Ok(())
    }
}

pub struct WriterRef<'w> {
    raw: RawWriter,
    _marker: PhantomData<&'w mut ()>,
}

impl WriterRef<'_> {
    pub fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
        self.raw.write_fmt(args)
    }

    pub fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.raw.write_str(s)
    }

    pub fn pull(&mut self, s: &mut String) -> Result<(), Error> {
        self.raw.pull(s)
    }
}

impl<'a> WriterRef<'a> {
    pub const fn from_ref<W>(writer: &'a mut W) -> Self
    where
        W: ?Sized + Writer,
    {
        Self {
            raw: unsafe { RawWriter::from_mut(writer) },
            _marker: PhantomData,
        }
    }
}

struct WriterVTable {
    type_id: ConstTyId,
    write_fmt: unsafe fn(*mut (), *mut (), fmt::Arguments<'_>) -> Result<(), Error>,
    write_str: unsafe fn(*mut (), *mut (), &str) -> Result<(), Error>,
    pull: unsafe fn(*mut (), *mut (), &mut String) -> Result<(), Error>,
    drop: unsafe fn(*mut (), *mut ()),
}

struct RawWriter {
    data: NonNull<()>,
    metadata: NonNull<()>,
    vtable: &'static WriterVTable,
}

impl Drop for RawWriter {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(self.data.as_ptr(), self.metadata.as_ptr()) }
    }
}

impl RawWriter {
    const unsafe fn from_mut<T>(writer: &mut T) -> Self
    where
        T: ?Sized + Writer,
    {
        let (data, metadata) = utils::deconstruct(writer);

        Self {
            data,
            metadata,
            vtable: const { todo!() },
        }
    }

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
        unsafe { (self.vtable.write_fmt)(self.data(), self.metadata(), args) }
    }

    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        unsafe { (self.vtable.write_str)(self.data(), self.metadata(), s) }
    }

    fn pull(&mut self, s: &mut String) -> Result<(), Error> {
        unsafe { (self.vtable.pull)(self.data(), self.metadata(), s) }
    }

    const fn data(&mut self) -> *mut () {
        self.data.as_ptr()
    }

    const fn metadata(&mut self) -> *mut () {
        self.metadata.as_ptr()
    }
}
