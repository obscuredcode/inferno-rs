use std::error::Error;
use std::fmt::{Debug, Formatter, write};
use std::fs::File;
use std::io::Read;
use modular_bitfield::error::InvalidBitPattern;
//use modular_bitfield::{bitfield, BitfieldSpecifier};
use modular_bitfield::prelude::*;
use crate::disvm::consts::*;

use num_traits::{FromPrimitive, ToPrimitive};
use crate::disvm::opcode::Opcode;

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

impl std::fmt::Display for ModuleParserError {
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
    source_op_mode: SourceDestOpMode,
    #[bits=3]
    dest_op_mode: SourceDestOpMode,
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

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum SourceDestOperandData {
    Immediate(i32),
    IndirectMP(i32),
    IndirectFP(i32),
    DoubleIndirectMP(i16, i16),
    DoubleIndirectFP(i16, i16),
    None
}

//#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Instruction {
    opcode: Opcode,
    address: OpAddressMode,
    middle_data:MiddleOperandData,
    src_data: SourceDestOperandData,
    dest_data: SourceDestOperandData
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

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Operand {
    Byte(i8),
    Short(i16),
    Word(i32)
}

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
            Operand::Word(_) => {panic!("Operand not word")}
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

//#[derive(Debug)]
pub struct TypeMap {
    bytes: Vec<u8>
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

#[derive(Debug)]
pub struct Type {
    desc_no: i32,
    size: i32,
    map_size: i32,
    map: TypeMap
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
}



#[derive(BitfieldSpecifier, Debug, PartialEq, Eq, Copy, Clone)]
#[bits=4]
pub enum DataCodeType {
    Byte          = 0b0001,
    Integer32     = 0b0010,
    StringUTF     = 0b0011,
    Float         = 0b0100,
    Array         = 0b0101,
    ArraySetIndex = 0b0110,
    RestoreBase   = 0b0111,
    Integer64     = 0b1000,
}

#[bitfield(bits=8)]
pub struct DataCode {
    #[bits=4]
    count: B4,
    #[bits=4]
    data_type: DataCodeType,
}

impl Debug for DataCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.data_type_or_err() {
            Ok(dt) => {
                write!(f, "DataCode {{ size: {}, type: {:?} }}", self.count(), dt)
            }
            Err(bit) => {
                write!(f, "DataCode {{ size: {}, unrecognized type: {:?} }}", self.count(), bit.invalid_bytes)
            }
        }
    }
}

#[derive(Debug)]
pub enum Data {
    Byte(Vec<u8>),
    Integer32(Vec<i32>),
    String(String),
    Float(Vec<f64>),
    Array(i32, i32),
    ArraySetIndex(i32),
    RestoreBase(),
    Integer64(Vec<i64>),
}

