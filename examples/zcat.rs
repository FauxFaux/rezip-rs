#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;
use std::io::Write;

use librezip::Result;
use librezip::Block;
use librezip::Code;

use librezip::infer;
use librezip::serialise;
use librezip::circles::CircularBuffer;

quick_main!(run);

fn run() -> Result<()> {
    let input = env::args().nth(1).ok_or("first argument: input-path.gz")?;
    let mut reader = io::BufReader::new(fs::File::open(input)?);
    librezip::gzip::discard_header(&mut reader)?;

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();

    let mut dictionary = CircularBuffer::new();

    for (id, block) in librezip::parse_deflate(&mut reader).into_iter().enumerate() {
        let block = block?;

        write!(stdout, "\n\nblock {}\n\n", id)?;
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                write!(stderr, " - uncompressed: {} bytes\n", data.len())?;
                stdout.write_all(&data)?;
                dictionary.extend(&data);
            }
            FixedHuffman(ref codes) | DynamicHuffman { ref codes, .. } => {
                write!(stderr, " - huffman codes: {}\n", codes.len())?;
                let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());
                serialise::decompressed_block(&mut decompressed, &mut dictionary, &block)?;
                stdout.write_all(&decompressed)?;
            }
        }
    }

    Ok(())
}
