#[macro_use]
extern crate error_chain;
extern crate librezip;

use std::env;
use std::fs;
use std::io;

use librezip::all_refs::AllRefs;
use librezip::serialise_trace;
use librezip::trace;
use librezip::Block;
use librezip::CircularBuffer;
use librezip::Code;
use librezip::Config;
use librezip::Guesser;
use librezip::Result;
use librezip::Trace;

quick_main!(run);

fn run() -> Result<()> {
    let input = env::args().nth(1).ok_or("first argument: input-path.gz")?;
    let mut reader = io::BufReader::new(fs::File::open(input)?);
    librezip::gzip::discard_header(&mut reader)?;

    let mut dictionary = CircularBuffer::new();

    for (id, block) in librezip::parse_deflate(&mut reader).into_iter().enumerate() {
        let block = block?;

        println!("block {}:", id);
        use self::Block::*;
        match block {
            Uncompressed(data) => {
                println!(" - uncompressed: {} bytes", data.len());
                dictionary.extend(&data);
            }
            FixedHuffman(codes) => {
                println!(" - fixed huffman:");
                print(&mut dictionary, &codes)?;
            }
            DynamicHuffman { trees, codes } => {
                println!(" - dynamic huffman: {:?}", trees);
                print(&mut dictionary, &codes)?;
            }
        }
    }

    Ok(())
}

fn print(dictionary: &mut CircularBuffer, codes: &[Code]) -> Result<()> {
    let old_dictionary = &dictionary.vec();

    let mut decompressed: Vec<u8> = Vec::with_capacity(codes.len());
    librezip::decompressed_codes(&mut decompressed, dictionary, codes)?;

    if false {
        print!("   * codes: ");
        for c in codes {
            match *c {
                Code::Literal(byte) if char::from(byte).is_alphanumeric() => {
                    print!("{}", char::from(byte))
                }
                Code::Literal(byte) => print!("{:?}", char::from(byte)),
                Code::Reference(r) => print!(" -- {:?} -- ", r),
            }
        }
        println!();
    }

    let mut refs_1 = AllRefs::with_sixteen(old_dictionary, &decompressed, 4);
    let mut refs_3 = AllRefs::with_sixteen(old_dictionary, &decompressed, 6);
    let mut all_refs = AllRefs::with_sixteen(old_dictionary, &decompressed, ::std::u16::MAX);

    // TODO: all_refs.apply_first_byte_bug_rule();

    if false {
        println!("refs_1:\n{:?}", refs_1);
        println!("refs_3:\n{:?}", refs_3);
        println!("refs_all:\n{:?}", all_refs);
    }

    if true {
        try_trace(
            &refs_1,
            "gzip --fast",
            Config::gzip(1),
            codes,
            &decompressed,
        );
    }

    if false {
        try_trace(&refs_3, "gzip -3", Config::gzip(3), codes, &decompressed);
    }

    if true {
        // TODO: gzip --default doesn't actually use all-refs, but it's close enough for our purpose
        // TODO: I HOPE.
        try_trace(
            &all_refs,
            "gzip [--default]",
            Config::gzip_16_default(),
            codes,
            &decompressed,
        );
    }

    if false {
        try_trace(
            &all_refs,
            "gzip --best",
            Config::gzip(9),
            codes,
            &decompressed,
        );
    }

    Ok(())
}

fn try_trace(all_refs: &AllRefs, name: &str, config: Config, codes: &[Code], decompressed: &[u8]) {
    let technique = librezip::Technique::new(config, all_refs);
    let trace = trace::validate(codes, &technique);
    let serialise = serialise_trace::verify(&trace);
    println!("   * trace: {} -> {}", name, serialise.len());
    let mut scanner = technique.scanner();

    for (t, c) in trace.iter().zip(codes.iter()) {
        let location_hint = String::from_utf8_lossy(
            &decompressed[scanner.pos.saturating_sub(5)..(scanner.pos + 5).min(decompressed.len())],
        );

        match *t {
            Trace::Correct => {}
            Trace::ActuallyLiteral => println!(
                "   {:4}. {:10?} guess: {:?} trace: literal",
                scanner.pos,
                location_hint,
                scanner.codes(),
            ),
            Trace::Actually(correct) => println!(
                "   {:4}. {:10?} guess: {:?} trace: {:?}",
                scanner.pos,
                location_hint,
                scanner.codes(),
                correct
            ),
        }

        scanner.feedback(*c);
    }

    println!("   * guesser: {:?}", scanner);

    println!();
}
