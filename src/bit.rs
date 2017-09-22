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

    pub fn read_bit(&mut self) -> Result<bool> {
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

    pub fn read_part(&mut self, bits: u8) -> Result<u16> {
        assert!(bits <= 16);

        let mut res = 0u16;
        for i in 0..bits {
            if self.read_bit()? {
                res |= 1 << i;
            }
        }

        Ok(res)
    }

    pub fn read_length_prefixed(&mut self) -> Result<Vec<u8>> {
        assert_eq!(0, self.position());

        let len = self.read_aligned_u16()?;
        let ones_complement = self.read_aligned_u16()?;

        ensure!(
            (len ^ 0xFFFF) == ones_complement,
            "uncompressed block length validation failed"
        );

        let mut buf = vec![0u8; len as usize];
        self.inner.read_exact(&mut buf)?;

        Ok(buf)
    }

    #[inline]
    fn read_aligned_u16(&mut self) -> Result<u16> {
        assert_eq!(0, self.position());

        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;

        Ok((u16::from(buf[1]) << 8) | u16::from(buf[0]))
    }
}
