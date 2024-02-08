#[macro_use]
extern crate enum_primitive_derive;
extern crate num_traits;
extern crate modular_bitfield;

mod disvm;

use crate::disvm::module::load_module;

fn main() {
    load_module("./doc/inferno-os/dis/echo.dis").unwrap();
}
