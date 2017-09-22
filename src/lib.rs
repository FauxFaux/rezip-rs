#[macro_use]
extern crate error_chain;

extern crate itertools;

#[macro_use]
extern crate lazy_static;

use std::io::Cursor;
use std::io::Read;
use std::io::Write;

mod bit;
mod circles;
mod code_tree;
mod errors;

use code_tree::CodeTree;
use circles::CircularBuffer;
use errors::*;

pub fn process<R: Read, W: Write>(mut from: R, mut into: W) -> Result<Vec<()>> {
    discard_gzip(&mut from)?;

    let mut reader = bit::BitReader::new(from);
    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);

    let mut ret = vec![];

    loop {
        let BlockDone { final_block, data, .. } = read_block(&mut reader, &mut dictionary)?;

        ret.push(());

        // ensure reproducibility

        into.write_all(&data)?;

        if final_block {
            break;
        }
    }

    Ok(ret)
}

fn discard_gzip<R: Read>(mut from: R) -> Result<()> {
    let mut header = [0u8; 10];
    from.read_exact(&mut header)?;

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
        let extra_field_length = ((buf[1] as usize) << 8) | (buf[0] as usize);
        from.read_exact(&mut vec![0u8; extra_field_length])?;
    }

    if has_bit(flags, 3) {
        // fname
        read_null_terminated(&mut from)?;
    }

    if has_bit(flags, 4) {
        // comment
        read_null_terminated(&mut from)?;
    }

    if has_bit(flags, 1) {
        // CRC
        from.read_exact(&mut [0u8; 2])?;
    }

    Ok(())
}

fn has_bit(val: u8, bit: u8) -> bool {
    (val & (1 << bit)) == (1 << bit)
}

lazy_static! {
    static ref FIXED_HUFFMAN_LENGTH_TREE: CodeTree = {
        let mut lens = [0u32; 288];
        for i in 0..144 {
            lens[i] = 8;
        }
        for i in 144..256 {
            lens[i] = 9;
        }
        for i in 256..280 {
            lens[i] = 7;
        }
        for i in 280..288 {
            lens[i] = 8;
        }

        CodeTree::new(&lens).expect("static data is valid")
    };

    static ref FIXED_HUFFMAN_DISTANCE_TREE: CodeTree =
        CodeTree::new(&[5u32; 32]).expect("static data is valid");
}

fn read_null_terminated<R: Read>(mut from: R) -> Result<()> {
    loop {
        let mut buf = [0u8; 1];
        from.read_exact(&mut buf)?;
        if 0 == buf[0] {
            return Ok(());
        }
    }
}

struct BlockDone {
    final_block: bool,
    data: Vec<u8>,
}

fn read_block<R: Read>(
    reader: &mut bit::BitReader<R>,
    dictionary: &mut CircularBuffer,
) -> Result<BlockDone> {
    let final_block = reader.read_always()?;
    let mut writer = Cursor::new(vec![]);

    match reader.read_part_u8(2)? {
        0 => read_uncompressed(reader, &mut writer, dictionary)?,
        1 => {
            read_huffman(
                reader,
                &mut writer,
                dictionary,
                &FIXED_HUFFMAN_LENGTH_TREE,
                Some(&FIXED_HUFFMAN_DISTANCE_TREE),
            )?
        }
        2 => {
            let (length, distance) = read_huffman_codes(reader)?;
            read_huffman(reader, &mut writer, dictionary, &length, distance.as_ref())?
        }
        3 => bail!("reserved block type"),
        _ => unreachable!(),
    }

    Ok(BlockDone {
        final_block,
        data: writer.into_inner(),
    })
}

fn read_huffman_codes<R: Read>(
    reader: &mut bit::BitReader<R>,
) -> Result<(CodeTree, Option<CodeTree>)> {
    let num_lit_len_codes = u16::from(reader.read_part_u8(5)?) + 257;
    let num_distance_codes = reader.read_part_u8(5)? + 1;

    let num_code_len_codes = reader.read_part_u8(4)? + 4;

    let mut code_len_code_len = [0u32; 19];
    code_len_code_len[16] = u32::from(reader.read_part_u8(3)?);
    code_len_code_len[17] = u32::from(reader.read_part_u8(3)?);
    code_len_code_len[18] = u32::from(reader.read_part_u8(3)?);
    code_len_code_len[0] = u32::from(reader.read_part_u8(3)?);

    for i in 0..(num_code_len_codes as usize - 4) {
        let pos = if i % 2 == 0 { 8 + i / 2 } else { 7 - i / 2 };
        code_len_code_len[pos] = u32::from(reader.read_part_u8(3)?);
    }

    let code_len_code = CodeTree::new(&code_len_code_len[..])?;

    let code_lens_len = num_lit_len_codes as usize + num_distance_codes as usize;
    let mut code_lens = vec![];
    for _ in 0..code_lens_len {
        code_lens.push(0);
    }

    let mut run_val = None;
    let mut run_len = 0;

    let mut i = 0;
    loop {
        if run_len > 0 {
            match run_val {
                Some(val) => code_lens[i] = val,
                None => bail!("invalid state"),
            }
            run_len -= 1;
            i += 1;
        } else {
            let sym = code_len_code.decode_symbol(reader)?;
            if sym <= 15 {
                code_lens[i] = sym;
                run_val = Some(sym);
                i += 1;
            } else if sym == 16 {
                ensure!(run_val.is_some(), "no value to copy");
                run_len = reader.read_part_u8(2)? + 3;
            } else if sym == 17 {
                run_val = Some(0);
                run_len = reader.read_part_u8(3)? + 3;
            } else if sym == 18 {
                run_val = Some(0);
                run_len = reader.read_part_u8(7)? + 11;
            } else {
                panic!("symbol out of range");
            }
        }

        if i >= code_lens_len {
            break;
        }
    }

    ensure!(run_len == 0, "run exceeds number of codes");

    let lit_len_code = CodeTree::new(&code_lens[0..num_lit_len_codes as usize])?;
    let dist_code_len = &code_lens[num_lit_len_codes as usize..];

    if 1 == dist_code_len.len() && 0 == dist_code_len[0] {
        return Ok((lit_len_code, None));
    }

    let mut one_count = 0;
    let mut other_positive_count = 0;

    for x in dist_code_len {
        if *x == 1 {
            one_count += 1;
        } else if *x > 1 {
            other_positive_count += 1;
        }
    }

    let dist_tree = if 1 == one_count && 0 == other_positive_count {
        // there's only one valid distance code, we have to fiddle with the
        // data so that the build succeeds: we insert a dummy code at the end

        let mut new_lens = [0; 32];

        let to_copy = std::cmp::min(dist_code_len.len(), 31);
        new_lens[..to_copy].copy_from_slice(&dist_code_len[..to_copy]);

        // dummy code
        new_lens[31] = 1;

        CodeTree::new(&new_lens)?
    } else {
        CodeTree::new(dist_code_len)?
    };

    Ok((lit_len_code, Some(dist_tree)))
}

