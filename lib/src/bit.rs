use std;
use std::fmt;
use std::io::Read;
use std::io::Write;
use std::ops::BitOrAssign;

use errors::*;

pub struct BitReader<R> {
    inner: R,
    current: u8,
    remaining_bits: u8,
    track: Option<BitVec>,
}

pub struct BitWriter<W> {
    inner: W,
    current: BitVec,
}

impl<R: Read> BitReader<R> {
    pub fn new(inner: R) -> Self {
        BitReader {
            inner,
            current: 0,
            remaining_bits: 0,
            track: None,
        }
    }

    fn position(&self) -> u8 {
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
        let bit = 1 == bit;

        if let Some(vec) = self.track.as_mut() {
            vec.push(bit);
        }

        Ok(bit)
    }

    pub fn align(&mut self) -> Result<()> {
        assert!(self.track.is_none());

        while 0 != self.position() {
            ensure!(!self.read_bit()?, "padding bits should always be empty");
        }
        Ok(())
    }

    pub fn read_part(&mut self, bits: u8) -> Result<u16> {
        BitSource::read_part(self, bits)
    }

    pub fn read_length_prefixed(&mut self) -> Result<Vec<u8>> {
        assert_eq!(0, self.position());
        assert!(self.track.is_none());

        let len = self.read_aligned_u16()?;
        let ones_complement = self.read_aligned_u16()?;

        ensure!(
            (len ^ 0xFFFF) == ones_complement,
            "uncompressed block length validation failed"
        );

        let mut buf = vec![0u8; len as usize];
        self.inner
            .read_exact(&mut buf)
            .chain_err(|| format!("reading a length-prefixed {} bytes", len))?;

        Ok(buf)
    }

    fn read_aligned_u16(&mut self) -> Result<u16> {
        assert_eq!(0, self.position());
        assert!(self.track.is_none());

        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;

        Ok((u16::from(buf[1]) << 8) | u16::from(buf[0]))
    }

    #[allow(unused)]
    pub fn into_inner(self) -> R {
        assert!(self.track.is_none());
        assert_eq!(0, self.position());

        self.inner
    }
}

impl<W: Write> BitWriter<W> {
    pub fn new(inner: W) -> Self {
        BitWriter {
            inner,
            current: BitVec::new(),
        }
    }

    pub fn write_bit(&mut self, bit: bool) -> Result<()> {
        self.current.push(bit);
        if self.current.len() >= 8 {
            assert_eq!(8, self.current.len());

            self.inner.write_all(&[self.current.pop_byte().unwrap()])?;
        }
        Ok(())
    }

    pub fn write_bits_val(&mut self, bits: u8, val: u16) -> Result<()> {
        for i in 0..bits {
            self.write_bit((val & (1 << i)) != 0)?;
        }
        Ok(())
    }

    pub fn align(&mut self) -> Result<()> {
        while !self.current.is_empty() {
            self.write_bit(false)?;
        }
        Ok(())
    }

    pub fn write_vec(&mut self, vec: &BitVec) -> Result<()> {
        for bit in vec.iter() {
            self.write_bit(bit)?;
        }
        Ok(())
    }

    pub fn write_length_prefixed(&mut self, data: &[u8]) -> Result<()> {
        self.align()?;
        ensure!(
            data.len() <= std::u16::MAX as usize,
            "data too long to store"
        );

        self.write_aligned_u16(data.len() as u16)?;
        self.write_aligned_u16((data.len() ^ 0xFFFF) as u16)?;
        self.inner.write_all(data)?;
        Ok(())
    }

    pub fn write_aligned_u16(&mut self, val: u16) -> Result<()> {
        self.inner.write_all(&[(val >> 8) as u8, val as u8])?;
        Ok(())
    }

    pub fn into_inner(self) -> W {
        assert_eq!(0, self.current.len());

        self.inner
    }
}

const WORD_SIZE: usize = 8;

#[derive(Clone, Default, Eq, PartialEq)]
pub struct BitVec {
    bytes: Vec<u8>,
    len: usize,
}

