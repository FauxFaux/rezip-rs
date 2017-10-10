use std::collections::HashMap;

use circles::CircularBuffer;
use errors::*;
use serialise;
use three::ThreePeek;
use unpack_run;

use Code;
use WindowSettings;

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

/// 1) checks if any code references before the start of this block
/// 2) checks if any code references the exact start of the block
pub fn outside_range_or_hit_zero(codes: &[Code]) -> (bool, bool) {
    let mut pos: u16 = 0;
    let mut hit_zero = false;

    for code in codes {
        if let Code::Reference { dist, .. } = *code {

            if dist == pos {
                hit_zero = true;
            }

            if dist > pos {
                return (true, hit_zero);
            }
        }

        // this can't overflow, as u16::MAX < 32_768 + max emitted_bytes
        pos = pos.checked_add(code.emitted_bytes()).unwrap();

        if pos > 32_768 {
            break;
        }
    }

    return (false, hit_zero);
}

pub fn guess_settings(mut preroll: &[u8], codes: &[Code]) -> Result<WindowSettings> {
    let window_size = max_distance(codes).unwrap();
    let (outside, hits_first_byte) = outside_range_or_hit_zero(codes);

    let config = WindowSettings {
        window_size,
        first_byte_bug: preroll.is_empty() && !hits_first_byte,
    };

    // optimisation
    if !outside {
        preroll = &[];
    }

    validate_reencode(&config, preroll, codes)?;

    return Ok(config);
}

pub fn validate_reencode(config: &WindowSettings, preroll: &[u8], codes: &[Code]) -> Result<()> {
    let mut expected = codes.iter();

    let mut seen = 0usize;

    attempt_reencoding(&config, preroll, codes, |code| {
        seen += 1;
        validate_expectation(seen, expected.next(), &code)
    })?;

    if expected.next().is_some() {
        bail!("we incorrectly gave up emitting codes after {}", seen);
    }

    Ok(())
}

