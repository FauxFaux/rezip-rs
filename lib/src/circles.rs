use std::io::Write;

use cast::usize;

use errors::*;

#[derive(Default)]
pub struct CircularBuffer {
    data: Vec<u8>,
    idx: usize,
    valid_cap: u16,
}

impl CircularBuffer {
    pub fn new() -> Self {
        Self::with_capacity(32 * 1024)
    }

    pub fn with_capacity(cap: u16) -> Self {
        assert!(cap > 0);

        CircularBuffer {
            idx: 0,
            data: vec![0; usize(cap)],
            valid_cap: 0,
        }
    }

    pub fn push(&mut self, val: u8) {
        self.data[self.idx] = val;
        self.idx = (self.idx + 1) % self.data.len();

        if (usize(self.valid_cap)) < self.data.len() {
            self.valid_cap += 1;
        }
    }

    pub fn extend(&mut self, val: &[u8]) {
        // TODO: optimise

        for byte in val {
            self.push(*byte);
        }
    }

    // This updates self, whereas run_from and friends do not.
    pub fn copy<W: Write>(&mut self, dist: u16, len: u16, mut into: W) -> Result<()> {
        // TODO: optimise

        ensure!(
            dist > 0 && dist <= self.valid_cap,
            "dist must fit: {} / {}",
            dist,
            self.valid_cap
        );

        let mut read_from = (self
            .idx
            .wrapping_sub(usize(dist))
            .wrapping_add(self.data.len()))
            % self.data.len();

        for _ in 0..len {
            let b = self.data[read_from];
            read_from = (read_from + 1) % self.data.len();
            into.write_all(&[b])?;
            self.push(b);
        }

        Ok(())
    }

    #[inline]
    pub fn get_at_dist(&self, dist: u16) -> u8 {
        debug_assert!(
            dist > 0,
            "distances are one-indexed; the most recent inserted value is 1"
        );
        debug_assert!(usize(self.valid_cap) <= self.data.len());
        debug_assert!(dist <= self.valid_cap);

        let target = self.idx as isize - (dist as isize);
        let idx = if target >= 0 {
            target
        } else {
            target + self.data.len() as isize
        } as usize;

        self.data[idx]
    }

    pub fn capacity(&self) -> u16 {
        self.data.len() as u16
    }

    pub fn len(&self) -> u16 {
        self.valid_cap
    }

    pub fn vec(&self) -> Vec<u8> {
        // TODO: optimise

        let mut ret = Vec::with_capacity(usize(self.valid_cap));
        for pos in (1..1 + self.valid_cap).rev() {
            ret.push(self.get_at_dist(pos));
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist() {
        let mut buf = CircularBuffer::with_capacity(10);
        buf.extend(b"abcdef");
        assert_eq!(b'f', buf.get_at_dist(1));
        assert_eq!(b'a', buf.get_at_dist(6));

        buf.extend(b"qrstuv");
        assert_eq!(b'v', buf.get_at_dist(1));
        assert_eq!(b'q', buf.get_at_dist(6));
        assert_eq!(b'f', buf.get_at_dist(7));
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_invalid_dist() {
        let mut buf = CircularBuffer::with_capacity(10);
        buf.extend(b"abcdef");
        buf.get_at_dist(7);
    }

    #[test]
    fn veccy() {
        let mut buf = CircularBuffer::with_capacity(6);
        buf.extend(b"abcdefghij");
        assert_eq!(b"efghij", buf.vec().as_slice());
    }
}
