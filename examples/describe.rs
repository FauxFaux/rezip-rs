extern crate librezip;

use librezip::BlockType;

use std::env;
use std::fs;
use std::io;
use std::io::Write;

fn main() {
    let input = env::args().nth(1).expect("first argument: input-path.gz");
    let results = librezip::deconstruct(
        io::BufReader::new(fs::File::open(input).expect("input readable")),
        NullWriter {},
    ).expect("processing");

    println!("digest: sha512-256");
    println!(" input: {}", hexify(&results.sha512_compressed[0..256 / 8]));
    println!(
        "output: {}",
        hexify(&results.sha512_decompressed[0..256 / 8])
    );

    println!("header: {}", hex_and_ascii(&results.header));

    println!("frames:");

    for result in results.instructions {
        match result.block_type {
            BlockType::Uncompressed => {
                println!(" - type: uncompressed");
                println!("   run: {} bytes", result.len);
            }
            BlockType::Fixed(symbols) => {
                println!(" - type: fixed huffman");
                print_symbols(&symbols);
            }
            BlockType::Dynamic(encoded, symbols) => {
                println!(" - type: dynamic huffman");
                println!("   tree: {:?}", encoded);
                print_symbols(&symbols);

            }
        }
    }

    println!("footer: {}", hex_and_ascii(&results.tail));
}

fn print_symbols(symbols: &librezip::SeenDistanceSymbols) {
    println!("   symbols:");
    for symbol in &symbols.stream {
        println!("    - literals: {}", symbol.literals);
        println!("      symbol: {:?}", symbol.symbol)
    }

    println!("   trailing literals: {}", symbols.trailing_literals);
}

fn hexify(buf: &[u8]) -> String {
    let mut ret = String::with_capacity(buf.len() * 2);
    for c in buf {
        ret.push_str(&format!("{:02x}", c));
    }

    ret
}

fn hex_and_ascii(buf: &[u8]) -> String {
    let mut hex = String::with_capacity(buf.len() * 2);
    let mut ascii = hex.clone();

    for c in buf {
        hex.push_str(&format!("{:02x} ", c));
        ascii.push(match *c as char {
            c if c >= ' ' && c <= '~' => c as char,
            _ => '.',
        });
    }

    format!("{}    {}", hex, ascii)
}

struct NullWriter {}

impl Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, _: &[u8]) -> io::Result<()> {
        Ok(())
    }
}