fn read_uncompressed<R: Read, W: Write>(
    reader: &mut bit::BitReader<R>,
    mut output: W,
    dictionary: &mut CircularBuffer,
) -> Result<()> {
    while 0 != reader.position() {
        ensure!(
            !reader.read_always()?,
            "padding bits should always be empty"
        );
    }

    let len = reader.read_aligned_u16()?;
    let ones_complement = reader.read_aligned_u16()?;
    ensure!(
        (len ^ 0xFFFF) == ones_complement,
        "uncompressed block length validation failed"
    );

    for _ in 0..len {
        let byte = reader.read_aligned_u8()?;

        output.write_all(&[byte])?;
        dictionary.append(byte);
    }

    Ok(())
}

fn read_huffman<R: Read, W: Write>(
    reader: &mut bit::BitReader<R>,
    mut output: W,
    dictionary: &mut CircularBuffer,
    length: &CodeTree,
    distance: Option<&CodeTree>,
) -> Result<()> {
    loop {
        let sym = length.decode_symbol(reader)?;
        if sym == 256 {
            // end of block
            return Ok(());
        }

        if sym < 256 {
            // literal byte
            output.write_all(&[sym as u8])?;
            dictionary.append(sym as u8);
            continue;
        }

        // length and distance encoding
        let run = decode_run_length(reader, sym)?;
        ensure!(run >= 3 && run <= 258, "invalid run length");
        let dist_sym = match distance {
            Some(dist_code) => dist_code.decode_symbol(reader)?,
            None => bail!("length symbol encountered but no table"),
        };

        let dist = decode_distance(reader, dist_sym)?;

        ensure!(dist >= 1 && dist <= 32_786, "invalid distance");
        dictionary.copy(dist, run, &mut output)?;
    }
}

fn decode_run_length<R: Read>(reader: &mut bit::BitReader<R>, sym: u32) -> Result<u32> {
    ensure!(sym >= 257 && sym <= 287, "decompressor bug");

    if sym <= 264 {
        return Ok(sym - 254);
    }

    if sym <= 284 {
        // 284 - 261 == 23
        // 23 / 4 == 5.7 -> 5.
        let extra_bits = ((sym - 261) / 4) as u8;
        return Ok(
            (((sym - 265) % 4 + 4) << extra_bits) + 3 +
                u32::from(reader.read_part_u8(extra_bits)?),
        );
    }

    if sym == 285 {
        return Ok(258);
    }

    // sym is 286 or 287
    bail!("reserved symbol: {}", sym);
}

fn decode_distance<R: Read>(reader: &mut bit::BitReader<R>, sym: u32) -> Result<u32> {
    ensure!(sym <= 31, "invalid distance symbol");

    if sym <= 3 {
        return Ok(sym + 1);
    }

    if sym <= 29 {
        let num_extra_bits = (sym / 2 - 1) as u8;
        return Ok(
            ((sym % 2 + 2) << num_extra_bits) + 1 +
                u32::from(reader.read_part_u16(num_extra_bits)?),
        );
    }

    bail!("reserved distance symbol")
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use ::*;

    #[test]
    fn seq_20() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            1,
            process(
                Cursor::new(&include_bytes!("../tests/data/seq-20.gz")[..]),
                &mut output,
            ).unwrap()
                .len()
        );

        let seq_20 = (1..21)
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join("\n") + "\n";

        assert_eq!(
            seq_20,
            String::from_utf8(output.into_inner().into_iter().collect()).unwrap()
        );
    }

    #[test]
    fn stored_hunk() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            18,
            process(
                Cursor::new(
                    &include_bytes!("../tests/data/librole-basic-perl_0.13-1.debian.tar.gz")[..],
                ),
                &mut output,
            ).unwrap()
                .len()
        );
    }

    #[test]
    fn some_flags_set() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            1, // TODO
            process(
                Cursor::new(
                    &include_bytes!("../tests/data/libcgi-untaint-email-perl_0.03.orig.tar.gz")[..],
                ),
                &mut output,
            ).unwrap()
                .len()
        );
    }
}
