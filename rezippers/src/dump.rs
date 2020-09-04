use std::io::Read;

use anyhow::Error;

use librezip;
use librezip::Block;
use librezip::Code;

pub fn run<R: Read>(mut reader: R) -> Result<(), Error> {
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
            Reference(r) => {
                println!(
                    "    - backref: {} byte(s) back, {} bytes long",
                    r.dist,
                    r.run()
                );
            }
        }
    }
}
