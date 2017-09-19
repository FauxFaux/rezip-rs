use std::io;
use std::io::Read;

use errors::*;

pub struct BitReader<R> {
    inner: R,
    // negative one for eof, wtf
    current: i32,
    remaining_bits: u8,
}

impl<R: Read> BitReader<R> {
    pub fn new(inner: R) -> Self {
        BitReader {
            inner,
            current: 0,
            remaining_bits: 0,
        }
    }

    pub fn position(&self) -> u8 {
        assert!(self.remaining_bits <= 7);
        (8 - self.remaining_bits) % 8
    }

    pub fn read_or_eof(&mut self) -> Result<Option<bool>> {
        if -1 == self.current {
            return Ok(None);
        }

        if 0 == self.remaining_bits {
            let mut buf = [0u8; 1];
            match self.inner.read(&mut buf)? {
                0 => return Ok(None),
                1 => self.current = buf[0] as i32,
                _ => unreachable!(),
            }

            self.remaining_bits = 8;
        }

        self.remaining_bits -= 1;

        let bit = (self.current >> (7 - self.remaining_bits)) & 1;
        Ok(Some(1 == bit))
    }

    pub fn read_always(&mut self) -> Result<bool> {
        match self.read_or_eof() {
            Ok(Some(bit)) => Ok(bit),
            Ok(None) => Err(
                io::Error::new(io::ErrorKind::UnexpectedEof, "read_always").into(),
            ),
            Err(e) => Err(e),
        }
    }

    pub fn read_part_u8(&mut self, bits: u8) -> Result<u8> {
        assert!(bits <= 8);
        let mut res = 0u8;
        for i in 0..bits {
            if self.read_always()? {
                res |= 1 << i;
            }
        }

        Ok(res)
    }
}
