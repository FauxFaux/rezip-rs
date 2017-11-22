use std::collections::HashMap;
use std::cmp;

use circles::CircularBuffer;
use errors::*;
use three::ThreePeek;
use serialise::Lengths;
use Code;
use pack_run;
use unpack_run;

type Key = (u8, u8, u8);
type BackMap = HashMap<Key, Vec<usize>>;

fn whole_map<I: Iterator<Item = u8>>(data: I) -> BackMap {
    let mut map = BackMap::with_capacity(32 * 1024);
    let mut it = ThreePeek::new(data);

    let mut pos = 0;
    while let Some(keys) = it.next_three() {
        map.entry(keys).or_insert_with(|| Vec::new()).push(pos);
        pos += 1;
    }

    map
}

pub fn find_all_options(lengths: Lengths, preroll: &[u8], data: &[u8]) -> Vec<Vec<Code>> {
    let map = whole_map(preroll.iter().chain(data).map(|x| *x));

    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
    dictionary.extend(preroll);

    let data_start = preroll.len();

    all_options(&mut dictionary, data_start, data, &map)
        .into_iter()
        .map(|mut v| {
            v.sort_by(|left, right| compare(&lengths, left, right));
            v
        })
        .collect()
}

fn all_options(
    dictionary: &mut CircularBuffer,
    data_start: usize,
    data: &[u8],
    map: &BackMap,
) -> Vec<Vec<Code>> {
    let mut ret = Vec::with_capacity(data.len());

    let mut it = ThreePeek::new(data.into_iter());

    while let Some(key) = it.next_three() {
        // TODO: This shouldn't really be full of &u8s, should it?
        let key = (*key.0, *key.1, *key.2);

        // it's always possible to emit the literal
        let current_byte = key.0;

        let candidates = match map.get(&key) {
            Some(val) => val,
            None => {
                dictionary.push(current_byte);
                ret.push(vec![Code::Literal(current_byte)]);
                continue;
            }
        };
        assert!(!candidates.is_empty());

        let data_pos = ret.len();
        let pos = data_pos + data_start;

        let mut us = Vec::with_capacity(candidates.len());
        us.push(Code::Literal(current_byte));

        for candidate_pos in candidates {
            let candidate_pos = *candidate_pos;

            // TODO: ge or gt?
            if candidate_pos >= pos {
                continue;
            }

            let dist = pos - candidate_pos;

            if dist > 32 * 1024 {
                continue;
            }

            let dist = dist as u16;

            let upcoming_data = &data[data_pos..];
            let run = dictionary.possible_run_length_at(dist, upcoming_data);

            assert!(run >= 3, "only matched {} bytes like {:?} at -{}", run, upcoming_data, dist);

            us.push(Code::Reference {
                dist,
                run_minus_3: pack_run(run),
            })
        }

        us.shrink_to_fit();

        dictionary.push(current_byte);
        ret.push(us);
    }

    for remaining_byte in it {
        ret.push(vec![Code::Literal(*remaining_byte)]);
    }

    assert_eq!(data.len(), ret.len());
    ret
}

fn compare(lengths: &Lengths, left: &Code, right: &Code) -> cmp::Ordering {
    let left_len = lengths.length(left).unwrap_or(u8::max_value()) as isize;
    let right_len = lengths.length(left).unwrap_or(u8::max_value()) as isize;

    // we could do even better than this by looking at the *actual* saving vs. all literals,
    // but it still won't be accurate as that would assume no further back-references.
    let left_saved = saved_bits(left, lengths.mean_literal_len) as isize;
    let right_saved = saved_bits(right, lengths.mean_literal_len) as isize;

    // firstly, let's compare their savings; savings are always good
    match (left_len - left_saved).cmp(&(right_len - right_saved)) {
        cmp::Ordering::Equal => {}
        other => return other,
    }

    // if both would save us the same amount, then...
    use Code::*;
    match *left {
        Literal(_) => match *right {
            Literal(_) => unreachable!(
                "there's never two different literals that could be encoded instead of each other"
            ),
            Reference { .. } => {
                // literals are worse than references
                cmp::Ordering::Greater
            }
        },
        Reference {
            dist: left_dist,
            run_minus_3: left_run_minus_3,
        } => match *right {
            Literal(_) => {
                // literals are worse than references
                cmp::Ordering::Less
            }

            Reference {
                dist: right_dist,
                run_minus_3: right_run_minus_3,
            } => {
                let left_run = unpack_run(left_run_minus_3);
                let right_run = unpack_run(right_run_minus_3);

                // shorter distances, then bigger runs.

                // bigger runs should already have been covered by the length calc,
                // and shorter distances are more likely to be spotted by flawed encoders?
                left_dist.cmp(&right_dist).then(right_run.cmp(&left_run))
            }
        },
    }
}

fn saved_bits(code: &Code, mean_literal_len: u8) -> u16 {
    u16::from(mean_literal_len) * match *code {
        Code::Literal(_) => 1,
        Code::Reference { run_minus_3, .. } => unpack_run(run_minus_3),
    }
}

#[cfg(test)]
mod tests {
    use super::find_all_options;
    use circles;
    use huffman;
    use serialise;
    use Code;
    use Code::Literal as L;
    use Code::Reference as R;

    #[test]
    fn single_backref_abcdef_bcdefghi() {
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


    // TODO: #[test]
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

//        let window_size = max_distance(codes).unwrap();
//        let mut ret = Vec::with_capacity(codes.len());
        let mut bytes = Vec::new();

        serialise::decompressed_codes(&mut bytes, &mut circles::CircularBuffer::with_capacity(32 * 1024), codes).unwrap();

        let lengths = serialise::Lengths::new(&huffman::FIXED_LENGTH_TREE, &huffman::FIXED_DISTANCE_TREE);

        for val in find_all_options(lengths, &[], &bytes) {
            println!("{:?}", val);
        }

        unimplemented!()
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
