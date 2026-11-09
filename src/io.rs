use alloc::string::String;

/// Trait to help with I/O operations
pub trait IO {
    type Error;
    type Path: ?Sized;
    type Handle: IoHandle;

    fn copy(&self, src: &Self::Path, dst: &Self::Path) -> Result<(), Self::Error>;

    fn read_to_string(&self, path: &Self::Path) -> Result<String, Self::Error>;

    fn open(&self, path: &Self::Path) -> Result<Self::Handle, Self::Error>;
}

pub trait IoHandle {
    type Error;

    fn write(&mut self, src: &[u8]) -> Result<usize, Self::Error>;

    fn write_all(&mut self, src: &[u8]) -> Result<(), Self::Error>;
}
