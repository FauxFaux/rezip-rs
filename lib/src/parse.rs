use std::io::Read;

use crate::bit::BitCollector;
use crate::bit::BitReader;
use crate::code_tree::CodeTree;
use crate::errors::*;
use crate::huffman;

use crate::Block;
use crate::Code;
use crate::Ref;

pub fn parse_deflate<R: Read>(bytes: R) -> BlockIter<R> {
    BlockIter {
        inner: BitReader::new(bytes),
        end: false,
    }
}

pub struct BlockIter<R: Read> {
    inner: BitReader<R>,
    end: bool,
}

impl<R: Read> Iterator for BlockIter<R> {
    type Item = Result<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end {
            return match self.inner.align() {
                Ok(()) => None,
                Err(e) => Some(Err(e)),
            };
        }

        self.end = match self.inner.read_bit() {
            Ok(end) => end,
            Err(e) => return Some(Err(e)),
        };

        Some(read_block(&mut self.inner))
    }
}

fn read_block<R: Read>(reader: &mut BitReader<R>) -> Result<Block> {
    match reader.read_part(2)? {
        0 => {
            reader.align()?;
            reader.read_length_prefixed().map(Block::Uncompressed)
        }
        1 => scan_huffman_data(
            reader,
            &huffman::FIXED_LENGTH_TREE,
            Some(&huffman::FIXED_DISTANCE_TREE),
        )
        .map(Block::FixedHuffman),
        2 => {
            // scope-based borrow sigh
            let ((length, distance), trees) = {
                let mut tracker = BitCollector::new(reader);
                (huffman::read_codes(&mut tracker)?, tracker.into_data())
            };

            scan_huffman_data(reader, &length, distance.as_ref())
                .map(|codes| Block::DynamicHuffman { trees, codes })
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

        let dist_sym = match distance {
            Some(dist_code) => dist_code.decode_symbol(reader)?,
            None => bail!("length symbol encountered but no table"),
        };

        let dist = huffman::decode_distance(reader, dist_sym)?;

        ensure!(dist >= 1 && dist <= 32_786, "invalid distance");

        ret.push(Code::Reference(Ref::new(dist, run)));
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::io::Read;

    use super::*;

    #[test]
    fn parse_lol() {
        // fixed huffman, no backreferences

        use super::Code::Literal;

        assert_eq!(
            vec![Block::FixedHuffman(vec![
                Literal(108),
                Literal(111),
                Literal(108),
            ])],
            parse_deflate(Cursor::new(&include_bytes!("../tests/data/lol.gz")[10..]))
                .map(|val| val.unwrap())
                .collect::<Vec<Block>>()
        );
    }
}