impl BitVec {
    pub fn new() -> Self {
        BitVec {
            bytes: Vec::new(),
            len: 0,
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Self {
        BitVec {
            bytes: bytes.to_vec(),
            len: bytes.len() * WORD_SIZE,
        }
    }

    pub fn push(&mut self, val: bool) {
        let word = self.len / WORD_SIZE;
        let bit = self.len % WORD_SIZE;

        self.len += 1;

        if word >= self.bytes.len() {
            assert_eq!(word, self.bytes.len());
            self.bytes.push(0);
        }

        if val {
            let word = &mut self.bytes[word];
            word.bitor_assign(1 << (bit));
        }
    }

    pub fn get(&self, pos: usize) -> bool {
        assert!(pos < self.len, "out of range");

        let word = pos / WORD_SIZE;
        let bit = pos % WORD_SIZE;

        self.bytes[word] & (1 << bit) == (1 << bit)
    }

    pub fn pop(&mut self) -> Option<bool> {
        if 0 == self.len {
            return None;
        }

        let answer = self.get(self.len - 1);

        self.len -= 1;

        if self.len % WORD_SIZE == 0 {
            self.bytes.pop();
        }

        Some(answer)
    }

    #[allow(unused)]
    fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        assert_eq!(self.len == 0, self.bytes.is_empty());
        self.len == 0
    }

    pub fn pop_byte(&mut self) -> Option<u8> {
        if self.len < 8 {
            return None;
        }

        let mut ret = 0u8;

        for pos in 0..8 {
            if self.pop().unwrap() {
                ret |= 1 << (7 - pos);
            }
        }

        Some(ret)
    }

    pub fn iter(&self) -> StackIterator {
        StackIterator {
            inner: self,
            pos: 0,
        }
    }

    pub fn pretty_print(&self) {
        let mut it = self.iter();
        loop {
            let mut line = String::with_capacity(8);
            loop {
                match it.next() {
                    Some(bit) => line.push(if bit { '1' } else { '0' }),
                    None => break,
                }

                if line.len() == 8 {
                    break;
                }
            }

            if line.is_empty() {
                break;
            }

            println!("{}", line);
        }
    }
}

pub struct StackIterator<'a> {
    inner: &'a BitVec,
    pos: usize,
}

impl<'a> Iterator for StackIterator<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.inner.len() {
            assert_eq!(self.pos, self.inner.len());
            return None;
        }

        let ret = self.inner.get(self.pos);

        self.pos += 1;

        Some(ret)
    }
}

impl fmt::Debug for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BitVec: {}: ", self.len())?;
        for bit in self.iter() {
            write!(f, "{}", if bit { "1" } else { "0" })?;
        }
        Ok(())
    }
}

pub trait BitSource {
    fn read_bit(&mut self) -> Result<bool>;

    fn read_part(&mut self, bits: u8) -> Result<u16> {
        assert!(bits <= 16);

        let mut res = 0u16;
        for i in 0..bits {
            if self.read_bit()? {
                res |= 1 << i;
            }
        }

        Ok(res)
    }
}

impl<R: Read> BitSource for BitReader<R> {
    fn read_bit(&mut self) -> Result<bool> {
        self.read_bit()
    }
}

impl<'a> BitSource for StackIterator<'a> {
    fn read_bit(&mut self) -> Result<bool> {
        Ok(self.next().expect("TODO: EOF"))
    }
}

pub struct BitCollector<'a, B: BitSource + 'a> {
    inner: &'a mut B,
    data: BitVec,
}

impl<'a, B: BitSource> BitCollector<'a, B> {
    pub fn new(inner: &'a mut B) -> Self {
        BitCollector {
            inner,
            data: BitVec::new(),
        }
    }

    pub fn into_data(self) -> BitVec {
        self.data
    }
}

impl<'a, B: BitSource> BitSource for BitCollector<'a, B> {
    fn read_bit(&mut self) -> Result<bool> {
        let bit = self.inner.read_bit()?;
        self.data.push(bit);
        Ok(bit)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_read() {
        let mut writer = BitWriter::new(Cursor::new(vec![]));
        writer.write_bit(true).unwrap();
        writer.write_bit(false).unwrap();
        writer.write_bit(false).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(false).unwrap();
        writer.write_bit(false).unwrap();

        let mut cursor = writer.into_inner();
        cursor.set_position(0);
        println!("{:0b}", cursor.get_ref()[0]);

        let mut reader = BitReader::new(cursor);
        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
    }

    #[test]
    fn vec_push() {
        let mut v = BitVec::new();
        for i in 0..100 {
            v.push(i % 2 == 0);
        }
    }

    #[test]
    fn vec_push_pop() {
        let mut v = BitVec::new();
        v.push(true);
        v.push(false);
        assert_eq!(2, v.len());
        assert!(!v.pop().unwrap());
        assert!(v.pop().unwrap());
        assert_eq!(0, v.len());

        v = eight_bits();
        assert!(v.pop().unwrap());
        assert!(v.pop().unwrap());
        assert!(v.pop().unwrap());
        assert!(!v.pop().unwrap());
        assert!(v.pop().unwrap());
        assert!(!v.pop().unwrap());
        assert!(!v.pop().unwrap());
        assert!(v.pop().unwrap());
        assert!(v.pop().is_none());
    }

    #[test]
    fn vec_pop_byte() {
        let mut v = eight_bits();

        let by = v.pop_byte().unwrap();
        assert_eq!(0b1110_1001, by);
        assert_eq!(0, v.len());
    }

    #[test]
    fn vec_iter() {
        let arr: Vec<bool> = eight_bits().iter().collect();
        assert_eq!(vec![true, false, false, true, false, true, true, true], arr);
    }

    fn eight_bits() -> BitVec {
        let mut v = BitVec::new();
        v.push(true);
        v.push(false);
        v.push(false);
        v.push(true);
        v.push(false);
        v.push(true);
        v.push(true);
        v.push(true);

        v
    }
}