#[derive(Debug)]
pub struct DataSection {
    code: DataCode,
    countopt: Option<i32>,
    offset: i32,
    data: Data
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
    entry_pc: i32,
    desc: i32,
    sig: i32,
    fn_name: String
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
    sig: i32,
    fn_name: String
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
    imports: Vec<Import>
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
    name: String,
    pc: i32
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
    frame_offset: i32,
    pc_start: i32,
    pc_end: i32,
    type_desc: i32,
    no_exceptions: i32,
    exceptions: Vec<ExceptionHandler>,
    default_pc: i32
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

pub fn parse_header(buf: &[u8]) {
    let first: u8 ;
}

#[derive(Debug)]
pub struct DisModule {
    magic: DisMagicNo,
    signature: Option<Vec<u8>>,
    runtime_flag: DisRuntimeFlag,
    stack_extent: i32,
    code_size: i32,
    data_size: i32,
    type_size: i32,
    link_size: i32,
    entry_pc: i32,
    entry_type: i32,
    code: Vec<Instruction>,
    types: Vec<Type>,
    data: Vec<DataSection>,
    module_name: String,
    exports: Vec<Export>,
    imports: Vec<ModuleImport>,
    handlers: HandlerSection,
    module_src_path: String
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

pub fn load_module(name: &str) -> Result<(), std::io::Error> {

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

    if (code_size.into_i32() < 0 || data_size.into_i32()  < 0 || type_size.into_i32()  < 0 || link_size.into_i32() < 0) {
        panic!("Invalid sizes in module header {:?}.", dis);
    }

    // Read Code

    //let code = buffer.get(offset..offset+code_size.into_usize()).expect("Unable to read code section");
    //let mut code_vec : Vec<Instruction> = Vec::with_capacity(code_size.into_usize());
    dis.code = Vec::with_capacity(code_size.into_usize());

    for i in 0..code_size.into_usize() {
        // read single instruction
        let mut ins = Instruction::new();
        let opadr = buffer.get(offset..offset+2).expect("Unable to read opcode and addressing mode");
        ins.opcode = Opcode::from_u8(opadr[0]);
        ins.address = OpAddressMode::from_bytes([opadr[1]]);
        offset += 2;

        if (ins.address.middle_op_mode() != MiddleOpMode::None) {
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

        if (ins.address.source_op_mode() != SourceDestOpMode::None) {
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

        if (ins.address.dest_op_mode() != SourceDestOpMode::None) {
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

        //println!("Instruction {} is {:?}", i, ins);
        dis.code.push(ins);

    }

    // read type descriptors

    for i in 0..dis.type_size {
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

        for j in 0..type_desc.map_size {
            let map_byte = buffer.get(offset..offset+1).expect("Unable to read map byte");
            offset += 1;

            type_desc.map.bytes.push(map_byte[0]);
        }
        dis.types.push(type_desc);
    }

    // read data

    // setup array parsing stack
    // setup base data address
    let mut base_address: usize = 0;

    let mut i = 0;
    'data: loop {


        let mut data = DataSection::new();

        let code = DataCode::from_bytes([buffer.get(offset..offset+1).
                                                                expect("Unable to read data code")[0]]);

        offset += 1;
        
        if (code.bytes[0] == 0) {
            break 'data;
        }

        let mut count = code.count() as i32;
        if (count == 0) {
            let countopt = parse_operand(buffer.get(offset..offset+4).expect("Unable to read data countop"));
            offset += countopt.size();
            count = countopt.into_i32();
            data.countopt = Some(count)
        }

        let data_offset = parse_operand(buffer.get(offset..offset+4).expect("Unable to read data offset"));
        offset += data_offset.size();
        data.offset = data_offset.into_i32();

        let mut dest_address = base_address + data_offset.into_usize();

        match code.data_type_or_err() {
            Ok(DataCodeType::Byte) => {
                //println!("Found {} bytes", count);
                let mut bytes: Vec<u8> = vec![];
                for j in 0..count {
                    let byte = buffer.get(offset..offset+1).expect("Unable to read data byte")[0];
                    offset += 1;
                    // read into dest_address
                    dest_address += 1;

                    bytes.push(byte);
                }
                data.data = Data::Byte(bytes);
            }
            Ok(DataCodeType::Integer32) => {
                //println!("Found {} integers", count);
                let mut integers: Vec<i32> = vec![];
                for j in 0..count {
                    let word = parse_word(buffer.get(offset..offset+4).expect("Unable to read data int32"));
                    offset += 4;
                    // read into dest_address
                    dest_address += 1;
                    integers.push(word);
                }
                data.data = Data::Integer32(integers);
            }
            Ok(DataCodeType::StringUTF) => {
                //let mut chars: String = String::with_capacity(count as usize);
                //chars.copy_from_slice(buffer.get(offset..offset+count as usize).expect("Unable to read utf8 string"));

                let chars = String::from_utf8_lossy(buffer.get(offset..offset+count as usize).expect("Unable to read utf8 string"));
                //println!("string {} is {}", i, chars);
                offset += count as usize;
                data.data = Data::String(chars.to_string());
            }
            Ok(DataCodeType::Float) => {
                //println!("found {} floats", count);
                let mut floats: Vec<f64> = vec![];
                for j in 0..count {
                    let mut bytes: [u8; 8] = [0; 8];
                    bytes.copy_from_slice(buffer.get(offset..offset+4).expect("Unable to read data float64"));
                    let float = f64::from_be_bytes(bytes);
                    offset += 8;
                    floats.push(float);
                }
            }
            Ok(DataCodeType::Array) => {
                let array_type = parse_word(buffer.get(offset..offset+4).expect("Unable to read array type"));
                offset += 4;
                if (array_type < 0) {
                    // deal with invalid type
                }

                let array_elements_count = parse_word(buffer.get(offset..offset+4).expect("Unable to read array elements count"));
                offset += 4;


            }
            Ok(DataCodeType::ArraySetIndex) => {
                let array_index = parse_word(buffer.get(offset..offset+4).expect("Unable to read array base address"));
                offset += 4;

                if (array_index < 0) {
                    // invalid index
                }
                // the current dest_address should refer to an array

                // push current base address
                // load new base address from array_index

            }
            Ok(DataCodeType::RestoreBase) => {
                // pop current base address
            }
            Ok(DataCodeType::Integer64) => {
               // println!("found {} int64s", count);
                let mut integers: Vec<i64> = vec![];
                for j in 0..count {
                    let mut buf: [u8; 8] = [0; 8];
                    buf.copy_from_slice(buffer.get(offset..offset+8).expect("Unable to read int64 datum"));
                    let big = i64::from_be_bytes(buf);
                    //print!("\t datum {} is {}", j, big);
                    offset += 8;
                    integers.push(big);
                }
                data.data = Data::Integer64(integers);
            }
            Err(bits) => {
               // println!("Invalid bytes {}", bits.invalid_bytes);
            }
        }
        dis.data.push(data);
        i += 1;
    }

    // read module name

    loop {
        let c = buffer.get(offset..offset+1).expect("Unable to read module name")[0];
        offset += 1;
        if (c == 0) {
            break;
        }
        dis.module_name.push(c as char)
    }
    //println!("Module Name: {}", dis.module_name);

    // link/export section

    for i in 0..dis.link_size {
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
            if (c == 0) {
                break;
            }
            export.fn_name.push(c as char);
        }

        dis.exports.push(export);
    }

    // import section

    if (dis.runtime_flag.to_i16().unwrap() & DisRuntimeFlag::HASLDT2.to_i16().unwrap() != 0) {
        let module_import_count = parse_operand(buffer.get(offset..offset+4).expect("Unable to read import module count"));
        offset += module_import_count.size();
        let module_import_count = module_import_count.into_usize();

        for i in 0..module_import_count {
            let mut module_imports = ModuleImport::new();

            let import_count = parse_operand(buffer.get(offset..offset+4).expect("Unable to read import count"));
            offset += import_count.size();
            let import_count = import_count.into_usize();

            for j in 0..import_count {
                let mut import = Import::new();

                import.sig = parse_word(buffer.get(offset..offset+4).expect("Unable to read import type checksum"));
                offset += 4;

                loop {
                    let c = buffer.get(offset..offset+1).expect("Unable to read import function name")[0];
                    offset += 1;
                    if (c == 0) {
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

    if (dis.runtime_flag.to_i16().unwrap() & DisRuntimeFlag::HASEXCEPT.to_i16().unwrap() != 0) {
        let mut handler = HandlerSection::new();

        let handler_offset = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler offset"));
        offset += handler_offset.size();
        handler.frame_offset = handler_offset.into_i32();

        let pc_start = parse_operand(buffer.get(offset..offset+4).expect("Unable to read handler pc start"));
        offset += pc_start.size();
        handler.pc_start = pc_start.into_i32();;

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

        for i in 0..handler_count {
            let mut exception_handler = ExceptionHandler::new();
            loop {
                let c = buffer.get(offset..offset+1).expect("Unable to read exception handler function name")[0];
                offset += 1;
                if (c == 0) {
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
        if (c == 0) {
            break;
        }
        dis.module_src_path.push(c as char)
    }

    //println!("{}", dis.module_src_path);

    println!("Module {:#?}", dis);

    Ok(())
}