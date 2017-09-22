extern crate librezip;

use std::env;
use std::fs;
use std::io;

fn main() {
    let input = env::args().nth(1).expect("first argument: input-path.gz");
    let output = env::args().nth(2).expect("second argument: output-path");
    let results = librezip::process(
        io::BufReader::new(fs::File::open(input).expect("input readable")),
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .expect("output creatable"),
    ).expect("processing");

    println!(" input: {}", hexify(&results.sha512_compressed[0..256/8]));
    println!("output: {}", hexify(&results.sha512_decompressed[0..256/8]));

    println!("header: {}", hex_and_ascii(&results.header));

    println!("frames:");

    for result in results.instructions {
        println!(" - {:?}", result);
    }

    println!("footer: {:?}", hex_and_ascii(&results.tail));
}

fn hexify(buf: &[u8]) -> String {
    let mut ret = String::with_capacity(buf.len() * 2);
    for c in buf {
        ret.push_str(&format!("{:02x}", c));
    }

    ret
}

fn hex_and_ascii(buf: &[u8]) -> String {
    let mut hex = String::with_capacity(buf.len() * 2);
    let mut ascii = hex.clone();

    for c in buf {
        hex.push_str(&format!("{:02x} ", c));
        ascii.push(match *c as char {
            c if c >= ' ' && c <= '~' => c as char,
            _ => '.'
        });
    }

    format!("{}    {}", hex, ascii)
}

