use core::error::Error as StdError;

use alloc::boxed::Box;

pub enum ErrorKind {
    IoError,
    FmtError,
    Other,
}

pub struct Error {
    kind: ErrorKind,
    payload: PayloadRepr,
}

enum PayloadRepr {
    Static(&'static str),
    Dynamic(Box<dyn StdError + Send + 'static>),
    None,
}

impl Error {
    pub const fn const_new(kind: ErrorKind, payload: &'static str) -> Self {
        Self {
            kind,
            payload: PayloadRepr::Static(payload),
        }
    }

    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + 'static>>,
    {
        Self {
            kind,
            payload: PayloadRepr::Dynamic(error.into()),
        }
    }
}
