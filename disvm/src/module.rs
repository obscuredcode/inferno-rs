use data::VmPtr;
use std::alloc::Layout;
use std::cell::RefCell;
use std::convert::TryInto;
use std::error::Error;
use std::ffi::CStr;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::Read;
use std::mem::size_of_val;
use std::rc::Rc;
use num_traits::{FromPrimitive, ToPrimitive};
use modular_bitfield::prelude::*;
use presser::{copy_from_slice_to_offset, copy_to_offset, Slab};
use consts::*;

use opcode::{INSTR_MNEMONICS, Opcode};
use data;
use data::{Data, DataCodeType, Heap, ModuleGlobalData, DataCode};
use util::BufferedReader;

#[derive(BitfieldSpecifier, Debug, Eq, PartialEq, Copy, Clone)]
#[bits=2]
pub enum MiddleOpMode {
    None             =0b00, // none   | no middle operand
    SmallImmediate   =0b01, // $SI    | small immediate
    SmallOffsetIndFP =0b10, // SO(FP) | small offset indirect from FP
    SmallOffsetIndMP =0b11  // SO(MP) | small offset indirect from MP
}

#[derive(BitfieldSpecifier, Debug, Eq, PartialEq, Copy, Clone)]
#[bits=3]
pub enum SourceDestOpMode {
    OffsetIndMP     =0b000, // LO(MP)     | offset indirect from MP
    OffsetIndFP     =0b001, // LO(FP)     | offset indirect from FP
    WordImmediate   =0b010, // $OP        | 30 bit immediate
    None            =0b011, // none       | no operand
    DoubleIndMP     =0b100, // SO(SO(MP)) | double indirect from MP
    DoubleIndFP     =0b101, // SO(SO(FP)) | double indirect from FP
    Reserved1       =0b110, //
    Reserved2       =0b111  //
}

//#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[bitfield(bits=8)]
pub struct OpAddressMode {
    #[bits=3]
    dest_op_mode: SourceDestOpMode,
    #[bits=3]
    source_op_mode: SourceDestOpMode,
    #[bits=2]
    middle_op_mode: MiddleOpMode
}

impl Debug for OpAddressMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpAddressMode {{ middle: {:?}, source {:?}, destination {:?} }}",
               self.middle_op_mode(), self.source_op_mode(), self.dest_op_mode())
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MiddleOperandData {
    SmallImmediate(i16),
    IndirectFP(i16),
    IndirectMP(i16),
    None
}

impl Display for MiddleOperandData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            MiddleOperandData::SmallImmediate(i) => {
                write!(f, "${}", i)
            }
            MiddleOperandData::IndirectFP(o) => {
                write!(f, "{}(fp)", o)
            }
            MiddleOperandData::IndirectMP(o) => {
                write!(f, "{}(mp)", o)
            }
            MiddleOperandData::None => {Ok(())}
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum SourceDestOperandData {
    Immediate(i32),
    IndirectMP(i32),
    IndirectFP(i32),
    DoubleIndirectMP(i16, i16),
    DoubleIndirectFP(i16, i16),
    None
}

impl Display for SourceDestOperandData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            SourceDestOperandData::Immediate(i) => {
                write!(f, "${}", i)
            }
            SourceDestOperandData::IndirectMP(ind) => {
                write!(f, "${}(mp)", ind)
            }
            SourceDestOperandData::IndirectFP(ind) => {
                write!(f, "${}(fp)", ind)
            }
            SourceDestOperandData::DoubleIndirectMP(ind1, ind2) => {
                write!(f, "${}({}(mp))", ind2, ind1)
            }
            SourceDestOperandData::DoubleIndirectFP(ind1, ind2) => {
                write!(f, "${}({}(fp))", ind2, ind1)
            }
            SourceDestOperandData::None => {Ok(())}
        }
    }
}


pub struct Instruction {
    pub(crate) opcode: Opcode,
    pub(crate) address: OpAddressMode,
    pub(crate) middle_data:MiddleOperandData,
    pub(crate) src_data: SourceDestOperandData,
    pub(crate) dest_data: SourceDestOperandData
}

impl Instruction {
    pub fn new() -> Self {
        Instruction {
            opcode: Opcode::INOP,
            address: OpAddressMode::from_bytes([0]),
            middle_data: MiddleOperandData::None,
            src_data: SourceDestOperandData::None,
            dest_data: SourceDestOperandData::None,
        }
    }
}

