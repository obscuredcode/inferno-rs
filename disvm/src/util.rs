use module::{Operand, parse_operand};

pub struct BufferedReader<'a> {
    pub(crate) buffer: &'a Vec<u8>,
    pub(crate) offset: usize,
    pub error: String,
}

impl<'a> BufferedReader<'a> {

    pub fn peek(&self, n: usize) -> &'a [u8] {
        self.buffer.get(self.offset..self.offset + n).unwrap_or_else(move || {
            panic!("failed to read {n} bytes at offset {self.offset} for {self.error}")
        })
    }
    pub fn read(&mut self, n: usize) -> &[u8] {
        let slice = self.peek(n);
        self.offset += n;
        slice
    }

    pub fn reade(&mut self, n: usize, err: &str) -> &[u8] {
        self.error = err.to_string();
        let slice = self.peek(n);
        self.offset += n;
        slice
    }


    pub fn u8(&mut self) -> u8 {
        self.read(1)[0]
    }

    pub fn u8e(&mut self, err: &str) -> u8 {
        self.error = err.to_string();
        self.u8()
    }

    pub fn i8(&mut self) -> i8 {
        self.read(1)[0] as i8
    }

    pub fn i8e(&mut self, err: &str) -> i8 {
        self.error = err.to_string();
        self.i8()
    }

    pub fn u32(&mut self) -> u32 {
        let mut buf: [u8; 4] = [0; 4];
        buf.copy_from_slice(self.read(4));
        u32::from_be_bytes(buf)
    }

    pub fn i32(&mut self) -> i32 {
        let mut buf: [u8; 4] = [0; 4];
        buf.copy_from_slice(self.read(4));
        i32::from_be_bytes(buf)
    }

    pub fn i32e(&mut self, err: &str) -> i32 {
        self.error = err.to_string();
        self.i32()
    }

    pub fn i64(&mut self) -> i64 {
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(self.read(8));
        i64::from_be_bytes(buf)
    }

    pub fn i64e(&mut self, err: &str) -> i64 {
        let mut buf: [u8; 8] = [0; 8];
        self.error = err.to_string();
        buf.copy_from_slice(self.read(8));
        i64::from_be_bytes(buf)
    }

    pub fn f64(&mut self) -> f64 {
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(self.read(8));
        f64::from_be_bytes(buf)
    }


    pub fn f64e(&mut self, err: &str) -> f64 {
        let mut buf: [u8; 8] = [0; 8];
        self.error = err.to_string();
        buf.copy_from_slice(self.read(8));
        f64::from_be_bytes(buf)
    }

    pub fn operand(&mut self) -> Operand {
        let o = parse_operand(self.peek(4));
        self.offset += o.size();
        o
    }

    pub fn skip(&mut self, n: usize) {
        self.offset += n;
    }

}