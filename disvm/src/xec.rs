use mem::{vm_types, ModuleHeap, VmPtr};
use module::{load_module, Instruction, MiddleOpMode, MiddleOperandData, Module, OpAddressMode, SourceDestOpMode, SourceDestOperandData};
use opcode::Opcode;
use program::Program;
use vm::DisVM;

//pub type Fn = fn(&VirtualMachine, &Program);

// lea
// load address of operand (what's actually in the register. this is still an address only
// relevant to the vm)
// s is operand
// d is destination for operand address
// *(d as *address) = s as *address

// movpc
// move pc[i] to dest
// s is address of instruction index i
// d is destination for address of instruction
// i = *(s as *word)
// *(d as *address) = &mod.instructions[i]

// movp
// move pointer
// s is src address for pointer
// d is dest address for pointer
// sv =  *(s as *pointer)
// dv =  *(d as *pointer)
// if sv is not null, count refs
// *(d as *pointer) = sv
// destroy dv


// movm
// move memory for addresses
// s is src address
// d is dest address
// *(m as *word) is no bytes
// memmove(d, s, W(m));

type Ptr = u32;
pub fn xec(mut p: Program) {
    let mut ctx = p.context_mut();
    let module = ctx.module.clone();
    // check if program need to be killed, if so kill

    loop {
        // fetch
        let m = &module.borrow_mut();
        let pc = &mut ctx.pc.get();
        let ins = ctx.fetch();
        println!("{}: {}", pc, ins);
        let mut imm: u32 = 0;
        let mut m_imm: u16 = 0;
        let mut ind: Ptr = 0;
        let mut dind: Ptr = 0;
        let mut no_mid: bool = true;

        // decode operands

        match ins.address.middle_op_mode() {
            MiddleOpMode::None => {}
            MiddleOpMode::SmallImmediate => {
                ctx.m = ins.middle_data.imm as u32;
            }
            MiddleOpMode::SmallOffsetIndFP => {
                ctx.m = ctx.FP + ins.middle_data.ind as u32;
            }
            MiddleOpMode::SmallOffsetIndMP => {
                ctx.m = ctx.MP + ins.middle_data.ind as u32;
            }
        }

        match ins.address.source_op_mode() {
            SourceDestOpMode::OffsetIndMP => {
                ctx.s = ctx.MP + ins.src_data.ind as u32;
            }
            SourceDestOpMode::OffsetIndFP => {
                ctx.s = ctx.FP + ins.src_data.ind as u32;
            }
            SourceDestOpMode::WordImmediate => {}
            SourceDestOpMode::None => {}
            SourceDestOpMode::DoubleIndMP => {}
            SourceDestOpMode::DoubleIndFP => {}
            SourceDestOpMode::Reserved1 => {}
            SourceDestOpMode::Reserved2 => {}
        }
        match ins.address.dest_op_mode() {
            SourceDestOpMode::OffsetIndMP => {
                ctx.d = ctx.MP + ins.src_data.ind as u32;
            }
            SourceDestOpMode::OffsetIndFP => {
                ctx.d = ctx.FP + ins.src_data.ind as u32;
            }
            SourceDestOpMode::WordImmediate => {}
            SourceDestOpMode::None => {}
            SourceDestOpMode::DoubleIndMP => {}
            SourceDestOpMode::DoubleIndFP => {}
            SourceDestOpMode::Reserved1 => {}
            SourceDestOpMode::Reserved2 => {}
        }


        // execute operand
        match ins.opcode {
            Opcode::INOP => {}
            Opcode::IALT => {}
            Opcode::INBALT => {}
            Opcode::IGOTO => {}
            Opcode::ICALL => {}
            Opcode::IFRAME => {}
            Opcode::ISPAWN => {}
            Opcode::IRUNT => {}
            Opcode::ILOAD => {
                let module_name = unsafe {
                    match m.module_heap {
                        None => { panic!("") }
                        Some(ref h) => 
                            {
                                let str_pr = *(ctx.s as *const u32);
                                let string = h.read(&VmPtr::<vm_types::String>::new(str_pr));
                                
                                String::from_raw_parts(string.data as *mut u8, string.len as usize, string.max as usize)
                            }
                    }
                };
                
                let loaded_module = load_module(module_name.as_str());
            }
            Opcode::IMCALL => {}
            Opcode::IMSPAWN => {}
            Opcode::IMFRAME => {}
            Opcode::IRET => {}
            Opcode::IJMP => {}
            Opcode::ICASE => {}
            Opcode::IEXIT => {}
            Opcode::INEW => {}
            Opcode::INEWA => {}
            Opcode::INEWCB => {}
            Opcode::INEWCW => {}
            Opcode::INEWCF => {}
            Opcode::INEWCP => {}
            Opcode::INEWCM => {}
            Opcode::INEWCMP => {}
            Opcode::ISEND => {}
            Opcode::IRECV => {}
            Opcode::ICONSB => {}
            Opcode::ICONSW => {}
            Opcode::ICONSP => {}
            Opcode::ICONSF => {}
            Opcode::ICONSM => {}
            Opcode::ICONSMP => {}
            Opcode::IHEADB => {}
            Opcode::IHEADW => {}
            Opcode::IHEADP => {}
            Opcode::IHEADF => {}
            Opcode::IHEADM => {}
            Opcode::IHEADMP => {}
            Opcode::ITAIL => {}
            Opcode::ILEA => {}
            Opcode::IINDX => {}
            Opcode::IMOVP => {}
            Opcode::IMOVM => {}
            Opcode::IMOVMP => {}
            Opcode::IMOVB => {}
            Opcode::IMOVW => {}
            Opcode::IMOVF => {}
            Opcode::ICVTBW => {}
            Opcode::ICVTWB => {}
            Opcode::ICVTFW => {}
            Opcode::ICVTWF => {}
            Opcode::ICVTCA => {}
            Opcode::ICVTAC => {}
            Opcode::ICVTWC => {}
            Opcode::ICVTCW => {}
            Opcode::ICVTFC => {}
            Opcode::ICVTCF => {}
            Opcode::IADDB => {}
            Opcode::IADDW => {}
            Opcode::IADDF => {}
            Opcode::ISUBB => {}
            Opcode::ISUBW => {}
            Opcode::ISUBF => {}
            Opcode::IMULB => {}
            Opcode::IMULW => {}
            Opcode::IMULF => {}
            Opcode::IDIVB => {}
            Opcode::IDIVW => {}
            Opcode::IDIVF => {}
            Opcode::IMODW => {}
            Opcode::IMODB => {}
            Opcode::IANDB => {}
            Opcode::IANDW => {}
            Opcode::IORB => {}
            Opcode::IORW => {}
            Opcode::IXORB => {}
            Opcode::IXORW => {}
            Opcode::ISHLB => {}
            Opcode::ISHLW => {}
            Opcode::ISHRB => {}
            Opcode::ISHRW => {}
            Opcode::IINSC => {}
            Opcode::IINDC => {}
            Opcode::IADDC => {}
            Opcode::ILENC => {}
            Opcode::ILENA => {}
            Opcode::ILENL => {}
            Opcode::IBEQB => {}
            Opcode::IBNEB => {}
            Opcode::IBLTB => {}
            Opcode::IBLEB => {}
            Opcode::IBGTB => {}
            Opcode::IBGEB => {}
            Opcode::IBEQW => {}
            Opcode::IBNEW => {}
            Opcode::IBLTW => {}
            Opcode::IBLEW => {}
            Opcode::IBGTW => {}
            Opcode::IBGEW => {}
            Opcode::IBEQF => {}
            Opcode::IBNEF => {}
            Opcode::IBLTF => {}
            Opcode::IBLEF => {}
            Opcode::IBGTF => {}
            Opcode::IBGEF => {}
            Opcode::IBEQC => {}
            Opcode::IBNEC => {}
            Opcode::IBLTC => {}
            Opcode::IBLEC => {}
            Opcode::IBGTC => {}
            Opcode::IBGEC => {}
            Opcode::ISLICEA => {}
            Opcode::ISLICELA => {}
            Opcode::ISLICEC => {}
            Opcode::IINDW => {}
            Opcode::IINDF => {}
            Opcode::IINDB => {}
            Opcode::INEGF => {}
            Opcode::IMOVL => {}
            Opcode::IADDL => {}
            Opcode::ISUBL => {}
            Opcode::IDIVL => {}
            Opcode::IMODL => {}
            Opcode::IMULL => {}
            Opcode::IANDL => {}
            Opcode::IORL => {}
            Opcode::IXORL => {}
            Opcode::ISHLL => {}
            Opcode::ISHRL => {}
            Opcode::IBNEL => {}
            Opcode::IBLTL => {}
            Opcode::IBLEL => {}
            Opcode::IBGTL => {}
            Opcode::IBGEL => {}
            Opcode::IBEQL => {}
            Opcode::ICVTLF => {}
            Opcode::ICVTFL => {}
            Opcode::ICVTLW => {}
            Opcode::ICVTWL => {}
            Opcode::ICVTLC => {}
            Opcode::ICVTCL => {}
            Opcode::IHEADL => {}
            Opcode::ICONSL => {}
            Opcode::INEWCL => {}
            Opcode::ICASEC => {}
            Opcode::IINDL => {}
            Opcode::IMOVPC => {}
            Opcode::ITCMP => {}
            Opcode::IMNEWZ => {}
            Opcode::ICVTRF => {}
            Opcode::ICVTFR => {}
            Opcode::ICVTWS => {}
            Opcode::ICVTSW => {}
            Opcode::ILSRW => {}
            Opcode::ILSRL => {}
            Opcode::IECLR => {}
            Opcode::INEWZ => {}
            Opcode::INEWAZ => {}
            Opcode::IRAISE => {}
            Opcode::ICASEL => {}
            Opcode::IMULX => {}
            Opcode::IDIVX => {}
            Opcode::ICVTXX => {}
            Opcode::IMULX0 => {}
            Opcode::IDIVX0 => {}
            Opcode::ICVTXX0 => {}
            Opcode::IMULX1 => {}
            Opcode::IDIVX1 => {}
            Opcode::ICVTXX1 => {}
            Opcode::ICVTFX => {}
            Opcode::ICVTXF => {}
            Opcode::IEXPW => {}
            Opcode::IEXPL => {}
            Opcode::IEXPF => {}
            Opcode::ISELF => {}
        }


        if ctx.ins_left == 0 {
            break;
        }
    }
}
