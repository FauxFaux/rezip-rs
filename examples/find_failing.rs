#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate librezip;
extern crate rand;

use std::fs::File;
use std::io::Cursor;
use std::io::Write;

use rand::Rng;

use librezip::Result;

quick_main!(run);

fn run() -> Result<()> {
    let mut rng = rand::thread_rng();
    let mut n = 25;
    loop {
        let output = compressed_file(n, &mut rng)?;
        let block = match librezip::parse_deflate(Cursor::new(&output)).next() {
            Some(Ok(block)) => block,
            other => bail!("couldn't deflate: {:?}", other),
        };

        let codes = match block {
            librezip::Block::FixedHuffman(codes)
            | librezip::Block::DynamicHuffman { codes, .. } => codes,
            _ => continue,
        };

        if librezip::infer::max_distance(&codes).is_none() {
            // pure literals
            continue;
        }

        let result = librezip::infer::guess_settings(&[], &codes);
        if result.is_err() {
            if let Ok(mut f) = File::create(format!("brokey-{}.deflate", n)) {
                f.write_all(&output)?;
            } else {
                println!("beaten");
                n -= 1;
            }
            println!("found a failure at n={}: {:?}", n, result);
            n -= 1;
        }
    }
    Ok(())
}

fn compressed_file(n: usize, mut rng: &mut rand::ThreadRng) -> Result<Vec<u8>> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::with_capacity(n), flate2::Compression::Default);
    let input: Vec<u8> = (0..n).map(|_| random_printable(&mut rng)).collect();
    encoder.write(&input)?;
    Ok(encoder.finish()?)
}

fn random_printable(rng: &mut rand::ThreadRng) -> u8 {
    //    rng.gen_range(32, 127)
    rng.gen_range(b'a', b'z')
}
