use std::collections::HashMap;

use circles::CircularBuffer;
use errors::*;
use serialise;
use three::ThreePeek;

use Code;

pub fn max_distance(codes: &[Code]) -> Option<u16> {
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
pub fn outside_range(codes: &[Code]) -> bool {
    // TODO: this is totally wrong; pos != byte number
    codes.iter().enumerate().any(|(pos, code)| {
        if let Code::Reference { dist, .. } = *code {
            dist as usize > pos // off-by-one?
        } else {
            false
        }
    })
}

fn single_block_mem(window_size: u16, codes: &[Code]) -> Vec<Code> {
    block_mem(window_size, &[], codes)
}

fn block_mem(window_size: u16, preroll: &[u8], codes: &[Code]) -> Vec<Code> {
    let mut ret = Vec::with_capacity(codes.len());
    block_encode_helper(
        window_size,
        preroll.len(),
        serialise::DecompressedBytes::new(preroll, codes.iter()),
        |code| {
            ret.push(code);
            Ok(())
        },
    ).expect("fails only if closure fails");

    ret
}

pub fn single_block_encode(window_size: u16, codes: &[Code]) -> Result<()> {
    block_encode(window_size, &[], codes)
}

pub fn block_encode(window_size: u16, preroll: &[u8], codes: &[Code]) -> Result<()> {
    let mut expected = codes.iter();

    let mut seen = 0usize;

    block_encode_helper(
        window_size,
        preroll.len(),
        serialise::DecompressedBytes::new(preroll, codes.iter()),
        |code| {
            seen += 1;
            validate_expectation(seen, expected.next(), &code)
        },
    )?;

    ensure!(
        seen == codes.len(),
        "wrong number of codes were emitted, expected: {} != {}",
        codes.len(),
        seen
    );

    Ok(())
}

fn validate_expectation(seen: usize, exp: Option<&Code>, code: &Code) -> Result<()> {
    use Code::*;

    match exp {
        Some(&Literal(expected_byte)) => validate_expected_literal(seen, expected_byte, &code),
        Some(&Reference {
                 dist: expected_dist,
                 run_minus_3,
             }) => validate_expected_range(seen, expected_dist, u16::from(run_minus_3) + 3, &code),
        None => {
            bail!(
                "{}: we emitted a code that isn't supposed to be there",
                seen
            )
        }
    }
}

fn validate_expected_literal(seen: usize, expected_byte: u8, code: &Code) -> Result<()> {
    use Code::*;

    match *code {
        Literal(byte) => {
            ensure!(
                expected_byte == byte,
                "{}: wrong literal, 0x{:02x} != 0x{:02x} ({:?} != {:?})",
                seen,
                expected_byte,
                byte,
                expected_byte as char,
                byte as char,
            );
            Ok(())
        }
        Reference { dist, run_minus_3 } => {
            let run = u16::from(run_minus_3) + 3;
            bail!(
                "{}: picked reference ({}, {}) that the original encoder missed",
                seen,
                dist,
                run
            )
        }
    }
}

fn validate_expected_range(
    seen: usize,
    expected_dist: u16,
    expected_run: u16,
    code: &Code,
) -> Result<()> {
    use Code::*;

    match *code {
        Literal(byte) => {
            bail!(
                "{}: failed to spot ({}, {}) backreference, wrote a 0x{:02x} literal instead",
                seen,
                expected_dist,
                expected_run,
                byte
            )
        }
        Reference { dist, run_minus_3 } => {
            let run = u16::from(run_minus_3) + 3;
            if expected_dist != dist || expected_run != run {
                bail!(
                    "{}: we found a different run: ({}, {}) != ({}, {})",
                    seen,
                    expected_dist,
                    expected_run,
                    dist,
                    run,
                );
            }
            Ok(())
        }
    }
}

fn block_encode_helper<B: Iterator<Item = u8>, F>(
    window_size: u16,
    preroll: usize,
    bytes: B,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(Code) -> Result<()>,
{
    let mut bytes = ThreePeek::new(bytes);
    let mut buf = CircularBuffer::with_capacity(32 * 1024 + 258 + 3);
    let mut map = HashMap::new();

    let mut remaining_preroll = preroll;
    let mut pos = 0usize;

    loop {
        println!(".");

        let key = match bytes.next_three() {
            Some(x) => x,
            None => {
                // drain the last few bytes as literals
                for byte in bytes {
                    emit(Code::Literal(byte))?;
                }
                return Ok(());
            }
        };

        buf.push(key.0);

        let old = map.insert(key, pos);

        if remaining_preroll > 0 {
            remaining_preroll -= 1;

            pos += 1;
            continue;
        }

        if old.is_none() {
            emit(Code::Literal(key.0))?;

            pos += 1;
            continue;
        }

        let old = old.unwrap();

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

        let mut run = 1u16;

        loop {
            if run >= 258 {
                assert_eq!(258, run);
                break;
            }

            let byte = match bytes.peek() {
                Some(byte) => byte,
                None => break,
            };

            //            #[cfg(never)]
            println!("{:?} != {:?}", buf.get_at_dist(dist) as char, byte as char);

            if buf.get_at_dist(dist) != byte {
                break;
            }

            match bytes.next_three() {
                Some(key) => {
                    buf.push(key.0);
                    map.insert(key, pos);
                }
                None => {
                    match bytes.next() {
                        Some(byte) => buf.push(byte),
                        None => break,
                    }
                }
            }

            pos += 1;

            run += 1;
        }

        emit(Code::Reference {
            dist,
            run_minus_3: (run - 3) as u8,
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use parse;
    use Block;

    use Code::Literal as L;
    use Code::Reference as R;

    #[test]
    fn find_single_ref_from_file() {
        match parse::parse_deflate(Cursor::new(
            &include_bytes!("../tests/data/abcdef-bcdefg.gz")[10..],
        )).next() {
            Some(Ok(Block::FixedHuffman(codes))) => single_block_encode(32, &codes).unwrap(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn find_single_lits() {
        let exp = &[
            L(b'a'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            L(b'e'),
            L(b'f'),
            L(b' '),
            R {
                dist: 6,
                run_minus_3: 2,
            },
            L(b'g'),
            L(b'h'),
            L(b'i'),
        ];
        assert_eq!(exp, single_block_mem(32, exp).as_slice());
    }

    #[test]
    fn zero_run() {
        let exp = &[
            L(b'0'),
            R {
                dist: 1,
                run_minus_3: 10,
            },
        ];
        assert_eq!(exp, single_block_mem(3, exp).as_slice());
    }

    #[test]
    fn ref_before() {
        let exp = &[
            R {
                dist: 1,
                run_minus_3: 10,
            },
        ];
        assert_eq!(exp, block_mem(3, &[0], exp).as_slice());
    }

    #[test]
    fn range() {
        assert!(!outside_range(&[L(5)]));
        assert!(outside_range(
            &[
                R {
                    dist: 1,
                    run_minus_3: 3,
                },
            ],
        ));
        assert!(!outside_range(
            &[
                L(5),
                R {
                    dist: 1,
                    run_minus_3: 3,
                },
            ],
        ));
    }
}
