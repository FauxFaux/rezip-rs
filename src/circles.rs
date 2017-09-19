use errors::*;

pub struct CircularBuffer {
    data: Vec<u8>,
    idx: usize,
}

impl CircularBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        CircularBuffer {
            idx: 0,
            data: vec![0; cap],
        }
    }

    pub fn append(&mut self, val: u8) {
        self.data[self.idx] = val;
        self.idx = (self.idx + 1) % self.data.len();
    }

    pub fn copy(&mut self, dist: u32, len: u32, into: ()) -> Result<()> {
        unimplemented!()
    }
}
