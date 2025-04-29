use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use enumflags2::{bitflags, make_bitflags, BitFlags};
use mem::vm_allocate;
use module::{Module, Instruction};

#[derive(Debug, Copy, Clone)]
pub struct PC {
    pub(crate) ip: usize
}

impl PC {
    pub fn set(&mut self, pc: i32) {
        self.ip = pc as usize;
    }
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
    pub(crate) ptr: *const Instruction
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Frame {
    base: u32,
    entry_ip: i32,
}


#[repr(C)]
#[derive(Debug)]
pub struct Stack {
    frames: VecDeque<Frame>,
    next_frame: Option<Frame>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            next_frame: None,
        }
    }
    
    pub fn new_frame(&mut self,  size: usize) -> &mut Frame {
        let base = vm_allocate(size);
        self.next_frame = Some(Frame {
            base: base,
            entry_ip: 0,
        });
        
        self.next_frame.as_mut().unwrap()
    }

    pub fn push_frame(&mut self) -> &mut Frame {
        
        match self.next_frame {
            Some(mut frame) =>  {
                self.frames.push_front(frame);
            }
            None => {
                panic!("no previously allocated stack frame when pushing call stack");
            }
        };
        
        self.frames.get_mut(0).unwrap()
    }

    pub fn pop_frame(&mut self) -> Frame {
        match self.frames.pop_back() {
            Some(frame) => frame,
            None => {
                panic!("no previous stack frame when popping call stack");
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramState {
    Palt,       // blocked in alt instruction
    Psend,      // waiting to send
    Precv,      // waiting to recv
    Pdebug,     // debugged
    Pready,     // ready to be scheduled
    Pexiting,   // exit because of kill or error
    Pbroken     // thread crashed
}


#[bitflags]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr( u8)]
pub enum ProgFlags { // flags for Program and ProgramGroup
    Ppropagate      = 1<<0, /* propagate exceptions within group */
    Pnotifyleader   = 1<<1, /* send exceptions to group leader */
    Prestrict       = 1<<2, /* enforce memory limits */
    Prestricted     = 1<<3,
    Pkilled         = 1<<4,
    Pprivatemem     = 1<<5, /* keep heap and stack private */
}

type Ptr = u32;

pub struct ExecutionContext {
    pub pc: PC,
    pub module: Rc<RefCell<Module>>,
    pub ins: InsPtr,
    pub ins_left: usize,

    pub MP: Ptr, // module data pointer
    pub FP: Ptr, // frame pointer
    pub SP: Ptr, // stack pointer
    pub TS: Ptr, // stack top pointer
    pub s: Ptr,
    pub d: Ptr,
    pub m: Ptr,

    pub stack: Stack
}

impl ExecutionContext {
    pub fn new(module:  Rc<RefCell<Module>>) -> Self {
        let ins_ptr = module.borrow().code.as_ptr();
        let ins_left = module.borrow().code.len();
        Self {
            pc: PC { ip: 0 },
            module: module,
            ins: InsPtr { ptr: ins_ptr },
            ins_left: ins_left,
            MP: 0,
            FP: 0,
            SP: 0,
            TS: 0,
            s: 0,
            d: 0,
            m: 0,
            stack: Stack::new(),
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
    pub ctx: Box<ExecutionContext>,
    pub flags: BitFlags<ProgFlags>,
    pub state: ProgramState,
    pub kill: Option<String>,
}

impl Program {
    pub fn new(parent: Option<&Program>, ml: &mut crate::vm::Modlink) -> Self {
        let module = ml.m.clone();
        // check parent flag status if it is being killed
        if let Some(parent) = parent  {
            parent.flags ;
            if let Some(ref kill) = parent.kill {
                // error (i think this is fatal)
            }
            if (parent.flags.contains(ProgFlags::Pkilled)) {
                // error (i think this is fatal)
            }
        }

        // initialize pid - ++pidnum
        // add to sched list
        // panic if pid is duplicate
        // put ref to module in Program (idt this is needed)
        // if no parent, add program to new group and return
        // if parent, continue
        // add program to parents groups
        // copy flags
        // copy osenv

        Self {
            pid: 0,
            ctx: Box::new(ExecutionContext::new(module)),
            flags: make_bitflags!(ProgFlags::Ppropagate),
            state: ProgramState::Pready, // TODO: maybe option this ??,
            kill: None,
        }
    }

    pub fn context(&self) -> &ExecutionContext { 
        &self.ctx
    }

    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.ctx
    }
    
    pub fn print_context(&self) {
        
    }
    
    pub fn setup_entry_frame(&mut self) {
        let ctx = self.context_mut();
        let module = ctx.module.borrow();
        let entryt = module.entry_frame_type();
        let mut stack_frame_size = if module.stack_extent < entryt.size + 16 {
            ( entryt.size + 16 ) as usize
        } else { module.stack_extent as usize };
        // create entry frame
        let frame = vm_allocate(stack_frame_size);
        ctx.SP = frame;
        ctx.TS = frame + stack_frame_size as u32;

        // copy initial frame into new frame

        // setup entry pc and frame
        ctx.pc.set(module.entry_pc);
        ctx.FP = ctx.SP;
        ctx.MP = module.origmp.unwrap();

    }
    pub fn extend_stack(&mut self) {

    }
}