use std::io;
use std::io::Read;

use failure::Error;
use librezip;
use librezip::CircularBuffer;

pub fn run<R: Read>(mut reader: R) -> Result<(), Error> {
    librezip::gzip::discard_header(&mut reader)?;

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut dictionary = CircularBuffer::new();

    for block in librezip::parse_deflate(&mut reader).into_iter() {
        librezip::decompressed_block(&mut stdout, &mut dictionary, &block?)?;
    }

    Ok(())
}
