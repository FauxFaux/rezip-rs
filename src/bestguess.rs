/// Right, time to try the trace-like algorithm again.
/// The most likely cases:
/// 1. There are no runs available:
///    * a literal is coming out; encode this as the empty string
/// 2. There's a run, but we can find a longer run in the near future. (this is what gzip does)
///    * a literal, 'cos we're going to use the longer run
///    * the encoded runs
/// 3. There's a length-3 run, which we can insert, before a longer run is detected
///    * the short run
///    * a literal (gzip behaviour)
///    * encoded runs
/// 4. There's a run, and we can't immediately see longer runs in the future.
///    * encoded runs, which will be:
///      1. the longest run at the shortest distance
///      2. a literal, in case the compressor is dumb (if we're not in one of the other modes)
///      3. the longest run at the next shortest distance
///      4. ...
///      5. the next longest run, at the shortest distance,
///      5. the next longest run, at the next shortest distance,
///      6. ..
///
///    114. the longest run, but terminating early by 1
///    115. the longest run, but terminating early by 2
///    115. the longest run, but terminating early by ..
///    116. the longest run, but at length 3
///    117. the longest run, but at the next furthest distance?
///    118. the longest run, but at the next furthest distance, but terminating early by 1
/// ..
///


use result::ResultOptionExt;

use errors::*;
use guesser::Ref;
use guesser::RefGuesser;

use usize_from;
use Code;


fn sorted_candidates<I: Iterator<Item = Ref>>(candidates: I) -> Vec<Ref> {
    let mut us: Vec<Ref> = candidates.collect();

    us.sort_by(|&(ld, lr), &(rd, rr)| rr.cmp(&lr).then(ld.cmp(&rd)));

    us
}

fn find_reference_score<I: Iterator<Item = Ref>>(
    actual_dist: u16,
    actual_run_minus_3: u8,
    candidates: I,
) -> Result<usize> {
    if 255 == actual_run_minus_3 && 1 == actual_dist {
        return Ok(0);
    }

    let cand = sorted_candidates(candidates);

    Ok(match cand.iter()
        .position(|&(dist, run_minus_3)| {
            actual_run_minus_3 == run_minus_3 && actual_dist == dist
        })
        .ok_or_else(|| {
            format!(
                "it must be there? {:?} {:?}",
                (actual_dist, actual_run_minus_3),
                cand
            )
        })? {
        0 => 0,
        other => {
            // we guessed incorrectly, so we let the literal have the next position,
            // and everything shifts up
            1 + other
        }
    })
}

fn reduce_code<I: Iterator<Item = Ref>>(orig: &Code, mut candidates: I) -> Result<Option<usize>> {
    Ok(match *orig {
        Code::Literal(_) => {
            if candidates.next().is_none() {
                //There are no runs, so a literal is the only, obvious choice
                None
            } else {
                // There's a run available, and we've decided not to pick it; unusual
                Some(1)
            }
        }

        Code::Reference { dist, run_minus_3 } => {
            Some(find_reference_score(dist, run_minus_3, candidates)?)
        }
    })
}

pub fn reduce_entropy(preroll: &[u8], data: &[u8], codes: &[Code]) -> Result<Vec<usize>> {
    let guesser = RefGuesser::new(preroll, data);

    let mut pos = 0usize;

    codes
        .into_iter()
        .flat_map(|orig| {
            let guesser = guesser.at(pos);
            let reduced = guesser
                .all_candidates()
                .and_then(|candidates| reduce_code(orig, candidates).invert());

            pos += usize_from(orig.emitted_bytes());

            reduced
        })
        .collect()
}

fn increase_code<I: Iterator<Item = Ref>, J: Iterator<Item = usize>>(
    candidates: I,
    mut hints: J,
) -> Option<Code> {
    let mut candidates = candidates.peekable();
    Some(match candidates.peek() {
        Some(&(dist, run_minus_3)) if 1 == dist && 255 == run_minus_3 => {
            (&(dist, run_minus_3)).into()
        }
        Some(_) => {
            let candidates = sorted_candidates(candidates);

            match hints
                .next()
                .expect("there were some candidates, so we should have some hints left")
            {
                0 => candidates.get(0).expect("invalid input 1").into(),
                1 => return None,
                other => candidates.get(other - 1).expect("invalid input 2").into(),
            }
        }
        None => return None,
    })
}

