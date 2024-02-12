#![feature(allocator_api)]

#[macro_use]
extern crate enum_primitive_derive;
extern crate num_traits;
extern crate core;
extern crate modular_bitfield;
extern crate presser;
extern crate libc;

pub mod opcode;
pub mod module;
pub mod consts;
pub mod data;
mod util;
mod vm;
mod program;
mod xec;