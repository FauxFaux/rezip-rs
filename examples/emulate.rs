#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;
use std::io::Write;

use librezip::Result;
use librezip::Block;

use librezip::emulate;
use librezip::serialise;
use librezip::circles::CircularBuffer;

quick_main!(run);

fn run() -> Result<()> {
    let input = env::args().nth(1).ok_or("first argument: input-path.gz")?;
    let mut reader = io::BufReader::new(fs::File::open(input)?);
    librezip::gzip::discard_header(&mut reader)?;

    let mut dictionary = CircularBuffer::new();

    for (id, block) in librezip::parse_deflate(&mut reader).into_iter().enumerate() {
        let block = block?;
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                println!("block {}: uncompressed: {} bytes\n", id, data.len());
                dictionary.extend(&data);
            }
            FixedHuffman(ref codes) | DynamicHuffman { ref codes, .. } => {
                println!("block {}: huffman codes: {}\n", id, codes.len());
                let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());

                let before = dictionary.vec();
                serialise::decompressed_block(&mut decompressed, &mut dictionary, &block)?;
                assert_eq!(*codes, emulate::gzip(&before, &decompressed)?);
            }
        }
    }

    Ok(())
}
