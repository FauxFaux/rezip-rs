#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;

use librezip::Result;
use librezip::Block;
use librezip::Code;

use librezip::circles::CircularBuffer;
use librezip::lookahead;
use librezip::guesser::RefGuesser;
use librezip::serialise;
use librezip::serialise_trace;
use librezip::trace;

use librezip::Trace;

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
    }

    Ok(())
}

fn print(dictionary: &mut CircularBuffer, codes: &[Code]) -> Result<()> {
    let old_dictionary = &dictionary.vec();

    let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());
    serialise::decompressed_codes(&mut decompressed, dictionary, codes)?;

    let rg = RefGuesser::new(old_dictionary, &decompressed);
    let trace = trace::validate(old_dictionary, codes, |finder, pos| {
        lookahead::three_zip(finder, pos)
    });
    let serialise = serialise_trace::verify(&trace);
    print!("   * codes: ");
    for c in codes {
        match *c {
            Code::Literal(byte) if char::from(byte).is_alphanumeric() => {
                print!("{}", char::from(byte))
            }
            Code::Literal(byte) => print!("{:?}", char::from(byte)),
            Code::Reference(r) => print!(" -- {:?} -- ", r),
        }
    }
    println!();
    print!("   * trace: ");
    let mut pos = 0usize;
    for (t, c) in trace.iter().zip(codes.iter()) {
        match *t {
            Trace::Correct => {}
            Trace::Actual(correct) => print!(
                "   {:4}. {:10?} guess: {:?} trace: {:?}\n",
                pos,
                String::from_utf8_lossy(
                    &decompressed[pos.saturating_sub(5)..(pos + 5).min(decompressed.len())]
                ),
                lookahead::three_zip(&rg, pos),
                correct
            ),
        }

        pos += c.emitted_bytes() as usize;
    }
    println!();
    println!("   * after: {}", serialise.len());

    Ok(())
}
