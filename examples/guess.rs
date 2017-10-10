#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;

use librezip::Result;
use librezip::Block;
use librezip::Code;

use librezip::guess;
use librezip::circles::CircularBuffer;

quick_main!(run);

fn run() -> Result<()> {
    let input = env::args().nth(1).ok_or("first argument: input-path.gz")?;
    let mut reader = io::BufReader::new(fs::File::open(input)?);
    librezip::gzip::discard_header(&mut reader)?;

    let mut dictionary = CircularBuffer::new();

    for (id, block) in librezip::parse_deflate(&mut reader).into_iter().enumerate() {
        let block = block?;

        println!("block {}:", id);
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                println!(" - uncompressed: {} bytes", data.len());
                dictionary.extend(&data);
            }
            FixedHuffman(codes) => {
                println!(" - fixed huffman:");
                print(&mut dictionary, &codes);
            }
            DynamicHuffman { trees, codes } => {
                println!(" - dynamic huffman: {:?}", trees);
                print(&mut dictionary, &codes);
            }
        }
    }

    Ok(())
}

fn print(dictionary: &mut CircularBuffer, codes: &[Code]) {
    let max = guess::max_distance(codes);
    println!("   max len: {:?}", max);

    let outside_range = guess::outside_range(codes);
    println!("   outta bounds: {}", outside_range);

    println!(
        "   block_encode: {:?}",
        guess::validate_reencode(max.unwrap_or(0), &dictionary.vec(), codes)
    );

    // AWFUL
    for byte in librezip::serialise::DecompressedBytes::new(&dictionary.vec(), codes.iter()) {
        dictionary.push(byte);
    }
}