fn validate_expectation(seen: usize, exp: Option<&Code>, code: &Code) -> Result<()> {
    use Code::*;

    match exp {
        Some(&Literal(expected_byte)) => validate_expected_literal(seen, expected_byte, &code),
        Some(&Reference {
                 dist: expected_dist,
                 run_minus_3,
             }) => validate_expected_range(seen, expected_dist, unpack_run(run_minus_3), &code),
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
            let run = unpack_run(run_minus_3);
            bail!(
                "{}: picked run ({}, {}) that the original encoder missed",
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
            let run = unpack_run(run_minus_3);
            if expected_dist != dist || expected_run != run {
                bail!(
                    "{}: we found a different run: them: ({}, {}) != us: ({}, {})",
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

fn attempt_reencoding<F>(
    config: &WindowSettings,
    preroll: &[u8],
    codes: &[Code],
    emit: F,
) -> Result<()>
where
    F: FnMut(Code) -> Result<()>,
{
    attempt_encoding(
        config,
        preroll.len(),
        serialise::DecompressedBytes::new(preroll, codes.iter()),
        emit,
    )
}

fn attempt_encoding<B, F>(
    config: &WindowSettings,
    preroll: usize,
    bytes: B,
    mut emit: F,
) -> Result<()>
where
    B: Iterator<Item = u8>,
    F: FnMut(Code) -> Result<()>,
{
    let mut bytes = ThreePeek::new(bytes);
    let mut buf = CircularBuffer::with_capacity(32 * 1024 + 258 + 3);
    let mut map: HashMap<(u8, u8, u8), usize> = HashMap::with_capacity(config.window_size as usize);

    let mut pos: usize = 0;

    loop {
        //println!("top: {}: ({}) {:?}", pos, buf.vec().len(), buf.vec());

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

        let old = if 0 != pos || !config.first_byte_bug {
            map.insert(key, pos)
        } else {
            None
        };

        pos += 1;

        if pos <= preroll {
            continue;
        }

        if old.is_none() {
            emit(Code::Literal(key.0))?;
            continue;
        }

        let old = old.unwrap();

        //println!("think we've found a run, we're at {} and the old was at {}", pos, old);

        let dist = pos - old - 1;

        if dist > (config.window_size as usize) {
            continue;
        }

        let dist = dist as u16;

        let mut run = 0u16;

        loop {
            if run >= 257 {
                assert_eq!(257, run);
                break;
            }

            let byte = match bytes.peek() {
                Some(byte) => byte,
                None => break,
            };

            //println!("inside: {}: ({}) {:?} {:?}", pos, buf.vec().len(), buf.vec(), map);
            //println!("{:?} != {:?}", buf.get_at_dist(dist) as char, byte as char);

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

        run += 1;

        emit(Code::Reference {
            dist,
            run_minus_3: ::pack_run(run),
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::attempt_reencoding;
    use super::guess_settings;
    use super::max_distance;
    use super::outside_range_or_hit_zero;
    use super::Code;
    use super::WindowSettings;

    use Code::Literal as L;
    use Code::Reference as R;

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
        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn two_length_three_runs() {
        let exp = &[
            L(b'a'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'e'),
            L(b'f'),
            L(b'g'),
            L(b'h'),
            L(b'7'),
            L(b'8'),
            L(b'9'),
            L(b'i'),
            L(b'j'),
            L(b'k'),
            L(b'l'),
            R {
                dist: 14,
                run_minus_3: 0,
            },
            L(b'm'),
            L(b'n'),
            L(b'o'),
            L(b'p'),
            R {
                dist: 14,
                run_minus_3: 0,
            },
            L(b'q'),
            L(b'r'),
            L(b's'),
            L(b't'),
        ];
        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn two_overlapping_runs() {
        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            R {
                dist: 6,
                run_minus_3: 0,
            },
            L(b'4'),
            L(b'5'),
            L(b'e'),
            L(b'f'),
            R {
                dist: 5,
                run_minus_3: 0,
            },
            L(b'g'),
        ];
        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
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
        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn ref_before() {
        let exp = &[
            R {
                dist: 1,
                run_minus_3: 10,
            },
        ];
        assert_eq!(exp, decode_then_reencode(&[0], exp).as_slice());
    }

    #[test]
    fn just_long_run() {
        let exp = &[
            L(5),
            R {
                dist: 1,
                run_minus_3: ::pack_run(258),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn two_long_run() {
        let exp = &[
            L(5),
            R {
                dist: 1,
                run_minus_3: ::pack_run(258),
            },
            R {
                dist: 1,
                run_minus_3: ::pack_run(258),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }


    #[test]
    fn many_long_run() {
        let enough_to_wrap_around = 10 + (32 * 1024 / 258);

        let mut exp = Vec::with_capacity(enough_to_wrap_around + 1);
        exp.push(L(5));
        for _ in 0..enough_to_wrap_around {
            exp.push(R {
                dist: 1,
                run_minus_3: ::pack_run(258)
            });
        }

        assert_eq!(exp, decode_then_reencode_single_block(&exp));
    }

    #[test]
    fn range() {
        assert_eq!((false, false), outside_range_or_hit_zero(&[L(5)]));

        assert_eq!(
            (true, false),
            outside_range_or_hit_zero(
                &[
                    R {
                        dist: 1,
                        run_minus_3: 3,
                    },
                ],
            )
        );

        assert_eq!(
            (false, true),
            outside_range_or_hit_zero(
                &[
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 3,
                    },
                ],
            )
        );

        assert_eq!(
            (false, false),
            outside_range_or_hit_zero(
                &[
                    L(5),
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 3,
                    },
                ],
            )
        );

        // Not an encoding a real tool would generate
        assert_eq!(
            (false, true),
            outside_range_or_hit_zero(
                &[
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 20,
                    },
                    R {
                        dist: 15,
                        run_minus_3: 3,
                    },
                ],
            )
        );

        assert_eq!(
            (true, true),
            outside_range_or_hit_zero(
                &[
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 4,
                    },
                    R {
                        dist: 15,
                        run_minus_3: 3,
                    },
                ],
            )
        );
    }

    #[test]
    fn guess_first_byte_bug() {
        assert_eq!(
            WindowSettings {
                window_size: 1,
                first_byte_bug: true,
            },
            guess_settings(
                &[],
                &[
                    L(5),
                    L(5),
                    R {
                        dist: 1,
                        run_minus_3: 5,
                    },
                ],
            ).unwrap()
        );
    }

    fn decode_then_reencode_single_block(codes: &[Code]) -> Vec<Code> {
        decode_then_reencode(&[], codes)
    }

    fn decode_then_reencode(preroll: &[u8], codes: &[Code]) -> Vec<Code> {
        use WindowSettings;

        let window_size = max_distance(codes).unwrap();
        let mut ret = Vec::with_capacity(codes.len());
        let config = WindowSettings {
            window_size,
            first_byte_bug: false,
        };

        attempt_reencoding(&config, preroll, codes, |code| {
            ret.push(code);
            Ok(())
        }).expect("fails only if closure fails");

        ret
    }
}
