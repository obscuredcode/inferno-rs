use std::alloc::{Layout, LayoutError};
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::AddAssign;
use std::ptr::NonNull;
use std::rc::Rc;
use libc::pgn_t;
use module::{DataSection, Type};
use module::TypeMap;
use data::Location::Module;
use modular_bitfield::prelude::*;
use modular_bitfield::private::static_assertions::const_assert;
use util::BufferedReader;


pub mod ctypes {
    const STRUCTALIGN: usize = 4;
    #[repr(C)]
    pub struct String {
        pub(crate) len: i32,
        pub(crate) max: i32,
        pub(crate) _tmp: i32,
        // char*
        pub(crate) data: [u8; STRUCTALIGN],
    }

    #[repr(C)]
    pub struct Type {
        _ref: i32,
        free: i32,
        size: i32,
        np: i32,
        _destroy: i32,
        _initialize: i32,
        map: [u8; STRUCTALIGN]
    }
}
const PointerMap: i32 = 0x80;

const Tstring: Type = Type {
    desc_no: 0,
    size: std::mem::size_of::<String>() as i32,
    map_size: 0,
    map: TypeMap { bytes: vec![] },
};

#[derive(Copy, Clone)]
#[bitfield(bits=8)]
pub struct DataCode {
    #[bits=4]
    count: B4,
    #[bits=4]
    pub data_type: DataCodeType,
}

impl Debug for DataCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.data_type_or_err() {
            Ok(dt) => {
                write!(f, "DataCode {{ size: {}, type: {:?} }}", self.count(), dt)
            }
            Err(bit) => {
                write!(f, "DataCode {{ unrecognized: {:?} }}", bit.invalid_bytes)
            }
        }
    }
}

impl Display for DataCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.data_type_or_err() {
            Ok(dt) => {
                write!(f, "DataCode {{ size: {}, type: {:?} }}", self.count(), dt)
            }
            Err(bit) => {
                write!(f, "DataCode {{ unrecognized: {:?} }}", bit.invalid_bytes)
            }
        }
    }
}

impl DataCode {
    pub fn is_zero(&self) -> bool {
        return self.bytes[0] == 0;
    }

    pub fn get_count(&self) -> u8 {
        return self.count();
    }
}

#[derive(Debug, Clone)]
pub enum Datum {
    Byte(u8),
    Integer32(i32),
    String(String),
    Float(f64),
    Array(Data),
    Integer64(i64)
}

#[derive(Clone)]
pub enum Data {
    Byte(Vec<u8>),
    Integer32(Vec<i32>),
    String(String),
    Float(Vec<f64>),
    Array(i32, i32, VmArray),
    Integer64(Vec<i64>),
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::Byte(v) => {
                write!(f, "{:?}", v)
            }
            Data::Integer32(v) => {
                write!(f, "{:?}", v)
            }
            Data::String(s) => {
                write!(f, "{:?}", s)
            }
            Data::Float(v) => {
                write!(f, "{:?}", v)
            }
            Data::Array(_, _, arr) => {
                write!(f, "{:?}", arr)
            }
            Data::Integer64(v) => {
                write!(f, "{:?}", v)
            }
        }
    }
}

#[derive(Clone)]
pub struct VmArray {
    typ: Type,
    count: usize,
    ptr: NonNull<u8>,
    layout: Layout
}

impl Default for VmArray {
    fn default() -> Self {
        Self {
            typ: Type {
                desc_no: 0,
                size: 0,
                map_size: 0,
                map: Default::default(),
            },
            count: 0,
            ptr: NonNull::<u8>::dangling(),
            layout: Layout::new::<u8>()
        }
    }
}

impl Debug for VmArray {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "size {}, count {}: [ ", self.size(), self.count)?;
        for i in 0..self.count {
            let a = i * self.align();
            write!(f, "{:?}", unsafe {self.read::<i32>(a)})?;
            write!(f, "{}", if i == self.count - 1 {
                                " "
                            } else {
                                ", "
                            })?;
        }
        write!(f, " ]")
    }
}
impl VmArray {
    pub fn align(&self) -> usize {
        self.typ.size as usize
    }
    pub fn size(&self) -> usize {
        self.count * self.align()
    }
    pub fn new(t: &Type, count: usize) -> Self {
        let size = t.size as usize;
        let (raw, layout) = unsafe {
            let layout = Layout::from_size_align_unchecked(size * count, 1);
            (std::alloc::alloc(layout), layout)
        };
        Self {
            typ: t.clone(),
            count: count,
            ptr: unsafe {NonNull::new_unchecked(raw)},
            layout: layout
        }
    }

