use circles::CircularBuffer;
use errors::*;
use serialise;

use Code;

pub fn guess_huffman(codes: &[Code]) {
    println!("{:?}", max_distance(codes))
}

fn max_distance(codes: &[Code]) -> Option<u16> {
    codes
        .iter()
        .flat_map(|code| if let Code::Reference { dist, .. } = *code {
            Some(dist)
        } else {
            None
        })
        .max()
}

/// checks if any code references before the start of this block
fn outside_range(codes: &[Code]) -> bool {
    codes
        .iter()
        .enumerate()
        .any(|(pos, code)| if let Code::Reference { dist, .. } = *code {
            dist as usize >= pos // off-by-one?
        } else {
            false
        })
}

#[cfg(never)]
fn search(window_size: u16, old_data: &[u8], codes: &[Code]) -> Result<Option<(u16, u16)>> {

    let data = {
        use std::io::Cursor;
        use std::io::Write;

        let mut data = Cursor::new(Vec::with_capacity(old_data.len() + codes.len()));
        data.write_all(old_data)?;

        let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
        serialise::decompressed_codes(&mut data, &mut dictionary, codes)?;
        data.into_inner()
    };

    let run_max = 256 + 3;

    let start = old_data.len();

    let mut pos = 0;
    while start + pos < data.len() {
        let window = &data[start.saturating_sub(usize_from(window_size))..start + pos];
        let next_three = window[pos..pos+3];
        window.windows(3).filter(|window| next_three == window);

            // this is dumb

        pos += 1;
    }

    unimplemented!()
}

fn usize_from(val: u16) -> usize {
    val as usize
}
