use std::io::Write;

use bit::BitWriter;
use circles::CircularBuffer;
use code_tree::CodeTree;
use errors::*;
use huffman;
use parse::Code;
use parse::Block;

pub fn decompressed_block<W: Write>(
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
        DynamicHuffman { ref codes, .. } => decompressed_codes(into, dictionary, codes),
    }
}

fn decompressed_codes<W: Write>(
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


pub fn compressed_block<W: Write>(into: &mut BitWriter<W>, block: &Block) -> Result<()> {
    use self::Block::*;

    match *block {
        Uncompressed(ref data) => {
            into.write_bits_val(2, 0)?;
            into.write_length_prefixed(data)?;
            Ok(())
        }
        FixedHuffman(ref codes) => {
            into.write_bits_val(2, 1)?;
            compressed_codes(
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
            into.write_bits_val(2, 2)?;
            into.write_vec(trees)?;
            let (length, distance) = huffman::read_codes(&mut trees.iter())?;
            compressed_codes(into, &length, distance.as_ref(), codes)
        }
    }
}

fn compressed_codes<W: Write>(
    into: &mut BitWriter<W>,
    length_tree: &CodeTree,
    distance_tree: Option<&CodeTree>,
    codes: &[Code],
) -> Result<()> {
    let length_tree = length_tree.invert();
    let distance_tree = distance_tree.map(|tree| tree.invert());

    assert!(length_tree.len() > 256);

    use self::Code::*;

    for code in codes {
        match *code {
            Literal(byte) => {
                into.write_vec(length_tree[byte as usize].as_ref().ok_or(
                    "invalid literal",
                )?)?;
            }
            Reference { dist, run_minus_3 } => {
                let run = u16::from(run_minus_3) + 3;

                into.write_vec(
                    length_tree[huffman::encode_run_length(run) as usize]
                        .as_ref()
                        .unwrap(),
                )?;

                if let Some((bits, val)) = huffman::extra_run_length(run) {
                    into.write_bits_val(bits, val)?;
                }

                if let Some((code, bits, val)) = huffman::encode_distance(dist) {
                    let distance_tree = distance_tree.as_ref().ok_or(
                        "reference but not distance tree",
                    )?;
                    into.write_vec(
                        distance_tree[code as usize].as_ref().unwrap(),
                    )?;
                    if bits > 0 {
                        into.write_bits_val(bits, val)?;
                    }
                }
            }
        }
    }

    // End of stream marker
    into.write_vec(length_tree[256].as_ref().unwrap())?;

    Ok(())
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
            decompressed_block(&mut into, &mut dictionary, &block.unwrap()).unwrap();
        }

        assert_eq!(20480, into.into_inner().len());
    }
}
