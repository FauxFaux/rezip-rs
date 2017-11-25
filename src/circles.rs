use std::io::Write;

use errors::*;
use usize_from;

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
            data: vec![0; usize_from(cap)],
            valid_cap: 0,
        }
    }

    pub fn push(&mut self, val: u8) {
        self.data[self.idx] = val;
        self.idx = (self.idx + 1) % self.data.len();

        if (self.valid_cap as usize) < self.data.len() {
            self.valid_cap += 1;
        }
    }

    pub fn extend(&mut self, val: &[u8]) {
        // TODO: optimise

        for byte in val {
            self.push(*byte);
        }
    }

    pub fn extendi<'a, I: Iterator<Item = &'a u8>>(&mut self, val: I) {
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

        let mut read_from = (self.idx
            .wrapping_sub(dist as usize)
            .wrapping_add(self.data.len())) % self.data.len();

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
        debug_assert!(self.valid_cap as usize <= self.data.len());
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

    pub fn possible_run_length_at(&self, dist: u16, upcoming_data: &[u8]) -> u16 {
        assert!(dist > 0, "dist must be positive");

        let upcoming_data = &upcoming_data[..258.min(upcoming_data.len())];
        for pos in 3..dist.min(upcoming_data.len() as u16) {
            if upcoming_data[pos as usize] != self.get_at_dist(dist - pos) {
                return pos;
            }
        }

        for pos in dist..(upcoming_data.len() as u16) {
            if upcoming_data[(pos % dist) as usize] != upcoming_data[pos as usize] {
                return pos;
            }
        }

        return upcoming_data.len() as u16;
    }

    pub fn vec(&self) -> Vec<u8> {
        // TODO: optimise

        let mut ret = Vec::with_capacity(usize_from(self.valid_cap));
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
    fn run_length_at() {
        let mut buf = CircularBuffer::with_capacity(100);
        buf.extend(b"abcdef b");
        //   distances: "87654321"
        assert_eq!(b'b', buf.get_at_dist(7));
        assert_eq!(5, buf.possible_run_length_at(7, b"bcdef"));
    }

    #[test]
    fn run_length_at_2() {
        let mut buf = CircularBuffer::with_capacity(100);
        buf.extend(b"a122b");
        assert_eq!(b'1', buf.get_at_dist(4));
        assert_eq!(3, buf.possible_run_length_at(4, b"122222"));
    }
}
