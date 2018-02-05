use std::io;
use std::io::Read;
use std::io::Write;

use byteorder::LE;
use byteorder::WriteBytesExt;
use crc;
use crc::Hasher32;
use flate2;
use librezip;

use errors::*;

const RSYNC_MIN: usize = 1024 * 8;
const RSYNC_MOD: usize = 1024 * 4;
const RSYNC_MAX: usize = 1024 * 64;

/// Return: empty iff input is empty.
fn take_rsync<I: Iterator<Item = io::Result<u8>>>(from: &mut I) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(RSYNC_MAX);
    let mut sum = 0usize;
    for _ in 0..RSYNC_MIN {
        match from.next() {
            Some(byte) => {
                let byte = byte?;
                sum = sum.wrapping_add(usize::from(byte));
                buf.push(byte);
            }
            None => return Ok(buf),
        }
    }

    for pos in 0..(RSYNC_MAX - RSYNC_MIN) {
        if 0 == (sum % RSYNC_MOD) {
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
    let mut reader = reader.bytes().peekable();
    let writer = io::stdout();
    let mut writer = writer.lock();

    assert!(
        reader.peek().is_some(),
        "TODO: can't deal with an empty file"
    );

    writer.write_all(&orig_header)?;

    let mut data_len = 0u32;
    let mut data_csum = crc::crc32::Digest::new(crc::crc32::IEEE);

    loop {
        let buf = take_rsync(&mut reader)?;
        if buf.is_empty() {
            break;
        }

        data_len = data_len.wrapping_add(buf.len() as u32);
        data_csum.write(&buf);

        if reader.peek().is_some() {
            // uncompressed block, not end of file
            writer.write_all(&[0])?;
        } else {
            // uncompressed block, end of file
            writer.write_all(&[0b0000_0001])?;
        }

        let len = buf.len() as u16;
        writer.write_u16::<LE>(len)?;
        writer.write_u16::<LE>(len ^ 0xffff)?;
        writer.write_all(&buf)?;
    }

    writer.write_u32::<LE>(data_csum.sum32())?;
    writer.write_u32::<LE>(data_len)?;
    Ok(())
}