impl Debug for Instruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Instruction {{op: {:?}, {:?}, middle_data {:x?}, src_data {:x?}, dest_data {:x?} }}",
               self.opcode, self.address, self.middle_data, self.src_data, self.dest_data)
    }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", INSTR_MNEMONICS[self.opcode as usize])?;
        if self.middle_data == MiddleOperandData::None {
            if self.src_data == SourceDestOperandData::None {
                write!(f, "{}", self.dest_data)
            } else if self.dest_data == SourceDestOperandData::None {
                write!(f, "{}", self.src_data)
            } else {
                write!(f, "{}, {}", self.src_data, self.dest_data)
            }
        } else {
            write!(f, "{}, {}, {}", self.src_data, self.middle_data, self.dest_data)
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Operand {
    Byte(i8),
    Short(i16),
    Word(i32)
}

#[allow(dead_code)]
impl Operand {
    #[inline]
    pub fn size(self) -> usize {
        match self {
            Operand::Byte(_) => {1}
            Operand::Short(_) => {2}
            Operand::Word(_) => {4}
        }
    }
    #[inline]
    pub fn into_u8(self) -> u8 {
        match self {
            Operand::Byte(val) => {val as u8}
            Operand::Short(_) => {panic!("Operand not byte")}
            Operand::Word(_) => {panic!("Operand not byte")}
        }
    }
    #[inline]
    pub fn into_u16(self) -> u16 {
        match self {
            Operand::Byte(val) => {val as u16}
            Operand::Short(val) => {val as u16}
            Operand::Word(_) => {panic!("Operand not word")}
        }
    }
    #[inline]
    pub fn into_i16(self) -> i16 {
        match self {
            Operand::Byte(val) => {val as i16}
            Operand::Short(val) => {val as i16}
            Operand::Word(val) => {
                println!("Operand {:?} not word", self);
                val as i16
            }
        }
    }

    #[inline]
    pub fn into_u32(self) -> u32 {
        match self {
            Operand::Byte(val) => {val as u32}
            Operand::Short(val) => {val as u32}
            Operand::Word(val) => {val as u32}
        }
    }
    #[inline]
    pub fn into_i32(self) -> i32 {
        match self {
            Operand::Byte(val) => {val as i32}
            Operand::Short(val) => {val as i32}
            Operand::Word(val) => {val as i32}
        }
    }
    #[inline]
    pub fn into_usize(self) -> usize {
        match self {
            Operand::Byte(val) => {val as usize}
            Operand::Short(val) => {val as usize}
            Operand::Word(val) => {val as usize}
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone)]
pub struct TypeMap {
    pub bytes: Vec<u8>
}



impl Debug for TypeMap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f,"[");
        for b in &self.bytes {
            writeln!(f, "   {b:08b},")?
        }
        write!(f, "]")
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Type {
    pub(crate) desc_no: i32,
    pub(crate) size: i32,
    pub(crate) map_size: i32,
    pub(crate) map: TypeMap
}

impl Type {
    pub fn new() -> Self {
        Type {
            desc_no: 0,
            size: 0,
            map_size: 0,
            map: TypeMap { bytes: vec![] },
        }
    }

    pub fn is_pointer(&self, offset: usize) -> bool {
        let byte_index = offset/32;

        let bit_index = 7 - (offset % 32)/4;

        if (byte_index < self.map_size as usize) {
            let byte_map = self.map.bytes[(self.map_size as usize - byte_index) - 1];
            // lowest offset is mapped to highest bit

            return (byte_map >> bit_index) & 0x01 == 0x01;
        }
        return false;
    }
}



#[derive(Debug)]
pub struct DataSection {
    pub code: DataCode,
    pub(crate) countopt: Option<i32>,
    pub(crate) offset: i32,
    pub(crate) data: Data
}

impl DataSection {
    pub fn new() -> Self {
        DataSection {
            code: DataCode::from_bytes([0]),
            countopt: None,
            offset: 0,
            data: Data::Byte(vec![0])
        }
    }
}

#[derive(Debug)]
pub struct Export {
    pub(crate) entry_pc: i32,
    pub(crate) desc: i32,
    pub(crate) sig: i32,
    pub(crate) fn_name: String
}

impl Export {
    pub fn new() -> Self {
        Export {
            entry_pc: 0,
            desc: 0,
            sig: 0,
            fn_name: "".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Import {
    pub(crate) sig: i32,
    pub(crate) fn_name: String
}

impl Import {
    pub fn new() -> Self {
        Import {
            sig: 0,
            fn_name: "".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct ModuleImport {
    pub(crate) imports: Vec<Import>
}

impl ModuleImport {
    pub fn new() -> Self {
        ModuleImport {
            imports: vec![],
        }
    }
}

#[derive(Debug)]
pub struct ExceptionHandler {
    pub(crate) name: String,
    pub(crate) pc: i32
}

impl ExceptionHandler {
    pub fn new() -> Self {
        ExceptionHandler {
            name: "".to_string(),
            pc: 0,
        }
    }
}


#[derive(Debug)]
pub struct HandlerSection {
    pub(crate) frame_offset: i32,
    pub(crate) pc_start: i32,
    pub(crate) pc_end: i32,
    pub(crate) type_desc: i32,
    no_exceptions: i32,
    exceptions: Vec<ExceptionHandler>,
    pub(crate) default_pc: i32
}

impl HandlerSection {
    pub fn new() -> Self {
        HandlerSection {
            frame_offset: 0,
            pc_start: 0,
            pc_end: 0,
            type_desc: 0,
            no_exceptions: 0,
            exceptions: vec![],
            default_pc: 0
        }
    }
}


#[derive(Debug)]
pub enum ModuleParserError {
    InvalidMagic(u32),
    MalformedOperand(Operand,[u8; 4]),
    UnrecognizedOpcode(u8),
    UnrecognizedOperandData(Operand, SourceDestOpMode),
    MalformedHeader(String),
    SliceError(usize),
    IOError(std::io::Error)
}

impl Display for ModuleParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            ModuleParserError::InvalidMagic(magic) =>
                write!(f, "The magic number {} is not recognized as either XMAGIC or SMAGIC", magic),
            ModuleParserError::MalformedOperand(operand, bytes) =>
                write!(f, "The bytes {:?} are not enough for operand of size {}", bytes, operand.size()),
            ModuleParserError::UnrecognizedOpcode(op) => write!(f, "Invalid Opcode {:x}", op),
            ModuleParserError::SliceError(off) => write!(f, "Not enough bytes for slice at offset {}", off),
            ModuleParserError::UnrecognizedOperandData(operand, mode) =>
                write!(f, "Unable to handle addressing mode {:?} for data {:?}", mode, operand),
            ModuleParserError::MalformedHeader(ref err) => write!(f, "{}", err),
            ModuleParserError::IOError(ref err) => std::fmt::Display::fmt(&err, f)
        }
    }
}

impl Error for ModuleParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            ModuleParserError::InvalidMagic(_) => None,
            ModuleParserError::MalformedOperand(_,_) => None,
            ModuleParserError::UnrecognizedOpcode(_) => None,
            ModuleParserError::UnrecognizedOperandData(_, _) => None,
            ModuleParserError::SliceError(_) => None,
            ModuleParserError::MalformedHeader(_) => None,
            ModuleParserError::IOError(ref err) => Some(err)
        }
    }
}


pub fn parse_operand(buffer: &[u8]) -> Operand {
    let operand_size = buffer[0] & 0xC0;


    match operand_size {
        0x00 => {
           Operand::Byte(buffer[0] as i8) //(buffer[0].into(), 1)
        },
        0x40 => {
            Operand::Byte((buffer[0] | !0x7F) as i8) //, 1)
        },
        0x80 => {
            let mut buf: [u8; 2] = [0; 2];
            buf.copy_from_slice(&buffer[..2]);
            if (operand_size & 0x20) != 0x00 {
                buf[0] |= !0x3F;
            }
            else {
                buf[0] &= 0x3F;
            }

            Operand::Short(i16::from_be_bytes(buf)) //.into(), 2)
        },
        0xC0 => {
            let mut buf: [u8; 4] = [0; 4];
            buf.copy_from_slice(&buffer[..4]);
            if (operand_size & 0x20) != 0x00 {
                buf[0] |= !0x3F;
            }
            else {
                buf[0] &= 0x3F;
            }
            Operand::Word(i32::from_be_bytes(buf)) // , 4)
        }
        _ => panic!("Unable to parse operand.")
    }
}


pub fn parse_word(bytes: &[u8]) -> i32 {
    let mut buf: [u8; 4] = [0; 4];
    buf.copy_from_slice(&bytes[..4]);
    i32::from_be_bytes(buf)
}


pub const MODULE_TYPE_DESC: i32 = 0;
pub const ARRAY_PARSER_STACK: usize = 10;

#[derive(Debug)]
pub struct DisModule {
    magic: DisMagicNo,
    signature: Option<Vec<u8>>,
    pub runtime_flag: DisRuntimeFlag,
    pub stack_extent: i32,
    code_size: i32,
    data_size: i32,
    type_size: i32,
    link_size: i32,
    pub entry_pc: i32,
    pub entry_type: i32,
    pub code: Vec<Instruction>,
    pub types: Vec<Type>,
    pub data: Vec<DataSection>,
    pub module_name: String,
    pub exports: Vec<Export>,
    pub imports: Vec<ModuleImport>,
    pub handlers: HandlerSection,
    pub module_src_path: String
}

impl DisModule {
    pub fn new() -> Self {
        DisModule {
            magic: DisMagicNo::XMAGIC,
            signature: None,
            runtime_flag: DisRuntimeFlag::MUSTCOMPILE,
            stack_extent: 0,
            code_size: 0,
            data_size: 0,
            type_size: 0,
            link_size: 0,
            entry_pc: 0,
            entry_type: 0,
            code: vec![],
            types: vec![],
            data: vec![],
            exports: vec![],
            imports: vec![],
            handlers: HandlerSection::new(),
            module_name: String::new(),
            module_src_path: String::new()
        }
    }
}

pub fn load_module(name: &str) -> Result<DisModule, std::io::Error> {

    let mut file = File::open(&name)?;

    let mut buffer = Vec::new();
    let mut offset = 0;



    let mut dis = DisModule::new();

    file.read_to_end(&mut buffer).expect("failed to read module");

    // Parse Header

    let magic  = parse_operand(buffer.get(offset..offset+4).expect("Unable to read magic"));
    offset += magic.size();

    //println!("magic: {:?}", magic);
    match DisMagicNo::from_u32(magic.into_u32()) {
        Some(DisMagicNo::XMAGIC) => {
            // no signature to parse
            dis.magic = DisMagicNo::XMAGIC;
        }
        Some(DisMagicNo::SMAGIC) => {
            // deal with signature
            let length = parse_operand(buffer.get(offset..offset+4).expect("Unable to read Signature Size"));
            offset += length.size();

            let signature_slice = buffer.get(offset..offset+length.into_usize()).expect("Unable to read signature");
            offset += length.into_usize();

            dis.magic = DisMagicNo::SMAGIC;
            dis.signature = Some(Vec::from(signature_slice));
        }
        None => {
            // TODO: deal with invalid magic
            panic!("Invalid magic number.");
        }
    }


    let runtime_flag = parse_operand(buffer.get(offset..offset+4).expect("Unable to read Runtime flag"));
    offset += runtime_flag.size();
    dis.runtime_flag = DisRuntimeFlag::from_u16(runtime_flag.into_u16()).expect("Unrecognized Runtime flag");

    let stack_extent = parse_operand(buffer.get(offset..offset+4).expect("Unable to read stack size"));
    offset += stack_extent.size();
    dis.stack_extent = stack_extent.into_i32();

    let code_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read code size"));
    offset += code_size.size();
    dis.code_size = code_size.into_i32();

    let data_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read data size"));
    offset += data_size.size();
    dis.data_size = data_size.into_i32();

    let type_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read type size"));
    offset += type_size.size();
    dis.type_size = type_size.into_i32();

    let link_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read link size"));
    offset += link_size.size();
    dis.link_size = link_size.into_i32();

    let entry_pc = parse_operand(buffer.get(offset..offset+4).expect("Unable to read entry program counter"));
    offset += entry_pc.size();
    dis.entry_pc = entry_pc.into_i32();

    let entry_type = parse_operand(buffer.get(offset..offset+4).expect("Unable to read entry type index"));
    offset += entry_type.size();
    dis.entry_type = entry_type.into_i32();

    if code_size.into_i32() < 0 || data_size.into_i32()  < 0 || type_size.into_i32()  < 0 || link_size.into_i32() < 0 {
        panic!("Invalid sizes in module header {:?}.", dis);
    }

    // Read Code

    //let code = buffer.get(offset..offset+code_size.into_usize()).expect("Unable to read code section");
    //let mut code_vec : Vec<Instruction> = Vec::with_capacity(code_size.into_usize());
    dis.code = Vec::with_capacity(code_size.into_usize());

    for _i in 0..code_size.into_usize() {
        // read single instruction
        let mut ins_start = offset;
        let mut ins = Instruction::new();
        let opadr = buffer.get(offset..offset+2).expect("Unable to read opcode and addressing mode");
        ins.opcode = Opcode::from_u8(opadr[0]);
        ins.address = OpAddressMode::from_bytes([opadr[1]]);
        offset += 2;

        if ins.address.middle_op_mode() != MiddleOpMode::None {
            let data = parse_operand(buffer.get(offset..offset+4).expect("Unable to read middle operand data"));
            offset += data.size();

            match ins.address.middle_op_mode() {
                MiddleOpMode::None => {}
                MiddleOpMode::SmallImmediate => {
                    ins.middle_data = MiddleOperandData::SmallImmediate(data.into_i16())
                }
                MiddleOpMode::SmallOffsetIndFP => {
                    ins.middle_data = MiddleOperandData::IndirectFP(data.into_i16())
                }
                MiddleOpMode::SmallOffsetIndMP => {
                    ins.middle_data = MiddleOperandData::IndirectMP(data.into_i16())
                }
            }
        }

        if ins.address.source_op_mode() != SourceDestOpMode::None {
            let data = parse_operand(buffer.get(offset..offset+4).expect("Unable to read source operand data"));
            offset += data.size();

            match ins.address.source_op_mode() {
                SourceDestOpMode::OffsetIndMP => {
                    ins.src_data = SourceDestOperandData::IndirectMP(data.into_i32());
                }
                SourceDestOpMode::OffsetIndFP => {
                    ins.src_data = SourceDestOperandData::IndirectFP(data.into_i32());
                }
                SourceDestOpMode::WordImmediate => {
                    ins.src_data = SourceDestOperandData::Immediate(data.into_i32());
                }
                SourceDestOpMode::None => {}
                SourceDestOpMode::DoubleIndMP => {
                    let data2 = parse_operand(buffer.get(offset..offset+4).expect("Unable to read source operand second indirect"));
                    offset += data2.size();
                    ins.src_data = SourceDestOperandData::DoubleIndirectMP(data.into_i16(), data2.into_i16())
                }
                SourceDestOpMode::DoubleIndFP => {
                    let data2 = parse_operand(buffer.get(offset..offset+4).expect("Unable to read source operand second indirect"));
                    offset += data2.size();
                    ins.src_data = SourceDestOperandData::DoubleIndirectFP(data.into_i16(), data2.into_i16())
                }
                SourceDestOpMode::Reserved1 => {
                    //panic!("Unknown Source Operand Data")
                }
                SourceDestOpMode::Reserved2 => {
                    //panic!("Unknown Source Operand Data")
                }
            }
        }
        let s = ins.address.bytes[0] & 0x07;

        if ins.address.dest_op_mode() != SourceDestOpMode::None {
            let data = parse_operand(buffer.get(offset..offset+4).expect("Unable to read dest operand data"));
            offset += data.size();

            match ins.address.dest_op_mode() {
                SourceDestOpMode::OffsetIndMP => {
                    ins.dest_data = SourceDestOperandData::IndirectMP(data.into_i32());
                }
                SourceDestOpMode::OffsetIndFP => {
                    ins.dest_data = SourceDestOperandData::IndirectFP(data.into_i32());
                }
                SourceDestOpMode::WordImmediate => {
                    ins.dest_data = SourceDestOperandData::Immediate(data.into_i32());
                }
                SourceDestOpMode::None => {}
                SourceDestOpMode::DoubleIndMP => {
                    let data2 = parse_operand(buffer.get(offset..offset+4).expect("Unable to read dest operand second indirect"));
                    offset += data2.size();
                    ins.dest_data = SourceDestOperandData::DoubleIndirectMP(data.into_i16(), data2.into_i16())
                }
                SourceDestOpMode::DoubleIndFP => {
                    let data2 = parse_operand(buffer.get(offset..offset+4).expect("Unable to read dest operand second indirect"));
                    offset += data2.size();
                    ins.dest_data = SourceDestOperandData::DoubleIndirectFP(data.into_i16(), data2.into_i16())
                }
                SourceDestOpMode::Reserved1 => {panic!("Unknown dest Operand Data")}
                SourceDestOpMode::Reserved2 => {panic!("Unknown dest Operand Data")}
            }
        }

        dis.code.push(ins);
        //println!("Ins {} is {:x?}", _i, buffer.get(ins_start..offset).unwrap());
    }

    // read type descriptors

    for _i in 0..dis.type_size {
        // read single type
        let mut type_desc = Type::new();
        let desc_no = parse_operand(buffer.get(offset..offset+4).expect("Unable to read type descriptor"));
        offset += desc_no.size();
        type_desc.desc_no = desc_no.into_i32();

        let type_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read type size"));
        offset += type_size.size();
        type_desc.size = type_size.into_i32();

        let map_size = parse_operand(buffer.get(offset..offset+4).expect("Unable to read type map size"));
        offset += map_size.size();
        type_desc.map_size = map_size.into_i32();

        for _j in 0..type_desc.map_size {
            let map_byte = buffer.get(offset..offset+1).expect("Unable to read map byte");
            offset += 1;

            type_desc.map.bytes.push(map_byte[0]);
        }
        dis.types.push(type_desc);
    }

    // find module type descriptor

    for desc in &dis.types {
        if desc.desc_no == MODULE_TYPE_DESC {
            //println!("{:#?}", desc);
            println!("Module Type Descriptor: ");
            if (desc.size != dis.data_size) {
                panic!("Module type descriptor invalid");
            }
        }
        //println!("{:#?}", desc);
    }

    let mut md = Heap::new(&dis.types[0]);

    let buffer_copy = buffer.clone();

    let mut reader = Rc::new(RefCell::new(BufferedReader {
        buffer: &buffer_copy,
        offset: offset,
        error: "".to_string()
    }));


    // read data
    // setup array parsing stack
    let mut array_stack: Vec<usize> = Vec::with_capacity(ARRAY_PARSER_STACK);
    // setup base data address
    let data_size = dis.data_size as usize;

    let mut module_data_vec: Vec<u8> = Vec::with_capacity(data_size);
    //module_data_vec.fill(0);
    for _i in 0..data_size {
        module_data_vec.push(0);
    }

   // let mut m = unsafe { ModuleData::new(dis.types[0].clone()) }.unwrap();
    /*let s = runtime::ctypes::String {
        len: 1,
        max: 2,
        _tmp: 3,
        data: [4, 5, 6, 7],
    };
    m.write_offset(s, 0);*/

    let mut m = ModuleGlobalData::new();

    let mut base_address: usize = 0;

    let mut i = 0;
    'data: loop {
        let mut data = DataSection::new();
        let mut count = 0;
        let mut code = DataCode::from_bytes([0]);
        {
            let mut  r = reader.borrow_mut();
            code = DataCode::from_bytes([r.u8()]);

            ;/*DataCode::from_bytes([buffer.get(offset..offset+1).
                                                                expect("Unable to read data code")[0]]);*/
            //offset += 1;

            if code.is_zero() {
                break 'data;
            }


            count = code.get_count() as i32;
            if count == 0 {
                let countopt = r.operand(); //parse_operand(buffer.get(offset..offset+4).expect("Unable to read data countop"));
                //offset += countopt.size();
                count = countopt.into_i32();
                data.countopt = Some(count)
            }

            if count < 0 {
                panic!("Invalid data count {count}");
            }

           // let data_offset = r.operand(); //parse_operand(buffer.get(offset..offset+4).expect("Unable to read data offset"));
            //offset += data_offset.size();


            //let mut dest_address = base_address + data_offset.into_usize();
            //data.offset = dest_address as i32; //data_offset.into_i32();
            //data.code = code;
        }
        //m.load_data(count as usize, &mut data, &dis.types, reader.clone());
        md.load_data(code, count as usize, &dis.types, reader.clone());
        /*match code.data_type_or_err() {
            Ok(DataCodeType::Byte) => {
                //println!("Found {} bytes", count);
                let mut bytes: Vec<u8> = vec![];
                for _j in 0..count {
                    let byte = buffer.get(offset..offset+1).expect("Unable to read data byte")[0];
                    offset += 1;
                    // read into dest_address
                    //copy_to_offset(&byte, &mut slab, dest_address);
                    bytes.push(byte);
                }
                data.data = Data::Byte(bytes);
                println!("offset {}, wanted: {}: {:?}",
                         data.offset,
                         data_offset.into_usize(),
                        &data.data);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Ok(DataCodeType::Integer32) => {
                //println!("Found {} integers", count);
                let mut integers: Vec<i32> = vec![];
                for _j in 0..count {
                    let word = parse_word(buffer.get(offset..offset+4).expect("Unable to read data int32"));
                    offset += 4;
                    // read into dest_address
                    //copy_to_offset(&word, &mut slab, dest_address);
                    integers.push(word);
                }

                data.data = Data::Integer32(integers);
                println!("offset {}, wanted: {}: {:?}",
                         data.offset,
                         data_offset.into_usize(),
                         &data.data);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Ok(DataCodeType::StringUTF) => {
                //let mut chars: String = String::with_capacity(count as usize);
                //chars.copy_from_slice(buffer.get(offset..offset+count as usize).expect("Unable to read utf8 string"));

                let chars = String::from_utf8_lossy(buffer.get(offset..offset+count as usize).expect("Unable to read utf8 string"));
                //println!("string {} is {}", _i, chars);
                offset += count as usize;
                data.data = Data::String(chars.to_string());
                //copy_from_slice_to_offset(chars.as_bytes(), &mut slab, dest_address);
                println!("offset {}, wanted: {}: {:?}",
                         data.offset,
                         data_offset.into_usize(),
                         &data.data);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Ok(DataCodeType::Float) => {
                //println!("found {} floats", count);
                let mut floats: Vec<f64> = vec![];
                for _j in 0..count {
                    let mut bytes: [u8; 8] = [0; 8];
                    bytes.copy_from_slice(buffer.get(offset..offset+4).expect("Unable to read data float64"));
                    let float = f64::from_be_bytes(bytes);
                    offset += 8;
                    floats.push(float);
                }
                data.data = Data::Float(floats);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Ok(DataCodeType::Array) => {
                let array_type = parse_word(buffer.get(offset..offset+4).expect("Unable to read array type"));
                offset += 4;
                if array_type < 0 {
                    // deal with invalid type
                }

                let array_elements_count = parse_word(buffer.get(offset..offset+4).expect("Unable to read array elements count"));
                offset += 4;

                data.data = Data::Array(array_type, array_elements_count, vec![]);
                println!("offset {}, wanted: {}: {:?}",
                         data.offset,
                         data_offset.into_usize(),
                         &data.data);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Ok(DataCodeType::ArraySetIndex) => {
                let array_index = parse_word(buffer.get(offset..offset+4).expect("Unable to read array base address"));
                offset += 4;

                if array_index < 0 {
                    // invalid index
                }
                // the current dest_address should refer to an array
                let arr = m.get(dest_address);
                match &arr {
                    Data::Array(t, c, _) => {
                        let typ = &dis.types[*t as usize];
                        println!("array typ: {:?}, count: {}", typ, *c);
                    }
                    _ => {
                        panic!("set index isn't following array")
                    }
                }
                // push current base address
                array_stack.push(base_address);
                // load new base address from array_index
                //base_address = array_index as usize + 4;
                m.set_index(array_index as usize, base_address, &dis.types);

            }
            Ok(DataCodeType::RestoreBase) => {
                // pop current base address
                let new = array_stack.pop().expect("Attempted to restore array base address when none was saved");
                println!("restored base from {} to {}", base_address, new);
                base_address = new;
            }
            Ok(DataCodeType::Integer64) => {
               // println!("found {} int64s", count);
                let mut integers: Vec<i64> = vec![];
                for _j in 0..count {
                    let mut buf: [u8; 8] = [0; 8];
                    buf.copy_from_slice(buffer.get(offset..offset+8).expect("Unable to read int64 datum"));
                    let big = i64::from_be_bytes(buf);
                    //print!("\t datum {} is {}", j, big);
                    offset += 8;
                    integers.push(big);
                }
                data.data = Data::Integer64(integers);
                m.set(dest_address, data.data);
                dest_address += 4;
            }
            Err(bits) => {
               panic!("Invalid data code bytes {}", bits.invalid_bytes);
            }
        }*/


        //dis.data.push(data);
        i += 1;
    }

    offset = reader.borrow().offset;
    let mut op = 0;
    for i in &dis.code {
        match i.src_data {
            SourceDestOperandData::Immediate(_) => {}
            SourceDestOperandData::IndirectMP(o) => {
                let d = md.get(o as usize);
                match d {
                    Some(data) => {
                        println!("\nsrc is {:?}", data);
                    }
                    _ => {

                    }
                }

            }
            SourceDestOperandData::IndirectFP(_) => {}
            SourceDestOperandData::DoubleIndirectMP(_, _) => {}
            SourceDestOperandData::DoubleIndirectFP(_, _) => {}
            SourceDestOperandData::None => {}
        }
        println!("{}: {}", op, i);
        op += 1;
    }

    //println!("{:#?}", m);
    /*unsafe {
        let s = CStr::from_ptr(slab.base_ptr().add(4) as *const i8);
        println!("{:?}", s)
    }*/

    // read module name

    loop {
        let c = buffer.get(offset..offset+1).expect("Unable to read module name")[0];
        offset += 1;
        if c == 0 {
            break;
        }
        dis.module_name.push(c as char)
    }
    //println!("Module Name: {}", dis.module_name);

    // link/export section

    for _i in 0..dis.link_size {
        let mut export = Export::new();
        let entry_pc = parse_operand(buffer.get(offset..offset+4).expect("Unable to read export entry pc"));
        offset += entry_pc.size();
        export.entry_pc = entry_pc.into_i32();

        let desc = parse_operand(buffer.get(offset..offset+4).expect("Unable to read export type descriptor"));
        offset += desc.size();
        export.desc = desc.into_i32();

        export.sig = parse_word(buffer.get(offset..offset+4).expect("Unable to read export type checksum"));
        offset += 4;

        loop {
            let c = buffer.get(offset..offset+1).expect("Unable to read export function name")[0];
            offset += 1;
            if c == 0 {
                break;
            }
            export.fn_name.push(c as char);
        }

        dis.exports.push(export);
    }

    // import section

    if dis.runtime_flag.to_i16().unwrap() & DisRuntimeFlag::HASLDT2.to_i16().unwrap() != 0 {
        let module_import_count = parse_operand(buffer.get(offset..offset+4).expect("Unable to read import module count"));
        offset += module_import_count.size();
        let module_import_count = module_import_count.into_usize();

        for _i in 0..module_import_count {
            let mut module_imports = ModuleImport::new();

            let import_count = parse_operand(buffer.get(offset..offset+4).expect("Unable to read import count"));
            offset += import_count.size();
            let import_count = import_count.into_usize();

            for _j in 0..import_count {
                let mut import = Import::new();

                import.sig = parse_word(buffer.get(offset..offset+4).expect("Unable to read import type checksum"));
                offset += 4;

                loop {
                    let c = buffer.get(offset..offset+1).expect("Unable to read import function name")[0];
                    offset += 1;
                    if c == 0 {
                        break;
                    }
                    import.fn_name.push(c as char);
                }
                module_imports.imports.push(import);
            }
            dis.imports.push(module_imports);
        }
        //println!("{:?}",dis.imports);
        offset += 1; // skip zero byte
    }


    // handler section

    if dis.runtime_flag.to_i16().unwrap() & DisRuntimeFlag::HASEXCEPT.to_i16().unwrap() != 0 {
        let mut handler = HandlerSection::new();

        let handler_offset = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler offset"));
        offset += handler_offset.size();
        handler.frame_offset = handler_offset.into_i32();

        let pc_start = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler pc start"));
        offset += pc_start.size();
        handler.pc_start = pc_start.into_i32();

        let pc_end = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler pc end"));
        offset += pc_end.size();
        handler.pc_end = pc_end.into_i32();

        let desc = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler type descriptor"));
        offset += desc.size();
        handler.type_desc = desc.into_i32();

        if handler.type_desc >= 0 {
            // TODO handle
        } else {

        }

        let handler_count = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler section count"));
        offset += handler_count.into_usize();
        let handler_count = handler_count.into_usize();

        for _i in 0..handler_count {
            let mut exception_handler = ExceptionHandler::new();
            loop {
                let c = buffer.get(offset..offset+1).expect("Unable to read exception handler function name")[0];
                offset += 1;
                if c == 0 {
                    break;
                }
                exception_handler.name.push(c as char);
            }

            let pc = parse_operand(buffer.get(offset..offset+4).expect("Unable to read exception handler function pc"));
            offset += 4;
            exception_handler.pc = pc.into_i32();
        }

        let default_pc  = parse_operand(buffer.get(offset..offset+4).expect("Unable to read exception handler function pc"));
        offset += 4;
        handler.default_pc = default_pc.into_i32();

        dis.handlers = handler;
    }

    loop {
        let c = buffer.get(offset..offset+1).expect("Unable to read module src path")[0];
        offset += 1;
        if c == 0 {
            break;
        }
        dis.module_src_path.push(c as char)
    }

    //println!("{}", dis.module_src_path);

    //println!("Module {:#?}", dis);

    Ok(dis)
}