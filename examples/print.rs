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

    for result in results {
        println!("{:?}", result);
    }
}
