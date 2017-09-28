#[macro_use]
extern crate error_chain;

extern crate itertools;

#[macro_use]
extern crate lazy_static;

extern crate sha2;

use std::io::Cursor;
use std::io::Read;
use std::io::Write;

mod bit;
mod circles;
mod code_tree;
mod errors;
mod filter;
mod gzip;
mod huffman;

use bit::BitReader;
use bit::BitVec;
use bit::BitWriter;

pub use huffman::SeenDistanceSymbols;

use circles::CircularBuffer;
use errors::*;

#[derive(Debug)]
pub struct Instructions {
    pub block_type: BlockType,
    pub len: usize,
}

pub struct Processed {
    pub header: Vec<u8>,
    pub instructions: Vec<Instructions>,
    pub tail: Vec<u8>,
    pub sha512_compressed: Vec<u8>,
    pub sha512_decompressed: Vec<u8>,
}

pub fn deconstruct<R: Read, W: Write>(from: R, into: W) -> Result<Processed> {
    let mut from = filter::FilterRead::new(from);
    let mut into = filter::FilterWrite::new(into);

    let header = gzip::discard_header(&mut from)?;

    let mut reader = BitReader::new(from);
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
        sha512_compressed: from.hash(),
        sha512_decompressed: into.hash(),
    })
}

pub fn reconstruct<R: Read, W: Write>(from: R, into: W, spec: &Processed) -> Result<()> {
    let mut from = filter::FilterRead::new(from);
    let mut into = filter::FilterWrite::new(into);

    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);

    into.write_all(&spec.header)?;

    let mut into = BitWriter::new(into);

    for (pos, op) in spec.instructions.iter().enumerate() {

        // final block marker
        into.write_bit(pos == spec.instructions.len() - 1)?;

        write_block(&mut from, &mut into, &mut dictionary, op)?;
    }

    assert!(from.read_exact(&mut [0u8; 1]).is_err());

    into.align()?;

    let mut into = into.into_inner();

    into.write_all(&spec.tail)?;

    ensure!(
        from.hash() == spec.sha512_decompressed,
        "source data hash mismatch"
    );
    ensure!(
        into.hash() == spec.sha512_compressed,
        "compressed data hash mismatch"
    );

    Ok(())
}

#[derive(Debug)]
pub enum BlockType {
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
    reader: &mut BitReader<R>,
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
    reader: &mut BitReader<R>,
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

fn write_block<R: Read, W: Write>(
    reader: R,
    writer: &mut BitWriter<W>,
    dictionary: &mut CircularBuffer,
    block: &Instructions,
) -> Result<()> {
    match block.block_type {
        BlockType::Uncompressed => {
            writer.write_bits_val(2, 0)?;
            unimplemented!();
        }
        BlockType::Fixed(ref seen) => {
            writer.write_bits_val(2, 1)?;
            huffman_block(
                reader,
                writer,
                dictionary,
                (
                    &huffman::FIXED_LENGTH_TREE,
                    Some(&huffman::FIXED_DISTANCE_TREE),
                ),
                seen,
            )?;
        }
        BlockType::Dynamic(ref tree, ref seen) => {
            writer.write_bits_val(2, 2)?;
            writer.write_vec(tree)?;
            let (length, distance) =
                huffman::read_codes(&mut BitReader::new(Cursor::new(bit::vec_to_bytes(tree))))?;

            huffman_block(
                reader,
                writer,
                dictionary,
                (&length, distance.as_ref()),
                seen,
            )?;
        }
    }

    Ok(())
}

fn huffman_block<R: Read, W: Write>(
    mut reader: R,
    writer: &mut BitWriter<W>,
    dictionary: &mut CircularBuffer,
    (length, distance): (&code_tree::CodeTree, Option<&code_tree::CodeTree>),
    seen: &huffman::SeenDistanceSymbols,
) -> Result<()> {
    let length = length.invert();

    for (chr, enc) in length.iter().enumerate() {
        if enc.is_some() {
            println!("{:?}: {:?}", chr as u8 as char, enc);
        }
    }

    for item in &seen.stream {
        write_literals(&mut reader, writer, dictionary, &length, item.literals)?;

        let mut run = vec![0u8; usize::from(item.run_minus_3) + 3];
        reader.read_exact(&mut run)?;

        let dist = dictionary.find_run(&run)? as u32;
        assert_eq!(dist, item.dist);

        writer.write_vec(&item.symbol)?;
    }

    write_literals(
        &mut reader,
        writer,
        dictionary,
        &length,
        seen.trailing_literals,
    )?;

    // end of block
    writer.write_vec(length[0x100].as_ref().unwrap())
}

fn write_literals<R: Read, W: Write>(
    mut reader: R,
    writer: &mut BitWriter<W>,
    dictionary: &mut CircularBuffer,
    length: &[Option<BitVec>],
    literals: usize,
) -> Result<()> {

    let mut buf = vec![0u8; literals];
    reader.read_exact(&mut buf)?;

    for byte in buf {
        dictionary.append(byte);
        writer.write_vec(length[usize::from(byte)].as_ref().expect(
            "valid code",
        ))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Cursor;
    use std::io::Write;
    use bit::BitVec;
    use ::*;

    #[test]
    fn seq_20() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            1,
            deconstruct(
                Cursor::new(&include_bytes!("../tests/data/seq-20.gz")[..]),
                &mut output,
            ).unwrap()
                .instructions
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
    fn seq_20_round_trip() {
        // no distance references at all, dynamic huffman
        round_trip(&include_bytes!("../tests/data/seq-20.gz")[..]);
    }

    #[test]
    fn lol_round_trip() {
        // fixed huffman, no backreferences
        round_trip(&include_bytes!("../tests/data/lol.gz")[..]);
    }

    #[test]
    fn like_love_round_trip() {
        // single true backreference in the middle, fixed huffman
        round_trip(&include_bytes!("../tests/data/like-love.gz")[..]);
    }

    #[test]
    fn simple_backreference_round_trip() {
        round_trip(&include_bytes!("../tests/data/abcdef-bcdefg.gz")[..]);
    }

    fn round_trip(orig: &[u8]) {
        let mut decompressed = Cursor::new(vec![]);

        let spec = deconstruct(Cursor::new(orig), &mut decompressed).expect("deconstruct");
        decompressed.set_position(0);

        let mut recompressed = Cursor::new(vec![]);
        let result = reconstruct(&mut decompressed, &mut recompressed, &spec);

        let reco = &recompressed.into_inner();

        result.expect("success");
    }

    #[test]
    fn stored_hunk() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            18,
            deconstruct(
                Cursor::new(
                    &include_bytes!("../tests/data/librole-basic-perl_0.13-1.debian.tar.gz")[..],
                ),
                &mut output,
            ).unwrap()
                .instructions
                .len()
        );
    }

    #[test]
    fn some_flags_set() {
        let mut output = Cursor::new(vec![]);

        assert_eq!(
            1, // TODO
            deconstruct(
                Cursor::new(
                    &include_bytes!("../tests/data/libcgi-untaint-email-perl_0.03.orig.tar.gz")[..],
                ),
                &mut output,
            ).unwrap()
                .instructions
                .len()
        );
    }
}
