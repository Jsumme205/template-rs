use core::{error::Error as StdError, fmt};

use alloc::boxed::Box;

#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    IoError,
    FmtError,
    ParseError,
    Other,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    payload: PayloadRepr,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args = match &self.payload {
            PayloadRepr::Dynamic(d) => {
                format_args!("ERROR kind = {:?}: {}", self.kind, *d)
            }
            PayloadRepr::Static(s) => {
                format_args!("ERROR kind = {:?} reason = {}", self.kind, *s)
            }
            PayloadRepr::None => {
                format_args!("ERROR kind = {:?}", self.kind)
            }
        };

        f.write_fmt(args)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.payload {
            PayloadRepr::Dynamic(d) => Some(&**d),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum PayloadRepr {
    Static(&'static str),
    Dynamic(Box<dyn StdError + Send + 'static>),
    None,
}

impl PayloadRepr {
    const fn as_statically_known_string(&self) -> Option<&'static str> {
        match self {
            Self::Static(s) => Some(*s),
            _ => None,
        }
    }
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

    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub const fn as_statically_known_str(&self) -> Option<&'static str> {
        self.payload.as_statically_known_string()
    }
}
