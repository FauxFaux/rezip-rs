#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;

use librezip::Result;
use librezip::Block;
use librezip::Code;
use librezip::unpack_run;

fn run() -> Result<()> {
    let input = env::args().nth(1).ok_or("first argument: input-path.gz")?;
    let mut reader = io::BufReader::new(fs::File::open(input)?);
    librezip::gzip::discard_header(&mut reader)?;
    for (id, block) in librezip::parse_deflate(&mut reader).into_iter().enumerate() {
        let block = block?;

        println!("block {}:", id);
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                println!(" - uncompressed: {} bytes", data.len());
            }
            FixedHuffman(codes) => {
                println!(" - fixed huffman:");
                print(&codes);
            }
            DynamicHuffman { trees, codes } => {
                println!(" - dynamic huffman: {:?}", trees);
                print(&codes);
            }
        }
    }

    Ok(())
}

fn print(codes: &[Code]) {
    use self::Code::*;

    for code in codes {
        match *code {
            Literal(chr) => {
                println!("    - lit: 0x{:02x}: {:?}", chr, char::from(chr));
            }
            Reference { dist, run_minus_3 } => {
                println!(
                    "    - backref: {} byte(s) back, {} bytes long",
                    dist,
                    unpack_run(run_minus_3)
                );
            }
        }
    }
}

quick_main!(run);
