use std::cell::RefCell;
use module::DisModule;

pub struct PC {

}

pub struct ExecutionContext {
    pc: PC,
    module: RefCell<DisModule>
}

pub struct Program {
    pid: i32
}

