use mem::{Header, VmObject, VmPtr};


// has to appear as 32-bit memory location
// this is a memory location visible to the VM
// all instruction parameters are effectively addresses except for integral immediates


type Ptr32 = u32;


// string
// will have data pointer which has to be a 32-bit address.
// this doesn't have to be in the heap

// array
// will have data pointer ^

// external pointers 64-bit
// need to be able to distinguish these from other kinds of pointers.
// they must appear as 32-bit addresses

// heap has to be a contiguous memory area addressable by a 32-bit pointer and by pointer+offset
// mp is going to point to the module data area in the heap
// the rest of the heap can be used for allocations
// heap will mostly store pointers but we can encapsulate them


// VmPtr will always point to this

#[repr(C, align(8))]
struct Object {
    // header ?
    content: ObjectContent
}


enum ObjectContent {
    Ptr32(i32),
    Ptr64(u64),
    Value(i32)
}




pub trait VmHeap {
    type VmPtr;
    fn allocate<T>(&mut self) -> VmPtr;
    fn read<T>(&self, src: VmPtr) -> T;
    fn write<T>(&mut self, dest: VmPtr, src: T);
    unsafe fn adopt_ptr64<T>(&mut self, ptr: *mut T) -> VmPtr;

}

struct Heap {
    heap: Ptr32,
    heap_start: u32,
    capacity: usize,
}

impl Heap {
    fn check_reallocate(&mut self) {

    }

    // n should be already be aligned
    fn allocate_bytes_raw(&mut self, n: usize) -> VmPtr {
        unsafe {
            let raw = self.heap + (self.heap_start);
            let ptr = VmPtr::new(raw as u32);
            self.heap_start += n as u32;
            ptr
        }
    }

    fn create_obj(&mut self) -> VmPtr {
        let size = std::mem::size_of::<Object>();
        let raw = self.heap + (self.heap_start);
        self.heap_start += size as u32;
        VmPtr::new(raw as u32)
    }



    fn allocate<T>(&mut self) -> VmPtr {

        let size = std::mem::size_of::<VmObject>();
        let mut ptr = self.allocate_bytes_raw(size);

        ptr.add(std::mem::size_of::<Header>()); // TODO ensure there is no padding
        ptr
    }





}

