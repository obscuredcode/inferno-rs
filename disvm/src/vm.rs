use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Error;
use std::ptr::null;
use std::rc::Rc;
use enumflags2::make_bitflags;
use libc::Elf64_Sxword;
use module::{load_module, Module};
use program::*;
use xec::xec;

pub(crate) struct Modlink {
    pub m: Rc<RefCell<Module>>,

}

struct Scheduler {
    // queue
    queue: Vec<Program>
}

impl Scheduler {
    pub fn new() -> Self {
        Self { queue: Vec::new()
            
        }
    }
    fn aquire() {

    }
    fn release() {

    }

    pub fn schedule(&mut self, prog: Program) {
        // TODO: move into vm loop and dispatch multithreading
        self.queue.push(prog);
    }
    
    // TODO: just for now
    pub fn start(&mut self) {
        let prog = self.queue.pop().unwrap();
        xec(prog);
    }
}

type Pid = i32;

pub struct DisVM {
    sched: Scheduler,
    progs: HashMap<Pid,Program>,
    // have a module loader or just end up putting modules here?
}


impl DisVM {
    pub fn new() -> Self {
        Self {
            sched: Scheduler::new(),
            progs: Default::default(),
        }
    }
    pub fn init(&mut self, init_prog: &str) {
        // load modules
        //  - modinit()
        // load init_prog
        let init_mod = match load_module(init_prog) {
            Ok(module) => module,
            Err(_) => {
                panic!("Failed to load initial program {}", init_prog);
            }
        };
        // schedule init_prog
        self.schedmod(init_mod);
        // start vm loop
        self.sched.start();
    }

    // TODO: should this be in Program?
    fn newprog(&mut self, parent: Option<&mut Program>, m: &mut Modlink) -> Program {
        Program {
            pid: 0,
            ctx: Box::new(ExecutionContext::new(m.m.clone())),
            flags: make_bitflags!(ProgFlags::Ppropagate),
            state: ProgramState::Pready,
            kill: None,
        }
    }

    // scheduler mutex?
    pub fn schedmod(&mut self, m: Module) {
        // create Modlink for module
        //  - mklinkmod(m,0)
        // copy original unchanged module data into MP
        if let Some(origmp) = m.origmp {

        }

        let mut p = self.newprog(None, &mut Modlink {
            m: Rc::new(RefCell::new(m)),
        });

        // new stack and setup entry pc and frame
        p.setup_entry_frame();


        self.schedule(p)
    }


    // schedule thread
    pub fn schedule(&mut self, prog: Program) {
        self.sched.schedule(prog)
    }

    pub fn spawn(&mut self, parent_pid: Pid, m: Module) {
        let parent = &self.progs[&parent_pid];
        let spawned = Program::new(Some(parent), &mut Modlink {
            m: Rc::new(RefCell::new(m)),
        });
        self.schedule(spawned);
    }
}
