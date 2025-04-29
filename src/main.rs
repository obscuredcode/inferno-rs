extern crate disvm;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use disvm::module::load_module;
use disvm::program::Program;
use disvm::xec::xec;
use clap::Parser;
use clap_derive::{Args, Subcommand};
use disvm::vm::DisVM;

#[derive(Parser, Debug)]
#[clap(author, about, version)]
struct Args {
    #[command(subcommand)]
    command: Commands,
    object: PathBuf,

}

#[derive(Subcommand, Debug)]
enum Commands {
    Disasm {
        #[clap(subcommand)]
        sections: Option<DisasmSubCommands>
    }
}



#[derive(Subcommand, Debug, Clone)]
enum DisasmSubCommands {
    Code,
    Data,
    Link,
    Handler
}



fn main() { // xd yacc
    //let m = load_module("./doc/inferno-os/dis/xd.dis").unwrap();
    //let p = Program::new(Rc::new(RefCell::new(m)));

    //xec(p);

    let mut vm = DisVM::new();
    vm.init("./doc/inferno-os/dis/xd.dis");
    
    //println!("{:#?}", m);
}
