use std::io::Write;

use cast::usize;
use failure::err_msg;
use failure::Error;
use failure::ResultExt;

use crate::bit::BitVec;
use crate::bit::BitWriter;
use crate::circles::CircularBuffer;
use crate::code_tree::CodeTree;
use crate::huffman;
use crate::Block;
use crate::Code;

pub fn decompressed_block<W: Write>(
    mut into: W,
    dictionary: &mut CircularBuffer,
    block: &Block,
) -> Result<(), Error> {
    use self::Block::*;

    match *block {
        Uncompressed(ref data) => {
            dictionary.extend(data);
            into.write_all(data)
                .with_context(|_| err_msg("storing uncompressed block"))?;
            Ok(())
        }
        FixedHuffman(ref codes) | DynamicHuffman { ref codes, .. } => {
            decompressed_codes(into, dictionary, codes)
        }
    }
}

pub fn decompressed_codes<W: Write>(
    mut into: W,
    dictionary: &mut CircularBuffer,
    codes: &[Code],
) -> Result<(), Error> {
    use self::Code::*;

    for code in codes {
        match *code {
            Literal(byte) => {
                dictionary.push(byte);
                into.write_all(&[byte])?
            }
            Reference(r) => {
                dictionary.copy(r.dist, r.run(), &mut into)?;
            }
        }
    }

    Ok(())
}

pub fn compressed_block<W: Write>(into: &mut BitWriter<W>, block: &Block) -> Result<(), Error> {
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

pub struct Lengths {
    length: Vec<Option<u8>>,
    distance: Vec<Option<u8>>,
    pub mean_literal_len: u8,
}

fn tree_to_lengths(tree: &CodeTree) -> Vec<Option<u8>> {
    tree.invert()
        .into_iter()
        .map(|opt| opt.map(|vec| vec.len() as u8))
        .collect()
}

impl Lengths {
    pub fn new(length_tree: &CodeTree, distance_tree: &CodeTree) -> Self {
        let length = tree_to_lengths(length_tree);
        let all_lengths: usize = length.iter().filter_map(|x| x.map(usize::from)).sum();
        let populated_lengths: usize = 1 + length.iter().filter_map(|x| *x).count();
        Lengths {
            length,
            distance: tree_to_lengths(distance_tree),
            mean_literal_len: ((all_lengths + populated_lengths) / populated_lengths) as u8,
        }
    }

    pub fn length(&self, code: &Code) -> Option<u8> {
        match *code {
            Code::Literal(byte) => self.length[usize::from(byte)],
            Code::Reference(r) => {
                let run = r.run();
                let run_symbol = huffman::encode_run_length(run);
                let run_symbol_len = match self.length[usize(run_symbol)] {
                    Some(len) => len,
                    None => return None,
                };

                let (code, bit_count, _) = huffman::encode_distance(r.dist).unwrap();
                let distance_symbol_len = match self.distance[usize::from(code)] {
                    Some(len) => len,
                    None => return None,
                };

                Some(run_symbol_len + distance_symbol_len + bit_count)
            }
        }
    }
}

fn compressed_codes<W: Write>(
    into: &mut BitWriter<W>,
    length_tree: &CodeTree,
    distance_tree: Option<&CodeTree>,
    codes: &[Code],
) -> Result<(), Error> {
    let length_tree = length_tree.invert();
    let distance_tree = distance_tree.map(|tree| tree.invert());

    assert!(length_tree.len() > 256);

    use self::Code::*;

    for code in codes {
        match *code {
            Literal(byte) => {
                into.write_vec(
                    length_tree[usize(byte)]
                        .as_ref()
                        .ok_or_else(|| err_msg("invalid literal"))?,
                )?;
            }
            Reference(r) => {
                encode_run(into, &length_tree, r.run())?;
                encode_distance(into, distance_tree.as_ref(), r.dist)?;
            }
        }
    }

    // End of stream marker
    into.write_vec(length_tree[256].as_ref().unwrap())?;

    Ok(())
}

fn encode_run<W: Write>(
    into: &mut BitWriter<W>,
    length_tree: &[Option<BitVec>],
    run: u16,
) -> Result<(), Error> {
    into.write_vec(
        length_tree[huffman::encode_run_length(run) as usize]
            .as_ref()
            .unwrap(),
    )?;

    if let Some((bits, val)) = huffman::extra_run_length(run) {
        into.write_bits_val(bits, val)?;
    }

    Ok(())
}

fn encode_distance<W: Write>(
    into: &mut BitWriter<W>,
    tree: Option<&Vec<Option<BitVec>>>,
    dist: u16,
) -> Result<(), Error> {
    if let Some((code, bits, val)) = huffman::encode_distance(dist) {
        let distance_tree = tree
            .as_ref()
            .ok_or_else(|| err_msg("reference but not distance tree"))?;

        into.write_vec(distance_tree[usize(code)].as_ref().unwrap())?;

        if bits > 0 {
            into.write_bits_val(bits, val)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::parse;

    #[test]
    fn decompress() {
        let mut into = Cursor::new(vec![]);
        let mut reco = BitWriter::new(Cursor::new(vec![]));

        let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
        let mut raw = Cursor::new(
            &include_bytes!("../tests/data/libcgi-untaint-email-perl_0.03.orig.tar.gz")[37..],
        );

        {
            let mut it = parse::parse_deflate(&mut raw).peekable();

            loop {
                let block = match it.next() {
                    Some(block) => block.unwrap(),
                    None => break,
                };

                let last = it.peek().is_none();

                decompressed_block(&mut into, &mut dictionary, &block).unwrap();
                reco.write_bit(last).unwrap();
                compressed_block(&mut reco, &block).unwrap();
            }
            reco.align().unwrap();
        }
        let raw = raw.into_inner().to_vec();
        let reco: Vec<u8> = reco.into_inner().into_inner();
        assert_eq!(&raw[..raw.len() - 8], &reco[..]);

        assert_eq!(20480, into.into_inner().len());
    }
}
