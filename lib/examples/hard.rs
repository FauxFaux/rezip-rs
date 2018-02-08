extern crate rand;

use std::io;
use std::io::Write;

use rand::IsaacRng;
use rand::Rng;
use rand::SeedableRng;

fn main() {
    let chars: Box<[u8]> = (32..128)
        .filter(|&c| (c as char).is_ascii_graphic())
        .collect::<Vec<u8>>()
        .into_boxed_slice();

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut rng = IsaacRng::from_seed(&[0]);

    loop {
        let mut buf = [0u8; 8];
        buf[buf.len() - 1] = b'\n';

        for k in 0..buf.len() - 1 {
            buf[k] = *rng.choose(&chars).unwrap();
        }

        stdout.write_all(&buf).unwrap();
    }
}
