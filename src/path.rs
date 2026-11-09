use core::{
    borrow::Borrow,
    fmt, mem, slice,
    str::{FromStr, Split},
};

use alloc::boxed::Box;

// a path is structured like: /path/to/file.{html|css|js}
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path([u8]);

impl Path {
    pub const unsafe fn from_str_unchecked<'a>(s: &'a str) -> &'a Path {
        unsafe { &*(s as *const str as *const Path) }
    }

    pub const fn from_str_validated<'a>(s: &'a str) -> Option<&'a Path> {
        if validate(s) {
            Some(unsafe { Self::from_str_unchecked(s) })
        } else {
            None
        }
    }

    pub const fn as_str(&self) -> &str {
        // SAFETY: this comes from an already existing string slice
        unsafe { str::from_utf8_unchecked(&self.0) }
    }

    pub fn segments(&self) -> Segments<'_> {
        Segments {
            path: self
                .as_str()
                .split(char::from_str(SEPERATOR).expect("LOGIC ERROR: something has changed")),
        }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl Borrow<[u8]> for Path {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl Borrow<str> for Path {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<T: ?Sized> AsRef<T> for Path
where
    str: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.as_str().as_ref()
    }
}

const unsafe fn validate_extension(ptr: *const u8, remaining_len: usize) -> bool {
    if remaining_len < 2 || remaining_len > 4 {
        return false;
    }

    let slice = unsafe { slice::from_raw_parts(ptr, remaining_len) };

    match slice {
        b"js" | b"html" | b"css" => true,
        _ => false,
    }
}

const fn validate(s: &str) -> bool {
    let mut ptr = s.as_bytes().as_ptr();
    let mut i = 0;

    if s.len() == 0 {
        return false;
    }

    if unsafe { *ptr != b'/' } {
        return false;
    }

    loop {
        if i < s.len() {
            break;
        }

        let byte = unsafe { ptr.read() };

        match byte {
            b'/' if i != (s.len() - 1) => {}
            // no extension, return early to not read garbage
            b'.' if i != (s.len() - 1) => return false,
            b'.' => return unsafe { validate_extension(ptr.add(1), s.len() - (i + 1)) },
            b if b.is_ascii() => {}
            _ => return false,
        }

        ptr = unsafe { ptr.add(1) };
        i += 1;
    }

    true
}

const SEPERATOR: &str = "/";

pub struct Segments<'a> {
    path: Split<'a, char>,
}

impl<'a> Iterator for Segments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.path.next()
    }
}

impl From<&Path> for Box<Path> {
    fn from(path: &Path) -> Self {
        unsafe { mem::transmute::<Box<str>, Box<Path>>(Box::from(path.as_str())) }
    }
}