    pub fn write<T>(&mut self, src: T, offset: usize) {
        let base = self.ptr.as_ptr();
        let dest = base.wrapping_add(offset);
        unsafe {
            std::ptr::write::<T>(dest as *mut T, src);
        }
    }

    pub fn read<T>(&self, offset: usize) -> T {
        let base = self.ptr.as_ptr();
        let src = base.wrapping_add(offset);
        unsafe {
            std::ptr::read::<T>(src as *const T)
        }
    }
}

#[derive(Debug)]
pub enum ModuleDataError {
    OutOfMemory,
    BadAllocation
}

impl Display for ModuleDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ModuleDataError {

}

impl From<LayoutError> for ModuleDataError {
    fn from(value: LayoutError) -> Self {
        ModuleDataError::BadAllocation
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
#[repr(i32)]
pub enum VmPtr {
    Ptr(i32)
}

impl VmPtr {
    pub fn as_i32(&self) -> i32 {
        match self { VmPtr::Ptr(i) => {*i} }
    }
}

#[derive(Debug, Copy, Clone)]
enum BasePtr {
    Module,
    Array {
        ptr: VmPtr,
        start_index: usize
    }
}

pub struct Heap {
    heap: NonNull<u8>,
    size: usize,
    pub(crate) ptr_table: std::collections::HashMap<VmPtr, Data>,
    allocated: Vec<VmPtr>,
    curr_ptr: i32,
    base: BasePtr,
    array_stack: Vec<BasePtr>,
    array_offset: usize,
    last_array: VmPtr
    //layout: Layout
}


impl Heap {
    /*pub unsafe fn new(module_type_desc: Type) -> Result<Self, ModuleDataError> {
        let size = module_type_desc.size as usize;
        let layout = Layout::from_size_align(size, 1)?;
        let ptr = std::alloc::alloc_zeroed(layout);

        let mut start = ptr as *mut i32;



        if ptr.is_null() {
            Err(ModuleDataError::OutOfMemory)
        } else {
            Ok(Self {
                heap: NonNull::new_unchecked(ptr),
                layout: layout
            })
        }
    }*/
    pub fn new(module_type_desc: &Type) -> Self {
        let size = module_type_desc.size as usize;
        unsafe {
            let raw = libc::mmap(
                (1u64 << 16) as *mut libc::c_void,
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_FIXED_NOREPLACE |
                    libc::MAP_ANONYMOUS | libc::MAP_32BIT,
                    -1,
                0

            );
            if raw.is_null() {
                panic!("Unable to create VM Heap");
            }

            Self::setup_pointers(raw as *mut u8, module_type_desc);

            Self {
                heap: NonNull::new_unchecked(raw as *mut u8),
                size,
                ptr_table: Default::default(),
                allocated: Default::default(),
                curr_ptr: 0,
                base: BasePtr::Module,
                array_stack: Default::default(),
                array_offset: 0,
                last_array: VmPtr::Ptr(-1)
            }
        }
    }

    pub fn setup_pointers(heap: *mut u8, t: &Type) {
        let mut start = heap as *mut i32;
        for m in &t.map.bytes {
            if *m != 0 {
                for i in 0..8 {
                    if (m >> (7-i)) & 0x01 == 0x01 {
                        unsafe {
                            start.wrapping_add(i).write(-1);
                        }
                    }
                }
            }
            start = start.wrapping_add(8);
        }
    }
    pub fn write_offset<T>(&mut self, src: T, offset: usize) {
        let base = self.heap.as_ptr();
        let dest = base.wrapping_add(offset);
        unsafe {
            std::ptr::write::<T>(dest as *mut T, src);
        }
    }

    pub fn read_offset<T>(&self, offset: usize) -> T {
        let base = self.heap.as_ptr();
        let src = base.wrapping_add(offset);
        unsafe {
            std::ptr::read::<T>(src as *const T)
        }
    }

    pub fn allocate(&mut self) -> VmPtr {
        let ptr = VmPtr::Ptr(self.curr_ptr);
        self.curr_ptr += 1;
        self.allocated.push(ptr);
        ptr
    }
    pub fn write<T: Debug>(&mut self, src: T, offset: usize) {
        match self.base {
            BasePtr::Module => {
                self.write_offset(src, offset);
            }
            BasePtr::Array { ptr, start_index } => {
                let Data::Array(t, c, v) =
                    self.ptr_table.get_mut(&ptr).unwrap() else {panic!("")};
                let dest = offset + self.array_offset;
                let size = std::mem::size_of_val(&src);
                v.write(src, dest);

                //println!("writing {:?} at {} to {:?}", src, dest, v);
                self.array_offset += size;
            }
        }

    }

    pub fn read<T>(&self, offset: usize) -> T {
        self.read_offset(offset)
    }


    pub fn get(&self, offset: usize) -> Option<&Data> {
        let ptr: VmPtr = VmPtr::Ptr(self.read::<i32>(offset));
        if ptr.as_i32() == -1 {
            return None;
        }
        self.ptr_table.get(&ptr)
    }

    pub fn set_index(&mut self, array_index: usize) {
        let old = self.base;
        self.array_stack.push(old);
        self.base = BasePtr::Array {
            ptr: self.last_array,
            start_index: array_index,
        };
            /*match self.base {
            BasePtr::Module => {panic!("set index not following array")}
            BasePtr::Array { ptr, start_index } => {} };*/

        self.array_offset = array_index;
        //println!("set base pointer from {:?} to {:?}", old, self.base);
    }

    pub fn restore(&mut self) {
        let new = self.array_stack.pop().expect("called restore before set index");
        //println!("restoring baseptr from {:?} to {:?}", self.base, new);
        self.base = new;
    }
    pub fn write_data(&mut self, data: Data, offset: usize) -> VmPtr {
        let ptr = self.allocate();
        self.ptr_table.insert(ptr, data);
        self.write(ptr.as_i32(), offset);
        ptr
    }

    pub fn load_data(&mut self,
                     code: DataCode,
                     count: usize,
                     type_descs: &Vec<Type>,
                     buffer: Rc<RefCell<BufferedReader>>) {
        let mut buffer = buffer.borrow_mut();
        let mut data = DataSection::new();
        let data_offset = buffer.operand();

        let offset = data_offset.into_usize();
        data.offset = offset as i32; //data_offset.into_i32();
        data.code = code;

        match data.code.data_type_or_err() {
            Ok(DataCodeType::Byte) => {
                for _j in 0..count {
                    self.write(buffer.u8e("Unable to read data byte"), offset);
                }
                //println!("writing byte at {}", offset);
            }
            Ok(DataCodeType::Integer32) => {
                for _j in 0..count {
                    self.write(buffer.i32(), offset)
                }
                //println!("writing int at {} from {}", offset, buffer.offset);
            }
            Ok(DataCodeType::StringUTF) => {
                let chars = String::from_utf8_lossy(buffer.reade(count, "Unable to read utf8 string"));
                let data = Data::String(chars.to_string());
                let ptr = self.write_data(data, offset);
                //println!("wrote str {:?} ptr {:?} at offset {}", chars, ptr, offset);
            }
            Ok(DataCodeType::Float) => {
                let mut floats: Vec<f64> = vec![];
                for _j in 0..count {
                    let float = buffer.f64();
                    floats.push(float);
                }
            }
            Ok(DataCodeType::Array) => {
                let array_type = buffer.i32e("Unable to read array type");
                if array_type < 0 {
                    panic!("Invalid array type {array_type}");
                }
                let array_elements_count = buffer.i32e("Unable to read array elements count");
                if array_elements_count < 0 {
                    panic!("Invalid array elements count {}", array_elements_count)
                }
                //println!("array cap {}", array_elements_count);
                let t = &type_descs[array_type as usize];
                let data = Data::Array(array_type, array_elements_count,
                                       VmArray::new(t, array_elements_count as usize));
                let ptr = self.write_data(data, offset);
                /*self.base = BasePtr::Array {
                    ptr,
                    start_index: 0,
                };*/
                self.last_array = ptr;
                //println!("started array ptr {:?} at offset {}", ptr, offset);
            }
            Ok(DataCodeType::ArraySetIndex) => {
                let array_index = buffer.i32e("Unable to read array base address");
                if array_index < 0 {
                    panic!("Invalid set array index {}", array_index);
                }
                self.set_index(array_index as usize);
                //println!("set index at offset {}", offset);
            }
            Ok(DataCodeType::RestoreBase) => {
                self.restore();
                //println!("-------------");
            }
            Ok(DataCodeType::Integer64) => {
                let mut integers: Vec<i64> = vec![];
                for _j in 0..count {
                    let big = buffer.i64e("Unable to read int64 datum");
                    integers.push(big);

                }
                data.data = Data::Integer64(integers);
            }
            Err(err) => {
                panic!("Invalid DataCode Type: {:?}", err.invalid_bytes)
            }
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            //std::alloc::dealloc(self.heap.as_ptr(), self.layout);
            libc::munmap(self.heap.as_ptr() as *mut libc::c_void, self.size);
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


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Location {
    Module(usize),
    Array(usize)
}

impl Location {
    pub fn as_i32(&self) -> i32 {
        match *self {
            Location::Module(i) => {i as i32}
            Location::Array(i) => {i as i32}
        }
    }
    pub fn as_usize(&self) -> usize {
        match *self {
            Module(i) => {i}
            Location::Array(i) => {i}
        }
    }
}

impl AddAssign<usize> for Location {
    fn add_assign(&mut self, rhs: usize) {
        *self = match *self {
            Module(i) => {Location::Module(i+rhs)}
            Location::Array(i) => {Location::Array(i+rhs)}
        }
    }
}

#[derive(Debug)]
pub struct ModuleGlobalData {
    pub map: std::collections::HashMap<i32, Data>,
    array_stack: Vec<Location>,
    base: Location,
    current: Location
}

impl ModuleGlobalData {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
            array_stack: Vec::with_capacity(16),
            base: Module(0),
            current: Module(0)
        }
    }
    pub fn set_index(&mut self, array_index: usize, type_descs: &Vec<Type>) {
        // the last thing we added must have been an array
        let current_datum= self.current.as_i32();

        /*let mut arr = self.map.get_mut(&current_datum).expect("array must precede set index");

        let (c, t): (i32, i32) = match arr {
            Data::Array(c, t, _) => (*c, *t),
            _ => panic!("set index not following array")
        };

        if c < 0 || t < 0 {
            panic!("invalid array {:?}", arr)
        }

        let t: &Type = &type_descs[t as usize];

        let array_start = array_index;*/
        self.array_stack.push(self.base);

        self.base = Location::Array(current_datum as usize);

    }



    pub fn restore(&mut self) {
        self.base = self.array_stack.pop().expect("no array base address to restore");
    }

    pub fn load_data(&mut self,
                     count: usize,
                     data: &mut DataSection,
                     type_descs: &Vec<Type>,
                     buffer: Rc<RefCell<BufferedReader>>) {
        let mut buffer = buffer.borrow_mut();
        let offset = data.offset as usize;
        let in_array = match self.base {
            Module(_) => { false }
            Location::Array(_) => { true }
        };
        let mut do_nothing = false;
        //self.current = self.base;
        match data.code.data_type_or_err() {
            Ok(DataCodeType::Byte) => {
                //println!("Found {} bytes", count);
                let mut bytes: Vec<u8> = vec![];
                for _j in 0..count {
                    let byte = buffer.u8e("Unable to read data byte");
                    bytes.push(byte);
                    if in_array {
                        self.put(offset, Datum::Byte(byte));
                    }

                }
                data.data = Data::Byte(bytes);
            }
            Ok(DataCodeType::Integer32) => {
                //println!("Found {} integers", count);
                let mut integers: Vec<i32> = vec![];
                for _j in 0..count {
                    let word = buffer.i32();
                    integers.push(word);
                    if in_array {
                        self.put(offset, Datum::Integer32(word));
                    }
                }
                data.data = Data::Integer32(integers);
            }
            Ok(DataCodeType::StringUTF) => {
                let chars = String::from_utf8_lossy(buffer.reade(count, "Unable to read utf8 string"));
                if in_array {
                    self.put(offset, Datum::String(chars.to_string()))
                }
                data.data = Data::String(chars.to_string());
            }
            Ok(DataCodeType::Float) => {
                let mut floats: Vec<f64> = vec![];
                for _j in 0..count {
                    let float = buffer.f64();
                    floats.push(float);
                    if in_array {
                        self.put(offset, Datum::Float(float));
                    }
                }
                data.data = Data::Float(floats);
            }
            Ok(DataCodeType::Array) => {
                let array_type = buffer.i32e("Unable to read array type");
                if array_type < 0 {
                    panic!("Invalid array type {array_type}");
                }
                let array_elements_count = buffer.i32e("Unable to read array elements count");
                if array_elements_count < 0 {
                    panic!("Invalid array elements count {}", array_elements_count)
                }

                if in_array {
                    self.put(offset, Datum::Array(Data::Array(array_type, array_elements_count,VmArray::default())))
                }
                data.data = Data::Array(array_type, array_elements_count, VmArray::default());
            }
            Ok(DataCodeType::ArraySetIndex) => {
                let array_index = buffer.i32e("Unable to read array base address");
                if array_index < 0 {
                    panic!("Invalid set array index {}", array_index);
                }
                self.set_index(array_index as usize, type_descs);
                do_nothing = true;
            }
            Ok(DataCodeType::RestoreBase) => {
                self.restore();
                do_nothing = true;
            }
            Ok(DataCodeType::Integer64) => {
                let mut integers: Vec<i64> = vec![];
                for _j in 0..count {
                    let big = buffer.i64e("Unable to read int64 datum");
                    integers.push(big);
                    if in_array {
                        self.put(offset, Datum::Integer64(big))
                    }
                }
                data.data = Data::Integer64(integers);
            }
            Err(err) => {
                panic!("Invalid DataCode Type: {:?}", err.invalid_bytes)
            }
        }
        /*println!("offset {}: {:?}",
                 data.offset,
                 &data.data);*/
        if !do_nothing {
            self.set(offset, data.data.clone());
        }
    }

    pub fn write(&mut self, offset: usize, datum: Datum) {
        match self.base {
            Module(off) => {
                let dest = (off + offset) as i32;
                let d = self.map.get_mut(&dest).expect("offset should be valid");
            }
            Location::Array(arr) => {

            }
        }
    }

    pub fn put_new(&mut self, offset: usize, datum: Datum) {
        let off = (offset as i32);
        match datum {
            Datum::Byte(b) => {
                self.map.insert(off, Data::Byte(vec![b]));
            }
            Datum::Integer32(i) => {
                self.map.insert(off, Data::Integer32(vec![i]));
            }
            Datum::String(s) => {
                self.map.insert(off, Data::String(s));
            }
            Datum::Float(_) => {}
            Datum::Array(data) => {
                self.map.insert(off, data);
            }
            Datum::Integer64(_) => {}
        };
    }

    pub fn put(&mut self, offset: usize, datum: Datum) {
        let data = match self.base {
            Module(_) => {
                self.map.get_mut(&(offset as i32))
            }
            Location::Array(i) => {
                self.map.get_mut(&(i as i32))
            }
        };

        match data {
            Some(Data::Array(_, _,ref mut v)) => {
                //v.push(datum)
                todo!("")
            }
            Some(Data::Byte(_)) => {

            }
            Some(Data::Integer32(v)) => {
                let Datum::Integer32(i) = datum else {panic!("")};
                v.push(i);
            }
           Some(Data::String(ref mut v)) => {
                let Datum::String(s) = datum else {panic!("")};
                *v = s;
            }
            Some(Data::Float(_)) => {

            }

            Some(Data::Integer64(_)) => {

            }
            None => {
                self.put_new(offset, datum);
            }
        }
    }

    pub fn set(&mut self, offset: usize, data: Data) {
        self.current = Module(offset);
        self.map.insert(offset as i32, data);
    }
    pub fn get(&self, offset: usize) -> Option<&Data> {
        self.map.get(&(offset as i32))
    }
}