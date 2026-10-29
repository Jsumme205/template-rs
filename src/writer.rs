use core::fmt::{self, Write};

use alloc::string::String;

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

impl Writer for alloc::string::String {
    fn write_str(&mut self, s: &str) -> Result<(), crate::error::Error> {
        self.push_str(s);
        Ok(())
    }

    fn write_args(&mut self, args: fmt::Arguments<'_>) -> Result<(), crate::error::Error> {
        self.write_fmt(args).unwrap();
        Ok(())
    }
}
