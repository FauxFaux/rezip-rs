use std::io::Write;

use circles::CircularBuffer;
use code_tree::CodeTree;
use errors::*;
use huffman;
use parse::Code;
use parse::Block;

pub fn decompress_block<W: Write>(
    mut into: W,
    dictionary: &mut CircularBuffer,
    block: &Block,
) -> Result<()> {
    use self::Block::*;

    match *block {
        Uncompressed(ref data) => {
            dictionary.extend(data);
            into.write_all(data).chain_err(
                || "storing uncompressed block",
            )
        }
        FixedHuffman(ref codes) |
        DynamicHuffman { ref codes, .. } => decompress_codes(into, dictionary, codes),
    }
}

fn decompress_codes<W: Write>(
    mut into: W,
    dictionary: &mut CircularBuffer,
    codes: &[Code],
) -> Result<()> {
    use self::Code::*;

    for code in codes {
        match *code {
            Literal(byte) => {
                dictionary.append(byte);
                into.write_all(&[byte])?
            }
            Reference { dist, run_minus_3 } => {
                let run = u16::from(run_minus_3) + 3;
                dictionary.copy(dist, run, &mut into)?;
            }
        }
    }

    Ok(())
}


pub fn write_compressed<W: Write>(mut into: W, block: &Block) -> Result<()> {
    use self::Block::*;

    match *block {
        Uncompressed(ref data) => {
            into.write_all(data)?;
            Ok(())
        }
        FixedHuffman(ref codes) => {
            encode(
                into,
                &huffman::FIXED_LENGTH_TREE,
                Some(&huffman::FIXED_DISTANCE_TREE),
                codes,
            )
        }
        DynamicHuffman {
            ref trees,
            ref codes,
        } => {
            let (length, distance) = huffman::read_codes(&mut trees.iter())?;
            encode(into, &length, distance.as_ref(), codes)
        }
    }
}

fn encode<W: Write>(
    into: W,
    length: &CodeTree,
    distance: Option<&CodeTree>,
    codes: &[Code],
) -> Result<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use parse;
    use super::*;

    #[test]
    fn decompress() {
        let mut into = Cursor::new(vec![]);
        let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
        let raw = Cursor::new(
            &include_bytes!("../tests/data/libcgi-untaint-email-perl_0.03.orig.tar.gz")[37..],
        );
        for block in parse::parse_deflate(raw) {
            decompress_block(&mut into, &mut dictionary, &block.unwrap()).unwrap();
        }

        assert_eq!(20480, into.into_inner().len());
    }
}
