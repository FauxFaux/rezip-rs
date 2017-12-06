use std::io;
use std::io::Read;

use librezip;
use librezip::CircularBuffer;

use errors::*;

pub fn run<R: Read>(mut reader: R) -> Result<()> {
    librezip::gzip::discard_header(&mut reader)?;

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut dictionary = CircularBuffer::new();

    for block in librezip::parse_deflate(&mut reader).into_iter() {
        librezip::decompressed_block(&mut stdout, &mut dictionary, &block?)?;
    }

    Ok(())
}
