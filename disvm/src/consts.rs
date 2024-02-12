#![allow(dead_code)]

// magic signatures for dis modules.

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Primitive)]
pub enum DisMagicNo {
    XMAGIC= 819248,
    SMAGIC=923426
}




pub const XMAGIC: u32 = 819248; // unencrpted
pub const SMAGIC: u32 = 923426; // encrypted

#[allow(non_camel_case_types)]
#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Primitive)]
pub enum DisRuntimeFlag {
    MUSTCOMPILE =  1,  // 1<<0,  module must be compiled JIT to be run
    DONTCOMPILE =  2,  // 1<<1,
    SHAREMP     =  4,  // 1<<2 each instance of module shared the same module data
    flag3       =  8,  // 1<<3 TODO figure out
    HASLDT1     = 16,  // 1<<4 contains import table. apparently not used.
    HASEXCEPT   = 32,  // 1<<5 contains exception handler
    HASLDT2     = 64,  // 1<<6 contains import table.
    flag7       = 128, // 1<<7 TODO
}