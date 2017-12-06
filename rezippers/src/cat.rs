use std::io;
use std::io::Read;
use std::io::Write;

use librezip;
use librezip::Block;
use librezip::circles::CircularBuffer;

use errors::*;

pub fn run<R: Read>(mut reader: R) -> Result<()> {
    librezip::gzip::discard_header(&mut reader)?;

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut dictionary = CircularBuffer::new();

    for block in librezip::parse_deflate(&mut reader).into_iter() {
        let block = block?;
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                stdout.write_all(&data)?;
                dictionary.extend(&data);
            }
            FixedHuffman(ref codes) | DynamicHuffman { ref codes, .. } => {
                let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());
                librezip::serialise::decompressed_block(&mut decompressed, &mut dictionary, &block)?;
                stdout.write_all(&decompressed)?;
            }
        }
    }

    Ok(())
}
