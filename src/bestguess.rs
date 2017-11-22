use std::collections::HashMap;
use std::cmp;
use std::slice;

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
        it: ThreePeek::new(data.into_iter()),
        data_pos: 0,
        lengths,
    }
}

pub struct AllOptions<'a> {
    dictionary: CircularBuffer,
    data_start: usize,
    data: &'a [u8],
    map: BackMap,
    it: ThreePeek<slice::Iter<'a, u8>>,
    data_pos: usize,
    lengths: Lengths,
}

impl<'a> Iterator for AllOptions<'a> {
    type Item = Vec<Code>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next_three() {
            Some(key) => {
                // TODO: This shouldn't really be full of &u8s, should it?
                let key = (*key.0, *key.1, *key.2);

                let ret = Some(self.stateful_options(key));
                self.dictionary.push(key.0);
                self.data_pos += 1;
                ret
            },
            None => self.it.next().map(|byte| vec![Code::Literal(*byte)])
        }
    }
}

impl<'a> AllOptions<'a> {
    fn stateful_options(&mut self, key: Key) -> Vec<Code> {
        // it's always possible to emit the literal
        let current_byte = key.0;

        let candidates = match self.map.get(&key) {
            Some(val) => val,
            None => {
                return vec![Code::Literal(current_byte)];
            }
        };
        assert!(!candidates.is_empty());

        let data_pos = self.data_pos;
        let pos = data_pos + self.data_start;

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

            let upcoming_data = &self.data[data_pos..];
            let run = self.dictionary.possible_run_length_at(dist, upcoming_data);

            assert!(
                run >= 3,
                "only matched {} bytes like {:?} at -{}",
                run,
                upcoming_data,
                dist
            );

            us.push(Code::Reference {
                dist,
                run_minus_3: pack_run(run),
            })
        }

        us.sort_by(|left, right| compare(&self.lengths, left, right));
        us.shrink_to_fit();
        us
    }
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
    u16::from(mean_literal_len) * code.emitted_bytes()
}

trait IteratorZoomer {
    fn advance(&mut self, n: usize);
}

impl<I: Iterator> IteratorZoomer for I {
    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            self.next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_all_options;
    use super::IteratorZoomer;
    use circles;
    use huffman;
    use serialise;
    use usize_from;
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

        assert_unity(exp);
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

        assert_unity(exp);
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

        assert_unity(exp);
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
        assert_unity(exp);
    }

    #[test]
    fn ref_before() {
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
    fn just_long_run() {
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

        assert_unity(exp);
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

        assert_unity(&exp);
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

        println!(
            "bytes: {:?}, str: {:?}",
            bytes,
            String::from_utf8_lossy(&bytes)
        );


        let lengths =
            serialise::Lengths::new(&huffman::FIXED_LENGTH_TREE, &huffman::FIXED_DISTANCE_TREE);

        let mut it = find_all_options(lengths, preroll, &bytes)
            .into_iter()
            .enumerate();

        let mut cit = codes.iter();

        let mut we_chose = Vec::with_capacity(codes.len());

        while let Some((pos, vec)) = it.next() {
            let orig = cit.next().expect("desync");
            println!(
                "byte {}: trying to guess {:?}, we have {:?}",
                pos,
                orig,
                vec
            );
            let chosen = vec.iter().position(|x| x == orig).expect("it must be here");
            we_chose.push(chosen);
            it.advance(usize_from(orig.emitted_bytes() - 1));
        }

        assert_eq!(None, cit.next());

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
}
