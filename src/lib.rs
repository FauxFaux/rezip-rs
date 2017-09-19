#[macro_use]
extern crate error_chain;

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

fn dump<R: Read>(mut from: R) -> Result<()> {
    let mut header = [0u8; 10];
    from.read_exact(&mut header)?;

    let mut reader = bit::BitReader::new(from);
    let mut writer = Cursor::new(vec![]);
    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);

    loop {
        let final_block = reader.read_always()?;

        match reader.read_part_u8(2)? {
            0 => read_uncompressed()?,
            1 => read_huffman(&mut reader, &mut writer, &mut dictionary,unimplemented!(), unimplemented!())?,
            2 => {
                let (length, distance) = read_huffman_codes(&mut reader)?;
                read_huffman(&mut reader, &mut writer, &mut dictionary, &length, distance.as_ref())?
            }
            3 => bail!("reserved block type"),
            _ => unreachable!(),
        }

        if final_block {
            break;
        }
    }

    Ok(())
}

fn read_huffman_codes<R: Read>(
    reader: &mut bit::BitReader<R>,
) -> Result<(CodeTree, Option<CodeTree>)> {
    let num_lit_len_codes = reader.read_part_u8(5)? as u16 + 257;
    let num_distance_codes = reader.read_part_u8(5)? + 1;

    let num_code_len_codes = reader.read_part_u8(4)? + 4;

    let mut code_len_code_len = [0u32; 19];
    code_len_code_len[16] = reader.read_part_u8(3)? as u32;
    code_len_code_len[17] = reader.read_part_u8(3)? as u32;
    code_len_code_len[18] = reader.read_part_u8(3)? as u32;
    code_len_code_len[0] = reader.read_part_u8(3)? as u32;

    for i in 0..(num_code_len_codes as usize - 4) {
        let pos = if i % 2 == 0 { 8 + i / 2 } else { 7 - i / 2 };
        code_len_code_len[pos] = reader.read_part_u8(3)? as u32;
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
            let sym = decode_symbol(reader, &code_len_code)?;
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

    if 1 == one_count && 0 == other_positive_count {
        unimplemented!()
    }

    Ok((lit_len_code, Some(CodeTree::new(dist_code_len)?)))
}

fn decode_symbol<R: Read>(reader: &mut bit::BitReader<R>, code_tree: &CodeTree) -> Result<u32> {
    let mut left = code_tree.left.clone();
    let mut right = code_tree.right.clone();

    use code_tree::Node::*;

    loop {
        match *if reader.read_always()? { right } else { left } {
            Leaf(sym) => return Ok(sym),
            Internal(ref new_left, ref new_right) => {
                left = new_left.clone();
                right = new_right.clone();
            }
        }
    }
}

fn read_uncompressed() -> Result<()> {
    unimplemented!()
}

fn read_huffman<R: Read, W: Write>(
    reader: &mut bit::BitReader<R>,
    mut output: W,
    dictionary: &mut CircularBuffer,
    length: &CodeTree,
    distance: Option<&CodeTree>,
) -> Result<()> {
    loop {
        let sym = decode_symbol(reader, length)?;
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
        let run = decode_run_length(sym)?;
        ensure!(run >= 3 && run <= 258, "invalid run length");
        let dist_sym = match distance {
            Some(dist_code) => decode_symbol(reader, dist_code)?,
            None => bail!("length symbol encountered but no table"),
        };

        let dist = decode_distance(dist_sym)?;

        ensure!(dist >= 1 && dist <= 32786, "invalid distance");
        dictionary.copy(dist, run, &mut output)?;
    }
}

fn decode_run_length(sym: u32) -> Result<u32> {
    unimplemented!()
}

fn decode_distance(sym: u32) -> Result<u32> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[test]
    fn dump() {
        ::dump(Cursor::new(&include_bytes!("../tests/data/seq-20.gz")[..])).unwrap();
    }
}
