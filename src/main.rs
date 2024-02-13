extern crate disvm;

use disvm::module::load_module;
use disvm::program::Program;
use disvm::xec::xec;

fn main() { // xd
    let m = load_module("./doc/inferno-os/dis/echo.dis").unwrap();
    let p = Program::new(m);
    xec(p);
    //println!("{:#?}", m);
}
