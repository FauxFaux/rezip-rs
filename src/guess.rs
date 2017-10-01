use std::collections::HashMap;
use std::fmt;

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
    codes.iter().enumerate().any(|(pos, code)| {
        if let Code::Reference { dist, .. } = *code {
            dist as usize >= pos // off-by-one?
        } else {
            false
        }
    })
}

fn single_block_encode(window_size: u16, codes: &[Code]) -> Result<()> {
    let mut coderator = serialise::DecompressedBytes::new(codes.iter())
        .enumerate()
        .peekable();
    let mut expected = codes.iter();
    let mut buf = CircularBuffer::with_capacity(32 * 1024 + 258 + 3);
    let mut map = HashMap::new();
    let mut key = Key::default();

    loop {
        let (pos, byte) = match coderator.next() {
            Some(x) => x,
            None => break,
        };

        key.push(byte);
        buf.append(byte);

        if pos < 2 {
            continue;
        }

        #[cfg(never)]
        println!("pos: {}, map: {:?}", pos, map);

        // the map tracks pointers to the *end* of where the block is,
        // as this removes a load of +1s and -2s from the code, not because
        // it's essentially very clear. I think.

        let old = match map.insert(key, pos) {
            Some(old) => old,
            None => {
                // we decided to emit a literal
                match expected.next() {
                    Some(&Code::Literal(_)) => continue,
                    Some(&Code::Reference { dist, run_minus_3 }) => {
                        bail!(
                            "we failed to spot the dist: {} run: {} backreference at {}",
                            dist,
                            run_minus_3 + 3,
                            pos
                        )
                    }
                    None => bail!("we think there's more codes at {} but there isn't", pos),
                }
            }
        };

        #[cfg(never)]
        println!(
            "think we've found a run, we're at {} and the old was at {}",
            pos,
            old
        );

        let dist = pos - old;

        if dist > (window_size as usize) {
            // TODO: off-by-one
            continue;
        }

        let dist = dist as u16;

        let mut run = 3;

        loop {
            if run >= 258 {
                assert_eq!(258, run);
                break;
            }

            let &(pos, byte) = coderator.peek().expect("TODO");

            #[cfg(never)]
            println!(
                "{:?} != {:?}",
                buf.get_at_dist(dist) as char,
                byte as char
            );

            if buf.get_at_dist(dist) != byte {
                break;
            }

            let (pos, byte) = coderator.next().expect("consuming peek'd value");

            key.push(byte);
            buf.append(byte);
            map.insert(key, pos);

            run += 1;
        }

        match expected.next() {
            Some(&Code::Reference {
                     dist: expected_dist,
                     run_minus_3,
                 }) => {
                let expected_run = u16::from(run_minus_3) + 3;
                if expected_dist != dist || expected_run != run {
                    bail!(
                        "we found a different run: ({}, {}) != ({}, {}) at {}",
                        expected_dist,
                        expected_run,
                        dist,
                        run,
                        pos
                    );
                }
            }
            Some(&Code::Literal(_)) => {
                bail!(
                    "we found a run ({}, {}) that the original encoder missed at {}",
                    dist,
                    run,
                    pos
                )
            }
            None => bail!("we tried to emit a run but the stream is finished"),
        }

    }

    return Ok(());
}

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
struct Key {
    vals: [u8; 3],
}

impl Key {
    fn push(&mut self, val: u8) {
        self.vals[0] = self.vals[1];
        self.vals[1] = self.vals[2];
        self.vals[2] = val;
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Key{{ {:?} {:?} {:?} 0x{:02x}{:02x}{:02x}}}",
            self.vals[0] as char,
            self.vals[1] as char,
            self.vals[2] as char,
            self.vals[0],
            self.vals[1],
            self.vals[2],
        )
    }
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
        let next_three = window[pos..pos + 3];
        window.windows(3).filter(|window| next_three == window);

        // this is dumb

        pos += 1;
    }

    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use parse;
    use Block;

    #[test]
    fn find_single_ref() {
        match parse::parse_deflate(Cursor::new(
            &include_bytes!("../tests/data/abcdef-bcdefg.gz")[10..],
        )).next() {
            Some(Ok(Block::FixedHuffman(codes))) => single_block_encode(32, &codes).unwrap(),
            _ => unreachable!(),
        }

    }
}

fn usize_from(val: u16) -> usize {
    val as usize
}
