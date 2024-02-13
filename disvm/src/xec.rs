use program::Program;
use vm::VirtualMachine;

//pub type Fn = fn(&VirtualMachine, &Program);


pub fn xec(mut p: Program) {
    let mut ctx = p.context_mut();

    loop {
        // fetch
        let pc = ctx.pc.get();
        let ins = ctx.fetch();
        println!("{}: {}", pc, ins);

        // decode operands


        // run


        if ctx.ins_left == 0 {
            break;
        }
    }
}
