#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate librezip;

extern crate flate2;
extern crate hex;
extern crate tempfile;

use std::fs::File;
use std::io::Cursor;
use std::io::Write;
use std::process;

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

    if 2 == slice.len() {
        return;
    }

    let gzip_file = exec_actual_gzip(data);
    if compressed[..] != gzip_file[10..gzip_file.len()-8] {
        return;
    }

    println!("input: {}", hex::encode(data));
    println!("compr: {}", hex::encode(&compressed));
    println!("slice: {:?}", slice);
    println!("codes: {:?}", codes);

    panic!()
}

fn exec_actual_gzip(input: &[u8]) -> Vec<u8> {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(input).unwrap();
    tmp.flush().unwrap();
    process::Command::new("gzip")
        .args(&["-n1c", tmp.path().to_str().unwrap()])
        .output()
        .unwrap()
        .stdout
}
