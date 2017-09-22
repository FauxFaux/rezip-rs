extern crate bit_vec;

#[macro_use]
extern crate error_chain;

extern crate itertools;

#[macro_use]
extern crate lazy_static;

use std::io::Cursor;
use std::io::Read;
use std::io::Write;

use bit_vec::BitVec;

mod bit;
mod circles;
mod code_tree;
mod errors;
mod gzip;
mod huffman;

use circles::CircularBuffer;
use errors::*;

#[derive(Debug)]
pub struct Instructions {
    block_type: BlockType,
    len: usize,
}

pub struct Processed {
    pub header: Vec<u8>,
    pub instructions: Vec<Instructions>,
    pub tail: Vec<u8>,
}

pub fn process<R: Read, W: Write>(mut from: R, mut into: W) -> Result<Processed> {
    let header = gzip::discard_header(&mut from)?;

    let mut reader = bit::BitReader::new(from);
    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);

    let mut instructions = vec![];

    loop {
        let BlockDone {
            final_block,
            data,
            block_type,
        } = read_block(&mut reader, &mut dictionary)?;

        instructions.push(Instructions {
            block_type,
            len: data.len(),
        });

        into.write_all(&data)?;

        if final_block {
            break;
        }
    }

    reader.align()?;

    let mut from = reader.into_inner();
    let mut tail = vec![];
    from.read_to_end(&mut tail)?;

    Ok(Processed {
        header,
        instructions,
        tail,
    })
}

#[derive(Debug)]
enum BlockType {
    Uncompressed,
    Fixed(huffman::SeenDistanceSymbols),
    Dynamic(BitVec, huffman::SeenDistanceSymbols),
}

struct BlockDone {
    final_block: bool,
    block_type: BlockType,
    data: Vec<u8>,
}

fn read_block<R: Read>(
    reader: &mut bit::BitReader<R>,
    dictionary: &mut CircularBuffer,
) -> Result<BlockDone> {
    let final_block = reader.read_bit()?;
    let mut writer = Cursor::new(vec![]);

    let block_type;

    match reader.read_part(2)? {
        0 => {
            read_uncompressed(reader, &mut writer, dictionary)?;
            block_type = BlockType::Uncompressed;
        }
        1 => {
            let symbols = huffman::read_data(
                reader,
                &mut writer,
                dictionary,
                &huffman::FIXED_LENGTH_TREE,
                Some(&huffman::FIXED_DISTANCE_TREE),
            )?;
            block_type = BlockType::Fixed(symbols);
        }
        2 => {
            reader.tracking_start();
            let (length, distance) = huffman::read_codes(reader)?;
            let tree = reader.tracking_finish();
            let symbols =
                huffman::read_data(reader, &mut writer, dictionary, &length, distance.as_ref())?;
            block_type = BlockType::Dynamic(tree, symbols);
        }
        3 => bail!("reserved block type"),
        _ => unreachable!(),
    }

    Ok(BlockDone {
        final_block,
        block_type,
        data: writer.into_inner(),
    })
}

fn read_uncompressed<R: Read, W: Write>(
    reader: &mut bit::BitReader<R>,
    mut output: W,
    dictionary: &mut CircularBuffer,
) -> Result<()> {
    reader.align()?;

    let buf = reader.read_length_prefixed()?;

    output.write_all(&buf)?;

    for byte in buf {
        dictionary.append(byte);
    }

    Ok(())
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
            ).unwrap().instructions
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
            ).unwrap().instructions
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
            ).unwrap().instructions
                .len()
        );
    }
}
