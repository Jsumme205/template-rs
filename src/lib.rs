#![no_std]

extern crate alloc;

pub(crate) mod class_interner;
pub(crate) mod page_set;

pub mod component;

pub mod error;
pub mod renderable;
pub mod utils;
pub mod writer;

pub mod asset;
pub mod body;
pub mod head;
pub mod module;
pub mod page;
pub mod path;
pub mod renderer;
pub mod router;

pub mod io;
