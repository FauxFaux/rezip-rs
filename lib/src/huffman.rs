use std;
use std::io::Read;

use bit::BitReader;
use bit::BitSource;
use code_tree::CodeTree;
use errors::*;

lazy_static! {
    pub static ref FIXED_LENGTH_TREE: CodeTree = {
        let mut lens = [0u8; 288];
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

    pub static ref FIXED_DISTANCE_TREE: CodeTree =
        CodeTree::new(&[5u8; 32]).expect("static data is valid");
}

pub fn read_codes<B: BitSource>(reader: &mut B) -> Result<(CodeTree, Option<CodeTree>)> {
    let num_lit_len_codes = u16::from(reader.read_part(5)?) + 257;
    let num_distance_codes = reader.read_part(5)? + 1;

    let num_code_len_codes = reader.read_part(4)? + 4;

    let mut code_len_code_len = [0u8; 19];
    code_len_code_len[16] = reader.read_part(3)? as u8;
    code_len_code_len[17] = reader.read_part(3)? as u8;
    code_len_code_len[18] = reader.read_part(3)? as u8;
    code_len_code_len[0] = reader.read_part(3)? as u8;

    for i in 0..(num_code_len_codes as usize - 4) {
        let pos = if i % 2 == 0 { 8 + i / 2 } else { 7 - i / 2 };
        code_len_code_len[pos] = reader.read_part(3)? as u8;
    }

    let code_len_code = CodeTree::new(&code_len_code_len[..])?;

    let code_lens_len = num_lit_len_codes as usize + num_distance_codes as usize;
    let mut code_lens = vec![];
    for _ in 0..code_lens_len {
        code_lens.push(0u8);
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
                code_lens[i] = sym as u8;
                run_val = Some(sym as u8);
                i += 1;
            } else if sym == 16 {
                ensure!(run_val.is_some(), "no value to copy");
                run_len = reader.read_part(2)? + 3;
            } else if sym == 17 {
                run_val = Some(0);
                run_len = reader.read_part(3)? + 3;
            } else if sym == 18 {
                run_val = Some(0);
                run_len = reader.read_part(7)? + 11;
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

pub fn encode_run_length(length: u16) -> u16 {
    match length {
        3...10 => 257 + length - 3,
        11...18 => 265 + (length - 11) / 2,
        19...34 => 269 + (length - 19) / 4,
        35...66 => 273 + (length - 35) / 8,
        67...130 => 277 + (length - 67) / 16,
        131...257 => 281 + (length - 131) / 32,
        258 => 285,
        _ => panic!("insane run length"),
    }
}

/// returns: Some(bit count, data (of which the first bit bits are valid)),
/// or None if the length is zero
///
/// This does not seem like a great API.
pub fn extra_run_length(length: u16) -> Option<(u8, u16)> {
    match length {
        3...10 => None,
        11...18 => Some((1, (length - 11) % 2)),
        19...34 => Some((2, (length - 19) % 4)),
        35...66 => Some((3, (length - 35) % 8)),
        67...130 => Some((4, (length - 67) % 16)),
        131...257 => Some((5, (length - 131) % 32)),
        258 => None,
        _ => panic!("insane run length"),
    }
}

/// Returns a run length between 3 and 258 inclusive, all other values are invalid.
pub fn decode_run_length<R: Read>(reader: &mut BitReader<R>, sym: u16) -> Result<u16> {
    ensure!(sym >= 257 && sym <= 287, "decompressor bug");

    if sym <= 264 {
        return Ok((sym - 254) as u16);
    }

    if sym <= 284 {
        // 284 - 261 == 23
        // 23 / 4 == 5.7 -> 5.
        let extra_bits = ((sym - 261) / 4) as u8;
        assert!(extra_bits < 6);

        let high_part = (((sym - 265) as u8) % 4 + 4) << extra_bits;
        let low_part = reader.read_part(extra_bits)? as u8;
        return Ok(u16::from(high_part) + u16::from(low_part) + 3);
    }

    if sym == 285 {
        return Ok(258);
    }

    // sym is 286 or 287
    bail!("reserved symbol: {}", sym);
}

/// Returns: Some(code, bit count, bits); never None (sigh)
pub fn encode_distance(distance: u16) -> Option<(u8, u8, u16)> {
    if distance <= 4 {
        Some((distance as u8 - 1, 0, 0))
    } else {
        let mut extra_bits = 1;
        let mut code = 4;
        let mut base = 4;

        while base * 2 < distance {
            extra_bits += 1;
            code += 2;
            base *= 2;
        }

        let half = base / 2;
        let delta = distance - base - 1;

        if distance <= base + half {
            Some((code, extra_bits, delta % half))
        } else {
            Some((code + 1, extra_bits, delta % half))
        }
    }
}

pub fn decode_distance<R: Read>(reader: &mut BitReader<R>, sym: u16) -> Result<u16> {
    ensure!(sym <= 31, "invalid distance symbol");

    if sym <= 3 {
        return Ok(sym as u16 + 1);
    }

    if sym <= 29 {
        let num_extra_bits = (sym / 2 - 1) as u8;
        return Ok((((sym % 2 + 2) as u16) << num_extra_bits) + 1
            + reader.read_part(num_extra_bits)?);
    }

    bail!("reserved distance symbol")
}

#[test]
fn print_fixed_tree() {
    println!("{:?}", *FIXED_LENGTH_TREE);
}
