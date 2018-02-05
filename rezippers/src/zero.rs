use std::io;
use std::io::Read;

use flate2;
use librezip;

use errors::*;

const CHUNK_SIZE: usize = 8096;
const MOD: usize = 4096;
const MAX: usize = 32768;

/// Return: empty iff input is empty.
fn take_rsync<I: Iterator<Item = io::Result<u8>>>(from: &mut I) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(MAX);
    let mut sum = 0usize;
    for _ in 0..CHUNK_SIZE {
        match from.next() {
            Some(byte) => {
                let byte = byte?;
                sum = sum.wrapping_add(usize::from(byte));
                buf.push(byte);
            }
            None => return Ok(buf),
        }
    }

    for pos in 0..(MAX - CHUNK_SIZE) {
        if 0 == (sum % MOD) {
            break;
        }

        match from.next() {
            Some(byte) => {
                let byte = byte?;
                sum = sum.wrapping_add(usize::from(byte));
                sum = sum.wrapping_sub(usize::from(buf[pos]));
                buf.push(byte);
            }
            None => break,
        }
    }

    Ok(buf)
}

pub fn run<R: Read>(mut reader: R) -> Result<()> {
    let orig_header = librezip::gzip::discard_header(&mut reader)?;

    let reader = flate2::bufread::DeflateDecoder::new(io::BufReader::new(reader));
    let mut reader = reader.bytes();
    let mut writer = io::stdout();

    loop {
        let buf = take_rsync(&mut reader)?;
        if buf.is_empty() {
            break;
        }
        println!("{}", buf.len());
    }

    Ok(())
}
