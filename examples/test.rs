extern crate librezip;

use std::env;
use std::fs;
use std::io;

use std::io::Write;

fn main() {
    let input = env::args().nth(1).expect("first argument: input-path.gz");
    let results = librezip::process(
        io::BufReader::new(fs::File::open(input).expect("input readable")),
        NullWriter {},
    ).expect("processing");

    for result in results {
        println!("{:?}", result);
    }
}

struct NullWriter {}

impl Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        Ok(())
    }
}