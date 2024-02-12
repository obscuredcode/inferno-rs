extern crate disvm;

use disvm::module::load_module;
use disvm::module::DisModule;

fn main() { // xd
    let m = load_module("./doc/inferno-os/dis/echo.dis").unwrap();
    let mut pc = 0;
    for i in &m.code {
       //println!("{pc}: {}", i);
        pc += 1;
    }
    //println!("{:#?}", m);
}
