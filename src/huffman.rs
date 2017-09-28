use std;
use std::fmt;
use std::io::Read;
use std::io::Write;

use bit::BitReader;
use bit::BitVec;
use circles::CircularBuffer;
use code_tree::CodeTree;
use errors::*;

lazy_static! {
    pub static ref FIXED_LENGTH_TREE: CodeTree = {
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

    pub static ref FIXED_DISTANCE_TREE: CodeTree =
        CodeTree::new(&[5u32; 32]).expect("static data is valid");
}

pub fn read_codes<R: Read>(reader: &mut BitReader<R>) -> Result<(CodeTree, Option<CodeTree>)> {
    let num_lit_len_codes = u16::from(reader.read_part(5)?) + 257;
    let num_distance_codes = reader.read_part(5)? + 1;

    let num_code_len_codes = reader.read_part(4)? + 4;

    let mut code_len_code_len = [0u32; 19];
    code_len_code_len[16] = u32::from(reader.read_part(3)?);
    code_len_code_len[17] = u32::from(reader.read_part(3)?);
    code_len_code_len[18] = u32::from(reader.read_part(3)?);
    code_len_code_len[0] = u32::from(reader.read_part(3)?);

    for i in 0..(num_code_len_codes as usize - 4) {
        let pos = if i % 2 == 0 { 8 + i / 2 } else { 7 - i / 2 };
        code_len_code_len[pos] = u32::from(reader.read_part(3)?);
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

pub struct SeenDistanceSymbol {
    pub literals: usize,
    pub symbol: BitVec,
    pub dist: u32,
    pub run_minus_3: u8,
}

#[derive(Debug)]
pub struct SeenDistanceSymbols {
    pub stream: Vec<SeenDistanceSymbol>,
    pub trailing_literals: usize,
}

pub fn read_data<R: Read, W: Write>(
    reader: &mut BitReader<R>,
    mut output: W,
    dictionary: &mut CircularBuffer,
    length: &CodeTree,
    distance: Option<&CodeTree>,
) -> Result<SeenDistanceSymbols> {
    let mut distance_symbols = vec![];
    let mut literals = 0usize;

    loop {
        reader.tracking_start();

        let sym = length.decode_symbol(reader)?;
        if sym == 256 {
            reader.tracking_abort();

            // end of block
            return Ok(SeenDistanceSymbols {
                stream: distance_symbols,
                trailing_literals: literals,
            });
        }

        if sym < 256 {
            // literal byte
            output.write_all(&[sym as u8])?;
            dictionary.append(sym as u8);
            literals += 1;

            reader.tracking_abort();
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

        distance_symbols.push(SeenDistanceSymbol {
            literals,
            symbol: reader.tracking_finish(),
            run_minus_3: (run - 3) as u8,
            dist,
        });
        literals = 0;

        ensure!(dist >= 1 && dist <= 32_786, "invalid distance");
        // TODO: usize >= 32bit
        dictionary.copy(dist as usize, run as usize, &mut output)?;
    }
}

fn decode_run_length<R: Read>(reader: &mut BitReader<R>, sym: u32) -> Result<u32> {
    ensure!(sym >= 257 && sym <= 287, "decompressor bug");

    if sym <= 264 {
        return Ok(sym - 254);
    }

    if sym <= 284 {
        // 284 - 261 == 23
        // 23 / 4 == 5.7 -> 5.
        let extra_bits = ((sym - 261) / 4) as u8;
        return Ok(
            (((sym - 265) % 4 + 4) << extra_bits) + 3 + u32::from(reader.read_part(extra_bits)?),
        );
    }

    if sym == 285 {
        return Ok(258);
    }

    // sym is 286 or 287
    bail!("reserved symbol: {}", sym);
}

fn decode_distance<R: Read>(reader: &mut BitReader<R>, sym: u32) -> Result<u32> {
    ensure!(sym <= 31, "invalid distance symbol");

    if sym <= 3 {
        return Ok(sym + 1);
    }

    if sym <= 29 {
        let num_extra_bits = (sym / 2 - 1) as u8;
        return Ok(
            ((sym % 2 + 2) << num_extra_bits) + 1 + u32::from(reader.read_part(num_extra_bits)?),
        );
    }

    bail!("reserved distance symbol")
}

impl fmt::Debug for SeenDistanceSymbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}, {:?}]", self.literals, self.symbol)
    }
}
