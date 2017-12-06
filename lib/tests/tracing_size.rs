extern crate librezip;

use std::io;

use librezip::CircularBuffer;
use librezip::Block;


fn try_gzip(level: u8, file: &[u8]) -> usize {
    let mut reader = io::Cursor::new(file);
    librezip::gzip::discard_header(&mut reader).unwrap();

    let mut dictionary = CircularBuffer::new();
    let mut sum = 0;

    for block in librezip::parse_deflate(&mut reader).into_iter() {
        let codes = match block.unwrap() {
            Block::Uncompressed(_) => unimplemented!(),
            Block::DynamicHuffman { codes, .. } | Block::FixedHuffman(codes) => codes,
        };

        let preroll = &dictionary.vec();
        let mut data: Vec<u8> = Vec::with_capacity(codes.len());
        librezip::decompressed_codes(&mut data, &mut dictionary, &codes).unwrap();

        sum += librezip::tracer::try_gzip(level, preroll, &data, &codes).len();
    }

    sum
}

// tiny-decay:
// 1abcdef,bcdef-cdef
// 012345678901234567
// LLLLLLLLSRRRRLSRRR
// 1: -----[6,5]-[11,4]
// 3: -----[6,5]-[5,4]
#[test]
fn tiny_decay_1_1() {
    assert_eq!(2, try_gzip(1, include_bytes!("data/tiny-decay-sixteen-1.gz")))
}

#[test]
fn tiny_decay_3_3() {
    assert_eq!(2, try_gzip(3, include_bytes!("data/tiny-decay-sixteen-3.gz")))
}

// decaying: S='defghijklm'; printf "0.abcdefg_hijklm,1.abc$S,2.bc$S,3.c$S,4.$S"
// decaying: 0.abcdefg_hijklm,1.abcdefghijklm,2.bcdefghijklm,3.cdefghijklm,4.defghijklm
#[test]
fn decaying_1_1() {
    assert_eq!(2, try_gzip(1, include_bytes!("data/decaying-sixteen-1.gz")))
}

#[test]
fn decaying_1_2() {
    assert_eq!(2, try_gzip(2, include_bytes!("data/decaying-sixteen-1.gz")))
}

#[test]
fn decaying_1_3() {
    assert_eq!(2, try_gzip(3, include_bytes!("data/decaying-sixteen-1.gz")))
}

#[test]
fn decaying_2_2() {
    assert_eq!(2, try_gzip(2, include_bytes!("data/decaying-sixteen-2.gz")))
}

#[test]
fn decaying_3_3() {
    assert_eq!(2, try_gzip(3, include_bytes!("data/decaying-sixteen-3.gz")))
}
