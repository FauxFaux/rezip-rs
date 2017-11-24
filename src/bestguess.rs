/// Right, time to try the trace-like algorithm again.
/// Ranking?
///  1. The longest, closest run, if there is one.
///  2. A literal
///  3. The next closest run of the same length.
///  4. The next slightly shorter run that's closest.


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

pub fn find_all_options<'a>(lengths: Lengths, preroll: &[u8], data: &'a [u8]) -> AllOptions<'a> {
    let map = whole_map(preroll.iter().chain(data).map(|x| *x));

    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
    dictionary.extend(preroll);

    let data_start = preroll.len();

    AllOptions {
        dictionary,
        data_start,
        data,
        map,
        data_pos: 0,
        lengths,
    }
}

pub struct AllOptions<'a> {
    dictionary: CircularBuffer,
    data_start: usize,
    data: &'a [u8],
    map: BackMap,
    data_pos: usize,
    lengths: Lengths,
}

fn key_from_bytes(from: &[u8]) -> Key {
    (from[0], from[1], from[2])
}

impl<'a> AllOptions<'a> {
    pub fn advance(&mut self, n: usize) {
        self.dictionary
            .extend(&self.data[self.data_pos..self.data_pos + n]);
        self.data_pos += n;
    }

    pub fn key(&self) -> Option<Key> {
        if self.data_pos + 2 < self.data.len() {
            Some(key_from_bytes(&self.data[self.data_pos..]))
        } else {
            None
        }
    }

    fn pos(&self) -> usize {
        self.data_pos + self.data_start
    }

    pub fn all_candidates(&self, key: &Key) -> &[usize] {
        // TODO: off-by-ones?
        let pos = self.pos();

        // we can only find ourselves, which is invalid, and not handled by (inclusive) range code
        // Maybe I should fix the inclusive range code? Or pretend this is an optimisation.
        if 0 == pos {
            return &[];
        }

        self.map
            .get(key)
            .map(|v| {
                sub_range_inclusive(pos.saturating_sub(32 * 1024), pos.saturating_sub(1), v)
            })
            .unwrap_or(&[])
    }

    fn reference_from_dist(&self, dist: u16) -> Code {
        Code::Reference {
            dist,
            run_minus_3: pack_run(self.possible_run_length_at(dist)),
        }
    }

    pub fn possible_run_length_at(&self, dist: u16) -> u16 {
        self.dictionary
            .possible_run_length_at(dist, &self.data[self.data_pos..])
    }
}

fn find_reference_score<I: Iterator<Item = u16>>(
    actual_dist: u16,
    actual_run: u16,
    options: &AllOptions,
    candidates: I,
) -> usize {
    if 258 == actual_run && 1 == actual_dist {
        return 0;
    }

    let mut us = Vec::with_capacity(candidates.size_hint().0);

    // let candidates: Vec<u16> = candidates.collect();
    // println!("{:?}", candidates);

    for dist in candidates {
        let run = options.possible_run_length_at(dist);
        // TODO: if run == 258 ..?
        us.push((dist, run));
    }

    us.sort();

    us.into_iter()
        .position(|(dist, run)| actual_run == run && actual_dist == dist)
        .expect("it must be there?")
}

fn sub_range_inclusive(start: usize, end: usize, range: &[usize]) -> &[usize] {
    let end_idx = match range.binary_search(&end) {
        Ok(e) => e + 1,
        Err(e) => e,
    };

    let range = &range[..end_idx];

    let start_idx = match range.binary_search(&start) {
        Ok(e) => e,
        Err(e) => e,
    };

    &range[start_idx..]
}

#[cfg(test)]
mod tests {
    use super::find_all_options;
    use super::find_reference_score;
    use circles;
    use huffman;
    use serialise;
    use usize_from;
    use u16_from;
    use unpack_run;
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

        assert_unity(exp);
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

        assert_unity(exp);
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

        assert_unity(exp);
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
        assert_unity(exp);
    }

    #[test]
    fn re_5_ref_before() {
        let exp = &[
            R {
                dist: 1,
                run_minus_3: 10,
            },
        ];
        assert_eq!(
            exp.iter().map(|_| 0usize).collect::<Vec<usize>>(),
            decode_then_reencode(&[0], exp)
        );
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

        assert_unity(exp);
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

        assert_unity(exp);
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

        assert_unity(&exp);
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

        assert_unity(exp);
    }

    fn decode_then_reencode_single_block(codes: &[Code]) -> Vec<usize> {
        decode_then_reencode(&[], codes)
    }

    fn decode_then_reencode(preroll: &[u8], codes: &[Code]) -> Vec<usize> {
        //        let window_size = max_distance(codes).unwrap();
        //        let mut ret = Vec::with_capacity(codes.len());
        let mut bytes = Vec::new();
        {
            let mut prebuf = circles::CircularBuffer::with_capacity(32 * 1024);
            prebuf.extend(preroll);
            serialise::decompressed_codes(&mut bytes, &mut prebuf, codes).unwrap();
        }

        #[cfg(never)]
        println!(
            "bytes: {:?}, str: {:?}",
            bytes,
            String::from_utf8_lossy(&bytes)
        );

        let mut we_chose = Vec::with_capacity(codes.len());

        let lengths =
            serialise::Lengths::new(&huffman::FIXED_LENGTH_TREE, &huffman::FIXED_DISTANCE_TREE);

        let mut it = find_all_options(lengths, preroll, &bytes);

        for orig in codes {
            let key = match it.key() {
                Some(key) => key,
                None => {
                    we_chose.push(0);
                    continue;
                }
            };



            we_chose.push(match *orig {
                Code::Literal(_) => {
                    if it.all_candidates(&key).is_empty() {
                        //There are no runs, so a literal is the only, obvious choice
                        0
                    } else {
                        // There's a run available, and we've decided not to pick it; unusual
                        1
                    }
                }

                Code::Reference {
                    dist: actual_dist,
                    run_minus_3: actual_run_minus_3,
                } => {
                    let candidates = it.all_candidates(&key);
                    let actual_run = unpack_run(actual_run_minus_3);
                    let pos = it.pos();
                    find_reference_score(
                        actual_dist,
                        actual_run,
                        &it,
                        candidates.into_iter().rev().map(|off| u16_from(pos - off)),
                    )
                }
            });

            it.advance(usize_from(orig.emitted_bytes()));
        }

        we_chose
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

        assert_unity(exp);
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

        assert_unity(exp);
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

        assert_eq!(
            &[0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
            decode_then_reencode_single_block(exp).as_slice()
        );
    }

    fn assert_unity(exp: &[Code]) {
        assert_eq!(
            exp.iter().map(|x| 0).collect::<Vec<usize>>(),
            decode_then_reencode_single_block(exp)
        );
    }

    #[test]
    fn sub_range() {
        use super::sub_range_inclusive as s;
        assert_eq!(&[5, 6], s(5, 6, &[4, 5, 6, 7]));
        assert_eq!(&[5, 6], s(5, 6, &[5, 6, 7]));
        assert_eq!(&[5, 6], s(5, 6, &[4, 5, 6]));

        assert_eq!(&[5, 6], s(4, 7, &[2, 3, 5, 6, 8, 9]));
        assert_eq!(&[5, 6], s(4, 7, &[5, 6, 8, 9]));
        assert_eq!(&[5, 6], s(4, 7, &[2, 3, 5, 6]));

        assert_eq!(&[0usize; 0], s(7, 8, &[4, 5, 6]));
        assert_eq!(&[0usize; 0], s(7, 8, &[9, 10]));
        assert_eq!(&[0usize; 0], s(7, 8, &[]));
    }
}
