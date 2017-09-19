use std::io::Write;

use errors::*;

pub struct CircularBuffer {
    data: Vec<u8>,
    idx: usize,
}

impl CircularBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        assert!(cap > 0);

        CircularBuffer {
            idx: 0,
            data: vec![0; cap],
        }
    }

    pub fn append(&mut self, val: u8) {
        self.data[self.idx] = val;
        self.idx = (self.idx + 1) % self.data.len();
    }

    pub fn copy<W: Write>(&mut self, dist: u32, len: u32, mut into: W) -> Result<()> {
        ensure!(dist > 0 && dist as usize <= self.data.len(), "dist must fit");

        let mut read_from = (self.idx - dist as usize + self.data.len()) % self.data.len();

        for i in 0..len {
            let b = self.data[read_from];
            read_from = (read_from + 1) % self.data.len();
            into.write_all(&[b])?;
            self.append(b);
        }

        Ok(())
    }
}
