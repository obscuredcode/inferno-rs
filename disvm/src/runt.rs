use std::env::Args;



mod vm_types {
    type Ptr = u32;

    #[repr(C)]
    pub struct Runtab {
        name: Ptr, // char*
        sig: u64,
        func: Box<dyn FnOnce()->()> // void (*fn)(void*)
    }
}

type FramePointer = u32;
pub struct NativeCall {
    fp: FramePointer,
}

impl FnOnce<()> for NativeCall {
    type Output = ();


    extern "rust-call" fn call_once(self, args: ()) -> Self::Output {
        todo!()
    }
}

fn builtinmod() {
    // create new empty module
    // - newmod()
    // load functions from Runtab into modules link section
    {
        // declare type descriptor for function stack frame
        // - dtype()
        // load function name, sig, type desc into link struct in module
        // - runtime()
    }
}