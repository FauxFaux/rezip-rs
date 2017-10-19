use std::cmp;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use itertools::Itertools;

use circles::CircularBuffer;
use errors::*;
use serialise;
use three::ThreePeek;
use unpack_run;
use usize_from;

use Code;
use WindowSettings;

type Key = (u8, u8, u8);
type BackMap = HashMap<Key, Vec<usize>>;

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
    let mut map: BackMap = HashMap::with_capacity(config.window_size as usize);

    let mut pos: usize = 0;

    loop {

        #[cfg(feature = "tracing")]
        fn lit(val: u8) -> String {
            format!("0x{:02x} {:?}", val, val as char)
        }

        #[cfg(feature = "tracing")]
        fn lit_key(key: Key) -> String {
            format!("({}, {}, {})", lit(key.0), lit(key.1), lit(key.2))
        }

        #[cfg(feature = "tracing")]
        {
            use itertools::Itertools;
            println!(
                "\n{}: top: ({}) [{}]",
                pos,
                buf.vec().len(),
                buf.vec().into_iter().map(|val| lit(val)).join(", ")
            );
        }

        let key: Key = match bytes.next_three() {
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

        let mut prev_poses = if 0 != pos || !config.first_byte_bug {
            write_map_key(key, pos, &mut map, &config)
        } else {
            None
        };

        if pos < preroll {
            pos += 1;
            continue;
        }

        #[cfg(feature = "tracing")]
        {
            println!(
                "{}: key: {}, prev_poses: {:?}",
                pos,
                lit_key(key),
                prev_poses
            );
            println!(
                "{}: map: {}",
                pos,
                map.iter()
                    .map(|(key, v)| format!("{} -> {:?}", lit_key(*key), v))
                    .join(", ")
            );
        }

        if prev_poses
            .as_mut()
            .map(|candidates| candidates.is_empty())
            .unwrap_or(true)
        {
            emit(Code::Literal(key.0))?;
            pos += 1;

            continue;
        }

        let (candidates, run) = track_run(
            prev_poses.unwrap(),
            pos,
            &mut bytes,
            &mut buf,
            &mut map,
            config,
        )?;

        println!("{}: consumed: {} {:?}", pos, run, candidates);

        let candidate = best_candidate(&candidates);

        assert_eq!(pos, candidate.run_start);

        let dist = (pos - candidate.data_pos) as u16;

        emit(Code::Reference {
            dist,
            run_minus_3: ::pack_run(run),
        })?;

        pos += usize_from(run);
    }
}

fn best_candidate(candidates: &[Prev]) -> &Prev {
    candidates.into_iter().min().unwrap()
}

fn write_map_key(
    key: Key,
    pos: usize,
    map: &mut BackMap,
    config: &WindowSettings,
) -> Option<Vec<usize>> {
    match map.entry(key) {
        Entry::Occupied(mut entry) => {
            let current = entry.get_mut();
            current.retain(|old| pos - old <= config.window_size as usize);
            let old = current.clone();
            current.push(pos);
            Some(old)
        }
        Entry::Vacant(entry) => {
            entry.insert(vec![pos]);
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq)]
struct Prev {
    data_pos: usize,
    run_start: usize,
}

impl cmp::Ord for Prev {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // lower run_start: longer run from where we found it -> where we are now.
        // as: run length is current - run_start

        // higher data_pos: shorter dist from where we found it -> where we are now.
        // as: dist is current - data_pos
        self.run_start.cmp(&other.run_start).reverse().then(self.data_pos.cmp(&other.data_pos))
    }
}

