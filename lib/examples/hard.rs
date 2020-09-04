extern crate rand;

use std::io;
use std::io::Write;

use rand::prelude::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

fn main() {
    let chars: Box<[u8]> = (32..128)
        .filter(|&c| (c as char).is_ascii_graphic())
        .collect::<Vec<u8>>()
        .into_boxed_slice();

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut rng = ChaCha20Rng::from_seed([0u8; 32]);

    loop {
        let mut buf = [0u8; 8];
        buf[buf.len() - 1] = b'\n';

        for k in 0..buf.len() - 1 {
            buf[k] = *chars.choose(&mut rng).expect("non-empty");
        }

        stdout.write_all(&buf).unwrap();
    }
}
