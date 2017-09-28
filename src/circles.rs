use std::io::Write;

use errors::*;

pub struct CircularBuffer {
    data: Vec<u8>,
    idx: usize,
}

impl CircularBuffer {
    pub fn with_capacity(cap: u16) -> Self {
        assert!(cap > 0);

        CircularBuffer {
            idx: 0,
            data: vec![0; usize_from(cap)],
        }
    }

    pub fn append(&mut self, val: u8) {
        self.data[self.idx] = val;
        self.idx = (self.idx + 1) % self.data.len();
    }

    pub fn copy<W: Write>(&mut self, dist: u16, len: u16, mut into: W) -> Result<()> {
        ensure!(
            dist > 0 && dist as usize <= self.data.len(),
            "dist must fit"
        );

        let mut read_from = (self.idx.wrapping_sub(dist as usize).wrapping_add(
            self.data.len(),
        )) % self.data.len();

        for _ in 0..len {
            let b = self.data[read_from];
            read_from = (read_from + 1) % self.data.len();
            into.write_all(&[b])?;
            self.append(b);
        }

        Ok(())
    }

    pub fn find_run(&self, run: &[u8]) -> Result<usize> {
        let cap = self.data.len();
        ensure!(run.len() < cap, "can't have a run that long");

        for dist in (run.len() - 1)..cap {
            let start = self.idx.wrapping_sub(dist).wrapping_add(cap) % cap;
            if self.run_at(start, run) {
                return Ok(dist);
            }
        }
        unimplemented!()
    }

    fn run_at(&self, start: usize, run: &[u8]) -> bool {
        for i in 0..run.len() {
            let j = start.wrapping_add(i) % self.data.len();
            if self.data[j] != run[i] {
                return false;
            }
        }
        true
    }
}

fn usize_from(val: u16) -> usize {
    val as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_across_end() {
        let mut buf = CircularBuffer::with_capacity(10);

        // 1234 will be dropped,
        // so buf logically contains 4567890abcd,
        // represented as abcd4567890
        // with the marker at ^ (position 4)

        for byte in b"1234567890abcd" {
            buf.append(*byte);
        }

        assert_eq!(3, buf.find_run(b"bc").unwrap());

        assert_eq!(6, buf.find_run(b"90ab").unwrap());
    }
}