pub fn increase_entropy(preroll: &[u8], data: &[u8], hints: &[usize]) -> Vec<Code> {
    let guesser = RefGuesser::new(preroll, data);
    let mut hints = hints.into_iter().map(|x| *x);

    let mut ret = Vec::with_capacity(data.len());
    let mut pos = 0usize;

    loop {
        let guesser = guesser.at(pos);
        let orig = match guesser.all_candidates() {
            Some(candidates) => {
                increase_code(candidates, &mut hints).unwrap_or_else(|| guesser.current_literal())
            }
            None => break,
        };
        ret.push(orig);
        pos += usize_from(orig.emitted_bytes());
    }

    while pos < guesser.data_len() {
        ret.push(guesser.at(pos).current_literal());
        pos += 1;
    }

    ret
}

#[cfg(test)]
mod tests {
    use super::reduce_entropy;
    use super::increase_entropy;
    use circles::CircularBuffer;
    use serialise;
    use Code;
    use Code::Literal as L;
    use Code::Reference as R;

    #[test]
    fn re_1_single_backref_abcdef_bcdefghi() {
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

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_2_two_length_three_runs() {
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

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_3_two_overlapping_runs() {
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

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_4_zero_run() {
        let exp = &[
            L(b'0'),
            R {
                dist: 1,
                run_minus_3: 10,
            },
        ];
        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_5_ref_before() {
        let exp = &[
            R {
                dist: 1,
                run_minus_3: ::pack_run(13),
            },
        ];
        assert_eq!(
            exp.iter().map(|_| 0usize).collect::<Vec<usize>>(),
            decode_maybe(&[0], exp)
        );
    }

    #[test]
    fn re_11_ref_long_before() {
        let exp = &[
            L(b'a'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            R {
                dist: 7,
                run_minus_3: ::pack_run(13),
            },
        ];
        assert_eq!(
            &[0],
            decode_maybe(&[b'q', b'r', b's', b't', b'u'], exp).as_slice()
        );
    }

    #[test]
    fn re_12_ref_over_edge() {
        let exp = &[
            L(b'd'),
            R {
                dist: 2,
                run_minus_3: ::pack_run(3),
            },
        ];
        assert_eq!(&[0], decode_maybe(&[b's', b't', b'u'], exp).as_slice());
    }

    #[test]
    fn re_6_just_long_run() {
        let exp = &[
            L(5),
            R {
                dist: 1,
                run_minus_3: ::pack_run(258),
            },
        ];

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_7_two_long_run() {
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

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }


    #[test]
    fn re_8_many_long_run() {
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

        assert_eq!(vec![0; 137], decode_then_reencode_single_block(&exp));
    }

    #[test]
    fn re_9_longer_match() {
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

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    fn decode_then_reencode_single_block(codes: &[Code]) -> Vec<usize> {
        decode_maybe(&[], codes)
    }

    fn decode_maybe(preroll: &[u8], codes: &[Code]) -> Vec<usize> {
        let mut data = Vec::with_capacity(codes.len());
        {
            let mut prebuf = CircularBuffer::with_capacity(32 * 1024);
            prebuf.extend(preroll);
            serialise::decompressed_codes(&mut data, &mut prebuf, codes).unwrap();
        }

        #[cfg(never)]
        println!(
            "data: {:?}, str: {:?}",
            data,
            String::from_utf8_lossy(&data)
        );

        let reduced = reduce_entropy(preroll, &data, codes).unwrap();
        assert_eq!(codes, increase_entropy(preroll, &data, &reduced).as_slice());
        reduced
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

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_10_repeat_after_ref_a122b_122_222() {
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

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
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

        assert_eq!(&[1, 0], decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn long_prelude() {
        let exp = &[
            L(b'b'),
            R {
                dist: 3,
                run_minus_3: ::pack_run(3),
            },
        ];

        let pre = concat(&[b'|'; 32768 + 1], b"ponies");

        #[cfg(never)]
        println!(
            "{}",
            String::from_utf8_lossy(
                serialise::DecompressedBytes::new(&pre, exp.iter())
                    .collect::<Vec<u8>>()
                    .as_slice()
            )
        );

        assert_eq!(&[0], decode_maybe(&pre, exp).as_slice());
    }

    fn concat(x: &[u8], y: &[u8]) -> Box<[u8]> {
        let mut v = Vec::with_capacity(x.len() + y.len());
        v.extend(x);
        v.extend(y);
        v.into_boxed_slice()
    }
}
