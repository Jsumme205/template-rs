use core::{
    borrow::Borrow,
    cell::LazyCell,
    hash::Hash,
    mem::MaybeUninit,
    ops::Deref,
    ptr::{self, NonNull},
    slice,
};

use alloc::boxed::Box;

pub mod cell;

pub struct ClassInterner {
    pool: Pool,
    map: LazyCell<hashbrown::HashMap<Interned, u32>>,
    next_id: u32,
}

// SAFETY: we use typical XOR mutablility rules
unsafe impl Send for ClassInterner {}
unsafe impl Sync for ClassInterner {}

impl ClassInterner {
    pub(crate) const fn new() -> Self {
        Self {
            pool: Pool::new(),
            map: LazyCell::new(|| hashbrown::HashMap::new()),
            next_id: 0,
        }
    }

    pub(crate) fn intern(&mut self, symbol: &str) -> Symbol {
        let raw = self.map.raw_entry_mut();
        let (i, &mut next_id) = raw.from_key(symbol).or_insert_with(|| {
            let ptr = self.pool.intern(symbol);
            let interned = Interned {
                ptr,
                len: symbol.len(),
            };

            let next_id = self.next_id;
            self.next_id += 1;
            (interned, next_id)
        });

        Symbol(next_id, NonNull::from_ref(i.as_slice()))
    }
}

pub struct Symbol(u32, NonNull<str>);

// SAFETY: this is just a wrapper around an integer and an always valid pointer to a string, as it never gets dropped
unsafe impl Send for Symbol {}
unsafe impl Sync for Symbol {}

impl Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { self.1.as_ref() }
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Symbol {}

impl Hash for Symbol {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

struct Interned {
    ptr: NonNull<u8>,
    len: usize,
}

impl Borrow<str> for Interned {
    fn borrow(&self) -> &str {
        self.as_slice()
    }
}

impl Interned {
    const fn as_slice(&self) -> &str {
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(self.ptr.as_ptr(), self.len)) }
    }
}

impl Hash for Interned {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl PartialEq for Interned {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl Eq for Interned {}

const BLOCK_SIZE: usize = 1024 * 2;

// currently this is a leaking design, may change in the future
struct Pool {
    current_block: NonNull<Block>,
    remaining: usize,
}

impl Pool {
    const fn should_grow(&self, len: usize) -> bool {
        self.remaining.checked_sub(len).is_none()
    }

    const fn new() -> Self {
        Self {
            current_block: NonNull::dangling(),
            remaining: 0,
        }
    }

    fn intern(&mut self, s: &str) -> NonNull<u8> {
        if self.should_grow(s.len()) {
            self.grow();
        }

        unsafe {
            let data_ptr = self.data_ptr();
            ptr::copy_nonoverlapping(s.as_ptr(), data_ptr.as_ptr(), s.len());
            self.remaining -= s.len();

            data_ptr
        }
    }

    fn grow(&mut self) {
        let next_block = Box::into_non_null(Box::new(Block {
            prev: Some(self.current_block),
            next: None,
            data: [MaybeUninit::uninit(); BLOCK_SIZE],
        }));

        unsafe {
            self.current_block.as_mut().next = Some(next_block);
            self.current_block = next_block;
            self.remaining = BLOCK_SIZE;
        }
    }

    unsafe fn data_ptr(&mut self) -> NonNull<u8> {
        unsafe {
            let ptr = self
                .current_block
                .as_mut()
                .data
                .as_mut_ptr_range()
                .end
                .sub(self.remaining);
            NonNull::new_unchecked(ptr.cast())
        }
    }
}

struct Block {
    prev: Option<NonNull<Block>>,
    next: Option<NonNull<Block>>,
    data: [MaybeUninit<u8>; BLOCK_SIZE],
}
