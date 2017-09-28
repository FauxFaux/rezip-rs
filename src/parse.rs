use std::io::Read;

use bit::BitReader;
use bit::BitVec;
use code_tree::CodeTree;
use errors::*;
use huffman;

#[derive(Debug, PartialEq, Eq)]
pub enum Code {
    Literal(u8),
    Reference { dist: u16, run_minus_3: u8 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Block {
    Uncompressed(Vec<u8>),
    FixedHuffman(Vec<Code>),
    DynamicHuffman { trees: BitVec, items: Vec<Code> },
}

pub fn parse_deflate<R: Read>(reader: &mut BitReader<R>) -> Result<Vec<Block>> {

    let mut blocks = Vec::new();

    loop {
        let last_block = reader.read_bit()?;
        blocks.push(read_block(reader)?);

        if last_block {
            reader.align()?;
            break;
        }
    }

    Ok(blocks)
}

fn read_block<R: Read>(reader: &mut BitReader<R>) -> Result<Block> {
    match reader.read_part(2)? {
        0 => {
            reader.align()?;
            reader.read_length_prefixed().map(|data| {
                Block::Uncompressed(data)
            })
        }
        1 => {
            scan_huffman_data(
                reader,
                &huffman::FIXED_LENGTH_TREE,
                Some(&huffman::FIXED_DISTANCE_TREE),
            ).map(|data| Block::FixedHuffman(data))
        }
        2 => {
            reader.tracking_start();
            let (length, distance) = huffman::read_codes(reader)?;
            let trees = reader.tracking_finish();
            scan_huffman_data(reader, &length, distance.as_ref()).map(
                |items| Block::DynamicHuffman { trees, items },
            )
        }
        3 => bail!("reserved block type"),
        _ => unreachable!(),
    }
}

fn scan_huffman_data<R: Read>(
    reader: &mut BitReader<R>,
    length: &CodeTree,
    distance: Option<&CodeTree>,
) -> Result<Vec<Code>> {

    let mut ret = Vec::new();

    loop {
        let sym = length.decode_symbol(reader)?;

        if sym == 256 {
            // end of block

            break;
        }

        if sym < 256 {
            // literal byte

            ret.push(Code::Literal(sym as u8));
            continue;
        }

        // length and distance encoding
        let run = huffman::decode_run_length(reader, sym)?;
        ensure!(run >= 3 && run <= 258, "invalid run length");

        let dist_sym = match distance {
            Some(dist_code) => dist_code.decode_symbol(reader)?,
            None => bail!("length symbol encountered but no table"),
        };

        let dist = huffman::decode_distance(reader, dist_sym)?;

        ensure!(dist >= 1 && dist <= 32_786, "invalid distance");

        ret.push(Code::Reference {
            dist,
            run_minus_3: (run - 3) as u8,
        });
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::io::Read;

    use bit::BitReader;

    use super::*;

    #[test]
    fn parse_lol() {
        // fixed huffman, no backreferences

        use super::Code::Literal;

        assert_eq!(
            vec![
                Block::FixedHuffman(vec![Literal(108), Literal(111), Literal(108)]),
            ],
            parse_deflate(&mut BitReader::new(Cursor::new(
                &include_bytes!("../tests/data/lol.gz")[10..],
            ))).unwrap()
        );
    }
}
