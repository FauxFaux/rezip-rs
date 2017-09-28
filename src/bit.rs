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
        assert!(self.track.is_none());

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

    pub fn tracking_start(&mut self) {
        assert!(self.track.is_none());
        self.track = Some(BitVec::new());
    }

    pub fn tracking_finish(&mut self) -> BitVec {
        // self.track.take() // :(
        std::mem::replace(&mut self.track, None).expect("tracking wasn't started")
    }

    fn read_aligned_u16(&mut self) -> Result<u16> {
        assert_eq!(0, self.position());
        assert!(self.track.is_none());

        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;

        Ok((u16::from(buf[1]) << 8) | u16::from(buf[0]))
    }

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
        while 0 != self.current.len() {
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

    pub fn into_inner(self) -> W {
        assert_eq!(0, self.current.len());

        self.inner
    }
}

pub fn vec_to_bytes(vec: &BitVec) -> Vec<u8> {
    let mut vec = vec.clone();
    while vec.len() % 8 != 0 {
        vec.push(false);
    }

    let mut it = vec.iter();

    let mut ret = vec![];

    let mut done = false;
    while !done {
        let mut val = 0u8;
        for i in 0..8 {
            match it.next() {
                Some(bit) => {
                    if bit {
                        val |= (1 << i);
                    }
                }
                None => {
                    done = true;
                    break;
                }
            }
        }
        ret.push(val);
    }

    ret
}

const WORD_SIZE: usize = 8;

#[derive(Clone)]
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

    pub fn push(&mut self, val: bool) {
        let word = self.len / WORD_SIZE;
        let bit = self.len % WORD_SIZE;

        self.len += 1;

        if word >= self.bytes.len() {
            assert_eq!(word, self.bytes.len());
            self.bytes.push(0);
        }

        if val {
            let word = self.bytes.get_mut(word).unwrap();
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
    fn tracking() {
        let mut reader = BitReader::new(Cursor::new(vec![0b0001_1001]));
        reader.tracking_start();
        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());

        let tracked = reader.tracking_finish();

        assert_eq!(
            &[true, false, false, true, true, false],
            &tracked.iter().collect::<Vec<bool>>().as_slice()
        );

        let mut writer = BitWriter::new(Cursor::new(vec![]));
        writer.write_vec(&tracked).unwrap();
        writer.align().unwrap();
        assert_eq!(vec![0b0001_1001u8; 1], writer.into_inner().into_inner());
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
