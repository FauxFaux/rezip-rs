#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate librezip;
extern crate flate2;

use std::fs::File;
use std::io::Cursor;
use std::io::Write;

use flate2::write::DeflateEncoder;

use librezip::Block;

fuzz_target!(|data: &[u8]| {
    run(data);
});

fn run(data: &[u8]) {
    if data.is_empty() {
        // TODO
        return;
    }

    let mut encoder = DeflateEncoder::new(Vec::with_capacity(data.len()), flate2::Compression::fast());
    encoder.write(&data).expect("writing");
    let compressed = encoder.finish().unwrap();

    let block = match librezip::parse_deflate(Cursor::new(&compressed)).next() {
        Some(Ok(block)) => block,
        other => panic!("couldn't deflate: {:?}", other),
    };

    let codes = match block {
        Block::FixedHuffman(codes)
        | Block::DynamicHuffman { codes, .. } => codes,
        Block::Uncompressed(_) => return,
    };

    let slice = librezip::tracer::try_gzip(1, &[], &data, &codes);

    assert_eq!(2, slice.len());
}
