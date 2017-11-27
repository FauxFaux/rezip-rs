#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;

use librezip::Result;
use librezip::Block;
use librezip::Code;

use librezip::bestguess;
use librezip::infer;
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

        println!("block {}:", id);
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                println!(" - uncompressed: {} bytes", data.len());
                dictionary.extend(&data);
            }
            FixedHuffman(codes) => {
                println!(" - fixed huffman:");
                print(&mut dictionary, &codes)?;
            }
            DynamicHuffman { trees, codes } => {
                println!(" - dynamic huffman: {:?}", trees);
                print(&mut dictionary, &codes)?;
            }
        }

        let dict = dictionary.vec();
        let observe = 20;
        if dict.len() > observe * 2 {
            let start = &dict[..observe];
            let end = &dict[dict.len() - observe..];
            println!(
                " - dict (after): {} bytes: {:?}...{:?} ({:?}...{:?})",
                dict.len(),
                start,
                end,
                String::from_utf8_lossy(start),
                String::from_utf8_lossy(end)
            );
        }
    }

    Ok(())
}

fn print(dictionary: &mut CircularBuffer, codes: &[Code]) -> Result<()> {
    let max = infer::max_distance(codes);
    println!("   max len: {:?}", max);

    let (outside_range, hit_zero) = infer::outside_range_or_hit_zero(codes);
    println!("   outta bounds: {}", outside_range);
    println!("       hit zero: {}", hit_zero);

    let old_dictionary = &dictionary.vec();

    let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());
    serialise::decompressed_codes(&mut decompressed, dictionary, codes)?;

    if max.is_some() {
        println!("   validate_reencode:");

        for reduced in bestguess::reduce_entropy(old_dictionary, &decompressed, codes)? {
            print!("{}", reduced);
        }

        println!();
    }

    Ok(())
}
