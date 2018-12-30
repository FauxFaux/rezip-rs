use std::io::Read;

use crate::errors::*;

pub fn discard_header<R: Read>(mut from: R) -> Result<Vec<u8>> {
    let mut whole_thing = Vec::new();

    let mut header = [0u8; 10];
    from.read_exact(&mut header)?;
    whole_thing.extend(&header);

    ensure!(0x1f == header[0] && 0x8b == header[1], "invalid magic");
    ensure!(0x08 == header[2], "unsupported compression method");

    let flags = header[3];
    ensure!(0 == (flags & 0b1110_0000), "reserved flags bits set");
    // 4, 5, 6, 7: mtime
    // 8: extra flags (compression level)
    // 9: OS

    if has_bit(flags, 2) {
        // extra
        let mut buf = [0u8; 2];
        from.read_exact(&mut buf)?;
        whole_thing.extend(&buf);
        let extra_field_length = ((buf[1] as usize) << 8) | (buf[0] as usize);
        let mut extra_field = vec![0u8; extra_field_length];
        from.read_exact(&mut extra_field)?;
        whole_thing.extend(&extra_field);
    }

    if has_bit(flags, 3) {
        // fname
        read_null_terminated(&mut from, &mut whole_thing)?;
    }

    if has_bit(flags, 4) {
        // comment
        read_null_terminated(&mut from, &mut whole_thing)?;
    }

    if has_bit(flags, 1) {
        // CRC
        let mut buf = [0u8; 2];
        from.read_exact(&mut buf)?;
        whole_thing.extend(&buf);
    }

    Ok(whole_thing)
}

#[inline]
fn has_bit(val: u8, bit: u8) -> bool {
    (val & (1 << bit)) == (1 << bit)
}

fn read_null_terminated<R: Read>(mut from: R, into: &mut Vec<u8>) -> Result<()> {
    loop {
        let mut buf = [0u8; 1];
        from.read_exact(&mut buf)?;
        into.push(buf[0]);
        if 0 == buf[0] {
            return Ok(());
        }
    }
}
