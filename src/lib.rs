#![feature(option_filter)]

extern crate byteorder;

#[macro_use]
extern crate error_chain;

extern crate itertools;

#[macro_use]
extern crate lazy_static;

extern crate result;
extern crate sha2;

use std::fmt;

pub mod bestguess;
mod bit;
pub mod circles;
mod code_tree;
pub mod emulate;
mod errors;
pub mod filter;
pub mod guesser;
pub mod gzip;
mod huffman;
pub mod infer;
pub mod parse;
pub mod serialise;
pub mod serialise_trace;
pub mod trace;

use bit::BitVec;

pub use errors::*;
pub use parse::parse_deflate;
pub use serialise::compressed_block;
pub use serialise::decompressed_block;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Ref {
    pub dist: u16,
    run_minus_3: u8,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Code {
    Literal(u8),
    Reference(Ref),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Uncompressed(Vec<u8>),
    FixedHuffman(Vec<Code>),
    DynamicHuffman { trees: BitVec, codes: Vec<Code> },
}

#[derive(Debug, Eq, PartialEq)]
pub struct WindowSettings {
    window_size: u16,

    /// gzip (including 1.6 and probably onwards) will mis-encode
    /// "aaaaaa" as "aa{ref one back, run=..}", as the encoder can't
    /// be bothered to cope with pointers to the 0th character.
    ///
    /// Test-case:
    ///
    /// ```text
    /// % yes aaaaaaaaaa | tr -d '\n' | head -c 8453631 | gzip > a.gz
    /// % cargo run --example dump a.gz | uniq -c
    ///     1 block 0:
    ///     1  - dynamic huffman: BitVec: 110: 101111...
    ///     2     - lit: 0x61: 'a'
    /// 32765     - backref: 1 byte(s) back, 258 bytes long
    ///     1 block 1:
    ///     1  - fixed huffman:
    ///     1     - backref: 1 byte(s) back, 258 bytes long
    ///     1     - lit: 0x61: 'a'
    /// ```
    ///
    /// Note the double 'a' at the start.
    first_byte_bug: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Trace {
    Correct,
    Actual(Code),
}

impl Code {
    pub fn emitted_bytes(&self) -> u16 {
        match *self {
            Code::Literal(_) => 1,
            Code::Reference(r) => r.run(),
        }
    }
}

impl Ref {
    #[inline]
    fn new(dist: u16, run: u16) -> Self {
        assert!(run >= 3);
        assert!(run <= 258);

        assert!(dist >= 1);
        assert!(dist <= 32768);

        let run_minus_3 = (run - 3) as u8;
        Ref { dist, run_minus_3 }
    }

    #[inline]
    pub fn run(&self) -> u16 {
        u16::from(self.run_minus_3) + 3
    }
}

impl From<Ref> for Code {
    fn from(r: Ref) -> Self {
        Code::Reference(r)
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Code::Literal(byte) => write!(f, "L(0x{:02x} {:?})", byte, byte as char),
            Code::Reference(r) => write!(f, "R(-{}, {})", r.dist, r.run()),
        }
    }
}

impl fmt::Debug for Trace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Trace::Correct => write!(f, "✓"),
            Trace::Actual(code) => write!(f, "c{:?}", code),
        }
    }
}

impl fmt::Debug for Ref {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "R[{}, {}]", self.dist, self.run())
    }
}

#[inline]
fn u16_from(val: usize) -> u16 {
    assert!(val <= (std::u16::MAX as usize));
    val as u16
}

#[inline]
fn usize_from(val: u16) -> usize {
    val as usize
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::io::Read;
    use std::io::Write;
    use bit::BitWriter;
    use circles::CircularBuffer;
    use ::*;

    #[test]
    fn seq_20_round_trip() {
        // no distance references at all, dynamic huffman
        round_trip(&include_bytes!("../tests/data/seq-20.gz")[..], 51);
    }

    #[test]
    fn lol_round_trip() {
        // fixed huffman, no backreferences
        round_trip(&include_bytes!("../tests/data/lol.gz")[..], 3);
    }

    #[test]
    fn like_love_round_trip() {
        // single true backreference in the middle, fixed huffman
        round_trip(&include_bytes!("../tests/data/like-love.gz")[..], 29);
    }

    #[test]
    fn simple_backreference_round_trip() {
        round_trip(&include_bytes!("../tests/data/abcdef-bcdefg.gz")[..], 13);
    }

    #[test]
    fn libcgi_round_trip() {
        round_trip(
            &include_bytes!("../tests/data/libcgi-untaint-email-perl_0.03.orig.tar.gz")[..],
            20480,
        );
    }

    #[test]
    fn librole_round_trip() {
        round_trip(
            &include_bytes!("../tests/data/librole-basic-perl_0.13-1.debian.tar.gz")[..],
            20480,
        );
    }

    fn round_trip(orig: &[u8], expected_len: usize) {
        let mut raw = Cursor::new(orig);
        let header = gzip::discard_header(&mut raw).unwrap();

        let mut decompressed = Vec::new();
        let mut recompressed = Cursor::new(Vec::new());
        recompressed.write_all(&header).unwrap();
        let mut recompressed = BitWriter::new(recompressed);

        {
            let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
            let mut it = parse::parse_deflate(&mut raw).peekable();

            loop {
                let block = match it.next() {
                    Some(block) => block.unwrap(),
                    None => break,
                };

                let last = it.peek().is_none();

                decompressed_block(&mut decompressed, &mut dictionary, &block).unwrap();

                recompressed.write_bit(last).unwrap();
                compressed_block(&mut recompressed, &block).unwrap();

                // TODO
                match block {
                    Block::FixedHuffman(codes) | Block::DynamicHuffman { codes, .. } => {}
                    _ => {}
                }
            }
            recompressed.align().unwrap();
        }

        let mut recompressed = recompressed.into_inner().into_inner();
        raw.read_to_end(&mut recompressed).unwrap();

        assert_eq!(raw.into_inner().to_vec(), recompressed);

        assert_eq!(expected_len, decompressed.len());
    }
}
