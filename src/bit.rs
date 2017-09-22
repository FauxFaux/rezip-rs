use std::io::Read;

use errors::*;

pub struct BitReader<R> {
    inner: R,
    current: u8,
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

    pub fn read_always(&mut self) -> Result<bool> {
        if 0 == self.remaining_bits {
            let mut buf = [0u8; 1];
            self.inner.read_exact(&mut buf)?;
            self.current = buf[0];
            self.remaining_bits = 8;
        }

        self.remaining_bits -= 1;

        let bit = (self.current >> (7 - self.remaining_bits)) & 1;
        Ok(1 == bit)
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

    pub fn read_part_u16(&mut self, bits: u8) -> Result<u16> {
        assert!(bits <= 16);

        let mut res = 0u16;
        for i in 0..bits {
            if self.read_always()? {
                res |= 1 << i;
            }
        }

        Ok(res)
    }

    pub fn read_aligned_u16(&mut self) -> Result<u16> {
        assert_eq!(0, self.position());

        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;

        Ok((u16::from(buf[1]) << 8) | u16::from(buf[0]))
    }

    pub fn read_aligned_u8(&mut self) -> Result<u8> {
        assert_eq!(0, self.position());

        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;

        Ok(buf[0])
    }
}
