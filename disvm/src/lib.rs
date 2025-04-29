#![feature(allocator_api)]
#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![feature(lazy_cell)]
#[macro_use]
extern crate enum_primitive_derive;
extern crate num_traits;
extern crate core;
extern crate modular_bitfield;
extern crate presser;
extern crate libc;
extern crate enumflags2;

pub mod opcode;
pub mod module;
pub mod consts;
pub mod mem;
mod util;
pub mod vm;
pub mod program;
pub mod xec;
mod runt;
