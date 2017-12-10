#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate librezip;
extern crate rand;

use std::fs::File;
use std::io::Cursor;
use std::io::Write;

use rand::Rng;

use librezip::Block;
use librezip::Result;

quick_main!(run);

fn run() -> Result<()> {
    let mut rng = rand::thread_rng();
    let mut n = 25;
    loop {
        let (uncompressed, compressed) = compressed_file(n, &mut rng)?;
        let block = match librezip::parse_deflate(Cursor::new(&compressed)).next() {
            Some(Ok(block)) => block,
            other => bail!("couldn't deflate: {:?}", other),
        };

        let codes = match block {
            Block::FixedHuffman(codes)
            | Block::DynamicHuffman { codes, .. } => codes,
            Block::Uncompressed(_) => continue,
        };

        // TODO: pure literals?

        let slice = librezip::tracer::try_gzip(1, &[], &uncompressed, &codes);

        if slice.len() != 2 {
            if let Ok(mut f) = File::create(format!("brokey-{}.deflate", n)) {
                f.write_all(&compressed)?;
            } else {
                println!("beaten");
                n -= 1;
            }
            println!("found a failure at n={}: {:?} {:?}", n, slice.len(), slice);
            n -= 1;
        }
    }

    unreachable!()
}

fn compressed_file(n: usize, mut rng: &mut rand::ThreadRng) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::with_capacity(n), flate2::Compression::fast());
    let input: Vec<u8> = (0..n).map(|_| random_printable(&mut rng)).collect();
    encoder.write(&input)?;
    Ok((input, encoder.finish()?))
}

fn random_printable(rng: &mut rand::ThreadRng) -> u8 {
    //    rng.gen_range(32, 127)
    rng.gen_range(b'a', b'z')
}
