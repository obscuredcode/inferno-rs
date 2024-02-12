
use std::alloc::{AllocError, Layout, LayoutError};
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ptr::NonNull;
use std::rc::Rc;
use module::{DataSection, Type};
use module::TypeMap;
use modular_bitfield::prelude::*;
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

const MP_HEAP_START: u64 = 8u64 << 16;
const HEAP_START: u64 = 1u64 << 8;

const Tstring: Type = Type {
    desc_no: 0,
    size: std::mem::size_of::<String>() as i32,
    map_size: 0,
    map: TypeMap { bytes: vec![] },
};

//<editor-fold desc="ModuleData Data Types">
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
        return self.count() as u8;
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
//</editor-fold>

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
impl Drop for VmArray {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.ptr.as_ptr(), self.layout);
        }
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

pub struct ModuleData {
    heap: NonNull<u8>,
    size: usize,
    pub(crate) ptr_table: std::collections::HashMap<VmPtr, Data>,
    allocated: Vec<VmPtr>,
    curr_ptr: i32,
    base: BasePtr,
    array_stack: Vec<BasePtr>,
    array_offset: usize,
    last_array: VmPtr,
    heap_start: usize,
}

const HEAP_SIZE: usize = 1024*1024;

impl ModuleData {
    pub fn new(module_type_desc: &Type) -> Self {
        let heap_start = module_type_desc.size as usize;
        let size =  heap_start + HEAP_SIZE;
        println!("MP Heap size: {}", size);
        unsafe {
            let raw = libc::mmap(
                MP_HEAP_START as *mut libc::c_void,
                size+HEAP_SIZE,
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
                last_array: VmPtr::Ptr(-1),
                heap_start: heap_start
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
    pub fn read_ptr(&self, ptr: &VmPtr) -> Option<&Data> {
        let mut data: Option::<&Data> = None;

        if (ptr.as_i32() != - 1) {
            data = self.ptr_table.get(&ptr)
        };

        data
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
        self.read_ptr(&ptr)
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

        //<editor-fold desc="Loading data into heap">
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
        //</editor-fold>
    }
}

impl Drop for ModuleData {
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


pub struct HeapPtr {
    ptr: u32
}

impl HeapPtr {
    pub fn as_ptrT<T>(&self) -> *const T {
        self.ptr as *const T
    }
    pub fn as_ptrT_mut<T>(&self) -> *mut T {
        self.ptr as *mut T
    }
}



//<editor-fold desc="Heap Allocator">
/*pub struct Heap {
    heap: NonNull<u8>,
    size: usize,
    top: usize,
}

impl Heap {
    pub fn new(module_type_desc: &Type) -> Self {
        let size = module_type_desc.size as usize;
        println!("Heap size: {}", size);
        unsafe {
            let raw = libc::mmap(
                HEAP_START as *mut libc::c_void,
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

            Self {
                heap: NonNull::new_unchecked(raw as *mut u8),
                size,
                top: 0,
            }
        }
    }
}

#[allow(unstable_features)]
unsafe impl std::alloc::Allocator for Heap {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let ptr = unsafe {
            NonNull::new_unchecked(self.heap.as_ptr().add(self.top))
        };
        self.top += size;
        Ok(ptr)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        todo!()
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            //std::alloc::dealloc(self.heap.as_ptr(), self.layout);
            libc::munmap(self.heap.as_ptr() as *mut libc::c_void, self.size);
        }
    }
}*/
//</editor-fold>