fn track_run<B>(
    prev_poses: Vec<usize>,
    original_start: usize,
    bytes: &mut ThreePeek<B>,
    buf: &mut CircularBuffer,
    map: &mut BackMap,
    config: &WindowSettings,
) -> Result<(Vec<Prev>, u16)>
where
    B: Iterator<Item = u8>,
{
    assert!(!prev_poses.is_empty());

    let mut run = 0u16;

    // we're already tracking everything, how much worse could it be?

    let mut prev_poses: Vec<Prev> = prev_poses
        .into_iter()
        .map(|data_pos| {
            Prev {
                data_pos,
                run_start: original_start,
            }
        })
        .collect();

    loop {
        run += 1;

        if run >= 258 {
            assert_eq!(258, run);
            return Ok((prev_poses, run));
        }

        let byte = match bytes.peek() {
            Some(byte) => byte,
            None => return Ok((prev_poses, run)),
        };

        #[cfg(feature = "tracing")]
        println!("inside: ({}) {:?} {:?}", buf.vec().len(), buf.vec(), map);

        let old_prev_poses = prev_poses.clone();

        prev_poses.retain(|candidate| {
            let dist = (candidate.run_start - candidate.data_pos) as u16;

            #[cfg(feature = "tracing")]
            println!(
                "{:?}: {:?} != {:?}",
                candidate,
                buf.get_at_dist(dist) as char,
                byte as char
            );

            buf.get_at_dist(dist) == byte
        });

        if prev_poses.is_empty() {
            #[cfg(feature = "tracing")]
            println!("no matches remain");
            return Ok((old_prev_poses, run));
        }

        match bytes.next_three() {
            Some(key) => {
                buf.push(key.0);
                let pos = original_start + usize_from(run);
                if let Some(new_prev_poses) = write_map_key(key, pos, map, &config) {
                    prev_poses.extend(new_prev_poses.into_iter().map(|data_pos| {
                        Prev {
                            data_pos,
                            run_start: original_start + usize_from(run),
                        }
                    }));
                }
            }
            None => {
                match bytes.next() {
                    Some(byte) => buf.push(byte),
                    None => return Ok((prev_poses, run)),
                }
            }
        }
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
    fn single_backref_abcdef_abcdef_ghi() {
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
        const ENOUGH_TO_WRAP_AROUND: usize = 10 + (32 * 1024 / 258);

        let mut exp = Vec::with_capacity(ENOUGH_TO_WRAP_AROUND + 1);

        exp.push(L(5));

        exp.extend(vec![
            R {
                dist: 1,
                run_minus_3: ::pack_run(258),
            };
            ENOUGH_TO_WRAP_AROUND
        ]);

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

    #[test]
    fn longer_match() {
        // I didn't think it would, but even:
        // echo a12341231234 | gzip --fast | cargo run --example dump /dev/stdin
        // ..generates this.

        // I was expecting it to only use the most recent hit for that hash item. Um.

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'4'),
            R {
                dist: 4,
                run_minus_3: ::pack_run(3),
            },
            R {
                dist: 7,
                run_minus_3: ::pack_run(4),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
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


    #[test]
    fn short_repeat() {
        // a122b122222
        // 01234567890

        let exp = &[
            L(b'a'),
            R {
                dist: 1,
                run_minus_3: ::pack_run(3),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn repeat_after_ref_a122b_122_222() {
        // a122b122222
        // 01234567890

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'2'),
            L(b'b'),
            R {
                dist: 4,
                run_minus_3: ::pack_run(3),
            },
            R {
                dist: 1,
                run_minus_3: ::pack_run(3),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn lazy_longer_ref() {
        // Finally, a test for this gzip behaviour.
        // It only does this with zip levels >3, including the default.

        // a123412f41234
        // 0123456789012

        // It gets to position 8, and it's ignoring the "412" (at position 6),
        // instead taking the longer run of "1234" at position 1.

        // I bet it thinks it's so smart.

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'4'),
            L(b'1'),
            L(b'2'),
            L(b'f'),
            L(b'4'),
            R {
                dist: 8,
                run_minus_3: ::pack_run(4),
            },
        ];

        assert_eq!(exp, decode_then_reencode_single_block(exp).as_slice());
    }
}
