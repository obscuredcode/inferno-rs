use std::cell::RefCell;
use module::{DisModule, Instruction};

#[derive(Debug, Copy, Clone)]
pub struct PC {
    ip: usize
}

impl PC {
    pub fn get(&self) -> usize {
        self.ip
    }
    pub fn next(&mut self) -> usize {
        let prev = self.ip;
        self.ip += 1;
        prev
    }
}


#[derive(Copy, Clone)]
pub struct InsPtr {
    ptr: *const Instruction
}

pub struct ExecutionContext {
    pub pc: PC,
    pub module: RefCell<DisModule>,
    pub ins: InsPtr,
    pub ins_left: usize,
}

impl ExecutionContext {
    pub fn new(module: DisModule) -> Self {
        let ins_ptr = module.code.as_ptr();
        let ins_left = module.code.len();
        Self {
            pc: PC { ip: 0 },
            module: RefCell::new(module),
            ins: InsPtr { ptr: ins_ptr },
            ins_left: ins_left
        }
    }
    pub fn get_ins(&self) -> &Instruction {
        unsafe { &*self.ins.ptr.add(self.pc.get()) }
    }
    pub fn fetch(&mut self) -> Instruction {
        let ins = self.get_ins().clone();
        self.ins_left -= 1;
        self.pc.next();
        ins
    }
}

pub struct Program {
    pub pid: i32,
    pub ctx: Box<ExecutionContext>
}

impl Program {
    pub fn new(module: DisModule) -> Self {
        Self {
            pid: 0,
            ctx: Box::new(ExecutionContext::new(module)),
        }
    }

    pub fn context(&self) -> &ExecutionContext {
     &self.ctx
    }

    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.ctx
    }
}