use std::io::Write;
use u16_from;

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

    // This updates self, whereas run_from and friends do not.
    pub fn copy<W: Write>(&mut self, dist: u16, len: u16, mut into: W) -> Result<()> {
        // TODO: optimise

        ensure!(dist > 0 && dist <= self.valid_cap, "dist must fit");

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

    pub fn get_at_dist(&self, dist: u16) -> u8 {
        assert!(dist <= self.valid_cap);
        self.data[self.idx
                      .wrapping_sub(dist as usize)
                      .wrapping_add(self.data.len()) % self.data.len()]
    }

    pub fn capacity(&self) -> u16 {
        self.data.len() as u16
    }

    pub fn possible_run_length_at(&self, dist: u16, upcoming_data: &[u8]) -> u16 {
        u16_from(self.run_from(dist).match_length(upcoming_data))
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

    pub fn vec(&self) -> Vec<u8> {
        // TODO: optimise

        let mut ret = Vec::with_capacity(usize_from(self.valid_cap));
        for pos in (1..1 + self.valid_cap).rev() {
            ret.push(self.get_at_dist(pos));
        }

        ret
    }

    pub fn run_from(&self, dist: u16) -> Runerator {
        assert!(dist > 0);
        Runerator {
            inner: self,
            stride: dist,
            pos: dist,
        }
    }
}

pub struct Runerator<'a> {
    inner: &'a CircularBuffer,
    stride: u16,
    pos: u16,
}

impl<'a> Iterator for Runerator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let found = Some(self.inner.get_at_dist(self.pos));

        self.pos -= 1;
        if 0 == self.pos {
            self.pos = self.stride;
        }

        found
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::max_value(), None)
    }
}

impl<'a> Runerator<'a> {
    pub fn match_length(&mut self, other: &[u8]) -> usize {
        self.take(258).zip(other).filter(|&(x, y)| x == *y).count()
    }
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

        buf.extend(b"1234567890abcd");

        assert_eq!(3, buf.find_run(b"bc").unwrap());

        assert_eq!(6, buf.find_run(b"90ab").unwrap());
    }

    #[test]
    fn runerator() {
        let mut buf = CircularBuffer::with_capacity(10);
        buf.extend(b"1234567890abcd");
        assert_eq!(
            b"bcdbcd",
            &buf.run_from(3).take(6).collect::<Vec<u8>>().as_slice()
        );
        assert_eq!(
            b"ddddd",
            &buf.run_from(1).take(5).collect::<Vec<u8>>().as_slice()
        );

        // TODO: test 258 boundary

        assert_eq!(4, buf.run_from(1).match_length(b"ddddef"));
    }
}
