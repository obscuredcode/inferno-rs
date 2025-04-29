
use std::alloc::{Layout, LayoutError};
use std::cell::{RefCell, UnsafeCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::iter::Map;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;
use module::{DataCode, DataCodeType, DataSection, Type};
use module::TypeMap;
use modular_bitfield::prelude::*;
use mem::vm_types::Array;
use util::BufferedReader;

pub mod heap;

pub mod vm_types {
    pub type Ptr = u32;
    const STRUCTALIGN: usize = 4;
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct String {
        pub(crate) len: i32,
        pub(crate) max: i32,
        pub(crate) _tmp: i32,
        // char*
        pub(crate) data: Ptr,
    }
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct Array {
        pub len: i32,
        pub t: u32,
        pub root: Ptr,
        pub data: Ptr,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct List {
        tail: Ptr,
        t: Ptr,
        data: Ptr,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct Type {
        ref_count: i32,
        free: i32,
        size: i32,
        np: i32,
        _destroy: Ptr,
        _initialize: Ptr,
        map: Ptr // ptr
    }

}
const POINTER_MAP: i32 = 0x80;

const MP_HEAP_START: u64 = 8u64 << 16;
const HEAP_START: u64 = 1u64 << 8;

const Tstring: Type = Type {
    desc_no: 0,
    size: std::mem::size_of::<String>() as i32,
    map_size: 0,
    map: TypeMap { bytes: vec![] },
    rc: 0
};

//<editor-fold desc="ModuleData Data Types">

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
    Array(VmPtr,Array),
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
            Data::Array(.., arr) => {
                write!(f, "{:?}", arr)
            }
            Data::Integer64(v) => {
                write!(f, "{:?}", v)
            }
        }
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        // TODO: nothing for now
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
    fn from(_value: LayoutError) -> Self {
        ModuleDataError::BadAllocation
    }
}
//</editor-fold>


// TODO: get rid of this and rewrite Data and Datum

//<editor-fold desc="VmArray">
#[derive(Clone)]
pub struct VmArray {
    typ:  Type,
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
                rc: 0
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


//</editor-fold>

type Ptr32 = u32; // 32-bit pointer outside of VM Heap
pub const VM_NULLPTR: Ptr32 = u32::MAX as Ptr32; // Dis likes this for empty pointers

#[repr(C, align(8))]
struct Header {
    upper_pointer: u32, // this will probably be entirely unused so i don't have to worry about alignment
    size: u32, // TODO: does this go here
    refcount: u32, // TODO: does this go here
    typ: VmType, // TODO: does this go here
}

#[repr(u32)]
pub enum VmType {
  null = 0,
  Tbyte = 1,
  Tptr = 2,
  Tstring = 3,
  Tarray = 4,
}

#[repr(C, align(8))]
pub struct VmObject {
    header: Header,
    data: Ptr32
}

impl VmObject {
    pub fn new() -> Self {
        Self {
            header: Header {
                upper_pointer: 0,
                size: 0,
                refcount: 0,
                typ: VmType::Tbyte,
            },
            data: 0,
        }
    }
    pub fn header_size() -> usize {
        std::mem::size_of::<VmObject>()
    }
    pub fn size(&self) -> usize {
        VmObject::header_size() + self.header.size as usize
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash, Ord, PartialOrd)]
pub struct VmPtr<T = u8> {
    ptr: u32,
    _t: PhantomData<T>,
}

impl<T> VmPtr<T> {

    pub fn new(p: u32) -> Self {
        Self {
            ptr: p,
            _t: PhantomData,
        }
    }
    pub fn as_u32(&self) -> u32 {
        self.ptr
    }

    pub fn as_generic(&self) -> VmPtr {
       VmPtr {
           ptr: self.as_u32(), _t: PhantomData
       }
    }

    pub fn as_type<A>(&self) -> VmPtr<A> {
        VmPtr::<A>::new(self.as_u32())
    }
    pub fn add(&self, offset: usize) -> Self {
        VmPtr::new(self.ptr + offset as u32)
    }
    pub fn offset(&mut self, offset: u32) {
        self.ptr += offset;
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

#[derive(Clone)]
pub struct VmAllocation {
    offset: usize,
    code: DataCode,
    size: usize,
    align: usize,
    base: usize,
    count: usize,
    data: Data
}

impl Debug for VmAllocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {

        write!(f,"VmAllocation: offset {}, code ({}, {:?}), size {}, align {}\n data: {:?}",
               self.offset, self.count, self.code.data_type_or_err().unwrap(), self.size, self.align, self.data)
    }
}


// there is no need to deal with 64-bit pointers, as all calls to system libraries
// go through handlers that translate system objects to the VM"s 32-bit memory
// this also means there is no need to worry about aligning memory access to
// 64-bit pointers

// | Ptr32<VmObject> | Pt32r<VmObject>> | Ptr32<VmObject> |

// VmObject
// metadata
//  refcount?
// pointer
//  32-bit VmPpr


// These VmObjects can then actually be held somewhere were they can
// be garbage-collected

// SAFETY: this is better than giving out raw pointers, because the compiler will remember to drop the VmObject
// and we get interior mutability of the Vec.
// we just wanna make sure we are inserting the allocations appropriately

// Vec<UnsafeCell<VmObject>>

// TODO: separate Heap, 32-bit general Allocator, and contiguous Memory
#[derive(Debug)]
pub struct ModuleHeap {
    heap: NonNull<u8>, // actual start of contiguous Heap memory allocation
    size: usize, // total size of Heap memory allocation
    base: BasePtr, // used to distinguish whether data is being written as a Ptr or into an Array
    array_stack: VecDeque<BasePtr>, // stack for dealing with recursive arrays
    array_offset: usize, // used to keep track of where in array we are. this should be part of BasePtr
    last_array: VmPtr, // used to know what array should be written to. also should be part of BasePtr
    heap_start: usize, // where is free contiguous memory in the heap after module data
    // VmObject Stuff
    allocations: Vec<UnsafeCell<VmObject>>,
    pub layout_map: BTreeMap<VmPtr, VmAllocation>

}

const HEAP_SIZE: usize = 1024*1024; //  4GB

impl ModuleHeap {
    pub fn new(desc: &mut Type) -> Self {
        let heap_start = desc.size as usize;
        let size =  heap_start + HEAP_SIZE/2;
        println!("Heap size: {}, Module Data Size {}", size, heap_start);
        unsafe {
            let raw = libc::mmap(
                0x80000 as *mut libc::c_void,
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
            // TODO: switch to  VmObject
            //desc.rc += 1;
            Self::setup_pointers(raw as *mut u8, desc);

            Self {
                heap: NonNull::new_unchecked(raw as *mut u8),
                size,
                base: BasePtr::Module,
                array_stack: Default::default(),
                array_offset: 0,
                last_array: VmPtr::new(0),
                heap_start: heap_start,
                allocations: vec![],
                layout_map: BTreeMap::new(),
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


    pub fn allocate_bytes(&mut self, n: usize) -> VmPtr {
        let raw = unsafe { self.heap.as_ptr().add(self.heap_start) } ;
        let ptr = VmPtr::new(raw as u32);
        let mut size = n;
        if (size % 4 != 0) {
            size = (size/4 + 1) * 4;
        }
        self.heap_start += size;
        ptr
    }

    pub fn allocate<T>(&mut self, t: T) -> VmPtr {
        let raw = unsafe { self.heap.as_ptr().add(self.heap_start) };
        let ptr = VmPtr::new(raw as u32);
        let size = std::mem::size_of::<T>();
        self.heap_start += size;
        ptr
    }

   /*pub fn allocaten(&mut self, n: usize) -> VmPtr {
        let header_size = std::mem::size_of::<VmObject>();
        let dest = self.allocate_bytes(header_size + n);
        self.write_ptr(
            VmObject {
            header:
                Header {
                    upper_pointer: 0, size: n as u32, typ: 0, refcount: 0
                },
            }, dest);
        dest
    }   */

/*
    fn create_obj<T>(&mut self, t: T) -> VmPtr {
        //let src = ptr.as_u32() as Ptr32;
        let size = std::mem::size_of_val(&t);

        let obj = UnsafeCell::new(VmObject {
            header: Header {
                upper_pointer: 0,
                size: 0,
                refcount: 0,
                typ: 0,
            },
            ptr: 0,
        });


        self.allocations.push(obj);

        VmPtr::Ptr(0)
    }

    pub fn translate(&self, ptr: VmPtr) {
        // ptr will always be a 32-bit pointer
        // use ptr to get term

        // read term and return actual pointer

    }
     */

    pub fn nheap(&mut self, n: usize) -> VmPtr<VmObject> {
       let header_size = std::mem::size_of::<VmObject>();
       let dest = self.allocate_bytes(header_size+n).as_type();
       self.write(VmObject {
           header: Header {
               upper_pointer: 0,
               size: 0,
               refcount: 0,
               typ: VmType::null,
           },
           data: 0
       }, &dest);
       dest
    }

    pub fn H2D<T>(&mut self, p: &VmPtr<VmObject>) -> VmPtr<T> {
        VmPtr::<T>::new(p.as_u32() + VmObject::header_size() as u32)

    }

    pub fn write<T>(&mut self, t: T, a: &VmPtr<T>) {
        unsafe {
            let raw = a.ptr as *mut T;
            std::ptr::write::<T>(raw, t);
        }
    }

    pub fn read<T>(&self, a: &VmPtr<T>) -> T {
        unsafe {
            std::ptr::read::<T>(a.ptr as *const T)
        }
    }

    pub fn read_bytes<T>(&self, a: VmPtr, n: usize) -> &[T] {
        unsafe {
            &*(std::ptr::slice_from_raw_parts(a.ptr as *const T, n))
        }
    }



    pub fn write_heap<T>(&mut self, src: T, dest: &VmPtr<T>) {
        let header_size = std::mem::size_of::<VmObject>();
        self.write(src, &dest.add(header_size));
    }

    pub fn read_heap<T>(&self, src: &VmPtr<T>) -> T {
        let header_size = std::mem::size_of::<VmObject>();
        self.read(&src.add(header_size))
    }

    pub fn read_n<T>(&self, src: &VmPtr, n: usize) -> &[T]{
        let header_size = std::mem::size_of::<VmObject>();
        self.read_bytes(src.add(header_size), n)
    }

    pub fn write_ptr<T,H>(&mut self, ptr: &VmPtr<T>, dest: &VmPtr<H>) {
        self.write(ptr.as_u32(), &dest.as_type())
    }

    pub fn write_module_data<T>(&mut self, src: T, offset: usize) {
        if offset > self.heap_start {
            panic!("module data written outside of beginning of heap {}", offset);
        }
        let dest: VmPtr<T> = VmPtr::new(self.heap.as_ptr().wrapping_add(offset) as u32);

        self.write(src, &dest);
    }

    pub fn read_module_data<T>(&self, offset: usize) -> T {
        if offset > self.heap_start {
            panic!("module data read outside of beginning of heap {}", offset);
        }
        let src: VmPtr<T> = VmPtr::new(self.heap.as_ptr().wrapping_add(offset) as u32);
        self.read(&src)
    }

    //pub fn write<T: Debug>(&mut self, src: T, offset: usize) {


    /*pub fn read<T>(&self, offset: usize) -> T {
        self.read_offset(offset)
    }*/


    pub fn get(&self, offset: usize) -> Option<VmPtr> {
        let ptr: VmPtr = VmPtr::new(self.read_module_data(offset));
        if ptr.as_u32() == 0 {
            return None;
        }
        //let raw = unsafe {ptr.as_u32() as *const vm_types::String};

        let cstr =self.read_heap::<vm_types::String>(&ptr.as_type());
        let str = unsafe {
            let b = self.read_n::<u8>(&VmPtr::new(cstr.data as u32), cstr.len as usize);
            let mut buffer: Vec<u8> = Vec::new();
            for i in 0..cstr.len as usize {
                buffer.push(b[i]);
            }
            //String::from_raw_parts(cstr.data as *mut u8, cstr.len as usize, cstr.max as usize)
            String::from_utf8_unchecked(buffer)
            //String::from_utf8_lossy(std::ptr::slice_from_raw_parts(&b, cstr.len as usize)).to_string()
        };
        println!("gotten '{}'", str);
        Some(ptr)
    }

    //<editor-fold desc="array operations">
    pub fn set_index(&mut self, array_index: usize) {
        let old = self.base;
        self.array_stack.push_back(old);
        let array = self.read_heap::<Array>(&self.last_array.as_type());

        self.base = BasePtr::Array {
            ptr: VmPtr::new(array.data),
            start_index: array_index,
        };
            /*match self.base {
            BasePtr::Module => {panic!("set index not following array")}
            BasePtr::Array { ptr, start_index } => {} };*/

        self.array_offset = array_index;
        //println!("set base pointer from {:?} to {:?}", old, self.base);
    }

    pub fn restore(&mut self) {
        let new = self.array_stack.pop_back().expect("called restore before set index");
        //println!("restoring baseptr from {:?} to {:?}", self.base, new);
        self.base = new;
    }
    //</editor-fold>

    pub fn new_string(&mut self, str: &String) -> VmPtr<vm_types::String> {
        let count = str.len();
        let string_size = std::mem::size_of::<vm_types::String>();
        let total_size = count + string_size;
        let vm_obj = self.nheap(total_size);
        self.write(VmObject {
            header: Header {
                upper_pointer: 0,
                size: total_size as u32,
                refcount: 0,
                typ: VmType::Tstring,
            },
            data: vm_obj.as_u32() + VmObject::header_size() as u32,
        }, &vm_obj);
        let dest = self.H2D::<vm_types::String>(&vm_obj);
        let mut str_start = dest.add(string_size);

        let mut string = vm_types::String {
            len: count as i32,
            max: count as i32,
            _tmp: 0,
            data: 0,
        };

        string.data = str_start.as_u32();
        self.write(string, &dest);


        for b in str.bytes() {
            self.write(b, &str_start.as_generic());
            str_start = str_start.add(1);
        }

        let vm = self.read(&vm_obj);
        let str = self.read(&dest);
        dest
    }



    pub fn write_data(&mut self, data: Data, offset: usize) -> VmPtr {
        if offset > self.heap_start {
            panic!("module data written outside of beginning of heap {}", offset);
        }
        let base = match self.base {
            BasePtr::Module => {
                self.heap.as_ptr() as u32
            }
            BasePtr::Array {ptr ,start_index } => {
                ptr.as_u32()+start_index as u32
            }
        };

        let dest: VmPtr = VmPtr::new(base.wrapping_add(offset as u32));
        assert!(dest.as_u32() < self.heap.as_ptr() as u32 + self.heap_start as u32);

        match data {
            Data::String(ref str) => {
                let str = self.new_string(str);
                let s = std::mem::size_of::<vm_types::String>();
                self.write_ptr(&str, &dest);
            }

            Data::Byte(ref byte) => {
                for i in 0..byte.len() {
                    self.write(byte[i], &dest.add(i*1).as_type());
                }
            }
            Data::Integer32(ref int) => {
                for i in 0..int.len() {
                    self.write(int[i], &dest.add(i*4).as_type());
                }

            }
            Data::Integer64(ref long) => {
                for i in 0..long.len() {
                    self.write(long[i], &dest.add(i*8).as_type());
                }
            }
            Data::Float(ref float) => {
                for i in 0..float.len() {
                    self.write(float[i], &dest.add(i*8).as_type());
                }
            }
            Data::Array(ref ptr, ref arr) => {
                self.write_ptr(ptr, &dest);
            }
            _ => {

            }
        };



        dest.as_generic()
    }

    pub fn load_data(&mut self,
                     code: DataCode,
                     count: usize,
                     type_descs: &mut Vec<Type>,
                     buffer: Rc<RefCell<BufferedReader>>) {
        let mut buffer = buffer.borrow_mut();
        let mut data = DataSection::new();
        let data_offset = buffer.operand();

        let offset = data_offset.into_usize();
        data.offset = offset as i32; //data_offset.into_i32();
        data.code = code;


        let mut alloc = VmAllocation {
            offset,
            code,
            size: 0,
            align: 0,
            base: 0,
            count,
            data: Data::Byte(vec![])
        };


        let print_data = true;
        //<editor-fold desc="Loading data into heap">
        match data.code.data_type_or_err() {
            Ok(DataCodeType::Byte) => {
                let mut bytes: Vec<u8> = Vec::with_capacity(count);
                for _j in 0..count {
                    //self.write(buffer.u8e("Unable to read data byte"), offset);
                    bytes.push(buffer.u8());
                    //self.write_datum(Datum::Byte(buffer.u8e("unable to read byte")), offset, j);
                }
                if(print_data) {
                    println!("writing {} byte at {} from {:?}: {:?}", count, offset, self.base, bytes);
                }
                alloc.data = Data::Byte(bytes.clone());
                let ptr = self.write_data(Data::Byte(bytes), offset);
                alloc.align = 1;
                alloc.size = count;
                //self.layout_map.insert(ptr, alloc);
             //   println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Ok(DataCodeType::Integer32) => {
                let mut ints: Vec<i32> = Vec::with_capacity(count);
                for j in 0..count {
                    //self.write(buffer.i32(), offset)
                    //println!("offset {} is {}: {}",offset, _j,buffer.i32());
                    let err = format!("unable to read int {} at offset {}", j, offset);
                    ints.push(buffer.i32e(&err));
                   // self.write_datum(Datum::Integer32(buffer.i32e(&err)), offset, j);
                }
                if(print_data) {
                    println!("writing {} int at {} from {:?}: {:?}", count, offset, self.base, ints);
                }
                alloc.data = Data::Integer32(ints.clone());
                let ptr = self.write_data(Data::Integer32(ints), offset);
                alloc.align = 4;
                alloc.size = 4*count;
                //self.layout_map.insert(ptr, alloc);
             //   println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Ok(DataCodeType::StringUTF) => {
                let chars = String::from_utf8_lossy(buffer.reade(count, "Unable to read utf8 string"));
                //println!("{}", chars.to_string());
                let data = Data::String(chars.to_string());
                alloc.data = data.clone();
                let ptr = self.write_data(data, offset);
                if (print_data) {
                    println!("wrote str {:?} ptr {:?} at offset {}", chars, ptr, offset);
                }
                alloc.align = 1;
                alloc.size = count;
                //self.layout_map.insert(ptr, alloc);
            //    println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Ok(DataCodeType::Float) => {
                let mut floats: Vec<f64> = Vec::with_capacity(count);
                for _j in 0..count {
                    //self.write_datum(Datum::Float(buffer.f64()), offset, j);
                    floats.push(buffer.f64())
                }
                if(print_data) {
                    println!("writing {} floats at {} from {:?}: {:?}", count, offset, self.base, floats);
                }
                alloc.data = Data::Float(floats.clone());
                let ptr = self.write_data(Data::Float(floats), offset);
                alloc.align = 4;
                //self.layout_map.insert(ptr, alloc);
            //    println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Ok(DataCodeType::Array) => {
                let array_type = buffer.i32e("Unable to read array type");
                if array_type < 0 {
                    panic!("Invalid array type {}", array_type);
                }
                let array_elements_count = buffer.i32e("Unable to read array elements count");
                if array_elements_count < 0 {
                    panic!("Invalid array elements count {}", array_elements_count)
                }
                //println!("array cap {}", array_elements_count);
                let t = &mut type_descs[array_type as usize];
                t.rc += 1;
                let array_size = t.size * array_elements_count;
                let vm_array_size = std::mem::size_of::<Array>() + array_size as usize;
                let array_ptr = self.nheap(vm_array_size);
                self.write(VmObject {
                    header: Header {
                        upper_pointer: 0,
                        size: vm_array_size as u32,
                        refcount: 0,
                        typ: VmType::Tarray,
                    },
                    data: array_ptr.add(VmObject::header_size()).as_u32(),
                }, &array_ptr);

                let vm_array =  Array {
                    len: array_elements_count,
                    t: array_type as u32,
                    root: self.last_array.as_u32(),
                    data: array_ptr.add(VmObject::header_size()+std::mem::size_of::<Array>()).as_u32(),
                };
                self.write_heap(vm_array, &array_ptr.as_type());

                let data = Data::Array(array_ptr.add(VmObject::header_size()).as_generic(), vm_array);
                alloc.data = data.clone();
                let ptr = self.write_data(data, offset);
                /*self.base = BasePtr::Array {
                    ptr,
                    start_index: 0,
                };*/
                self.last_array = array_ptr.as_generic();
                if (print_data) {
                    println!("started array ptr {:?} at offset {} len {} size {} --------------------",
                             ptr, offset, array_elements_count, t.size);
                }
                alloc.align = t.size as usize;
                //self.layout_map.insert(ptr, alloc);
              //  println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Ok(DataCodeType::ArraySetIndex) => {
                let array_index = buffer.i32e("Unable to read array base address");
                if array_index < 0 {
                    panic!("Invalid set array index {}", array_index);
                }
                self.set_index(array_index as usize);
                if (print_data) {
                    println!("set index {} at offset {} to base: {:?}", array_index,offset, self.base);
                }

            }
            Ok(DataCodeType::RestoreBase) => {
                self.restore();
                if (print_data) {
                    println!("------------ restore base to {:?}-", self.base);
                }

            }
            Ok(DataCodeType::Integer64) => {
                let mut bigs: Vec<i64> = vec![];
                for _j in 0..count {
                    let big = buffer.i64e("Unable to read int64 datum");
                    bigs.push(big);
                    //self.write_datum(Datum::Integer64(buffer.i64e("Unable to read int64 datum")), offset, _j);
                }
                if(print_data) {
                    println!("writing {} long at {} from {:?}: {:?}", count, offset, self.base, bigs);
                }
                alloc.data = Data::Integer64(bigs.clone());
                let ptr = self.write_data(Data::Integer64(bigs), offset);

                alloc.align = 8;
                //self.layout_map.insert(ptr, alloc);
                //println!("loaded alloc {:?} at {:?}\n", alloc, ptr)
            }
            Err(err) => {
                panic!("Invalid DataCode Type: {:?}", err.invalid_bytes)
            }
        }
        //</editor-fold>
    }

    pub fn module_data_start(&self) -> vm_types::Ptr {
        self.heap.as_ptr() as u32
    }
}

impl Drop for ModuleHeap {
    fn drop(&mut self) {
        unsafe {
            //std::alloc::dealloc(self.heap.as_ptr(), self.layout);
            libc::munmap(self.heap.as_ptr() as *mut libc::c_void, self.size);
        }
    }
}


pub fn vm_allocate(n: usize) -> vm_types::Ptr {
    let ptr = unsafe { libc::mmap(0x1_000_000 as *mut libc::c_void, n,
                                  libc::PROT_READ | libc::PROT_WRITE,
                                  libc::MAP_PRIVATE | libc::MAP_FIXED_NOREPLACE |
                                        libc::MAP_ANONYMOUS | libc::MAP_32BIT , -1, 0) };
    if ptr == ((-1 as i32) as *mut libc::c_void) {
       unsafe {
           panic!("mmap failed, unable to allocate {} bytes errno {}", n, std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
       };
    }
    
    ptr as vm_types::Ptr
}