/// Right, time to try the trace-like algorithm again.
/// Ranking?
///  1. The longest, closest run, if there is one.
///  2. A literal
///  3. The next closest run of the same length.
///  4. The next slightly shorter run that's closest.


use std::collections::HashMap;
use std::iter;

use itertools::Itertools;
use result::ResultOptionExt;

use errors::*;

use pack_run;
use u16_from;
use usize_from;
use Code;

type Key = (u8, u8, u8);
// len, run_minus_3
type Ref = (u16, u8);
type BackMap = HashMap<Key, Vec<usize>>;

fn whole_map<I: Iterator<Item = u8>>(data: I) -> BackMap {
    let mut map = BackMap::with_capacity(32 * 1024);

    for (pos, keys) in data.tuple_windows::<Key>().enumerate() {
        map.entry(keys).or_insert_with(|| Vec::new()).push(pos);
    }

    map
}

struct AllOptions<'p, 'd> {
    preroll: &'p [u8],
    data: &'d [u8],
    map: BackMap,
}

struct AllOptionsCursor<'a, 'p: 'a, 'd: 'a> {
    inner: &'a AllOptions<'p, 'd>,
    data_pos: usize,
}

fn key_from_bytes(from: &[u8]) -> Key {
    (from[0], from[1], from[2])
}

impl<'p, 'd> AllOptions<'p, 'd> {
    fn new(preroll: &'p [u8], data: &'d [u8]) -> Self {
        Self {
            preroll,
            data,
            map: whole_map(preroll.iter().chain(data).map(|x| *x)),
        }
    }

    fn at(&self, pos: usize) -> AllOptionsCursor {
        AllOptionsCursor {
            inner: self,
            data_pos: pos,
        }
    }

    fn data_len(&self) -> usize {
        self.data.len()
    }
}

impl<'a, 'p, 'd> AllOptionsCursor<'a, 'p, 'd> {
    pub fn key(&self) -> Option<Key> {
        if self.data_pos + 2 < self.inner.data.len() {
            Some(key_from_bytes(&self.inner.data[self.data_pos..]))
        } else {
            None
        }
    }

    fn pos(&self) -> usize {
        self.data_pos + self.inner.preroll.len()
    }

    // None if we are out of possible keys, or Some(possibly empty list)
    pub fn all_candidates<'m>(&'m self) -> Option<Box<Iterator<Item = Ref> + 'm>> {
        let key = match self.key() {
            Some(key) => key,
            None => return None,
        };

        // TODO: off-by-ones?
        let pos = self.pos();

        // we can only find ourselves, which is invalid, and not handled by (inclusive) range code
        // Maybe I should fix the inclusive range code? Or pretend this is an optimisation.
        if 0 == pos {
            return Some(Box::new(iter::empty()));
        }

        Some(Box::new(
            self.inner.map
                .get(&key)
                .map(|v| {
                    sub_range_inclusive(pos.saturating_sub(32 * 1024), pos.saturating_sub(1), v)
                })
                .unwrap_or(&[])
                .into_iter()
                .rev()
                .map(move |off| {
                    let dist = u16_from(pos - off);
                    let run = pack_run(self.possible_run_length_at(dist));
                    (dist, run)
                }),
        ))
    }

    pub fn current_literal(&self) -> Code {
        Code::Literal(self.inner.data[self.data_pos])
    }

    fn get_at_dist(&self, dist: u16) -> u8 {
        debug_assert!(dist > 0);
        let pos = self.data_pos;
        let dist = usize_from(dist);

        if dist <= pos {
            self.inner.data[pos - dist]
        } else {
            self.inner.preroll[self.inner.preroll.len() - (dist - pos)]
        }
    }

    fn possible_run_length_at(&self, dist: u16) -> u16 {
        let upcoming_data = &self.inner.data[self.data_pos..];
        let upcoming_data = &upcoming_data[..258.min(upcoming_data.len())];

        for cur in 3..dist.min(upcoming_data.len() as u16) {
            if upcoming_data[cur as usize] != self.get_at_dist(dist - cur) {
                return cur;
            }
        }

        for cur in dist..(upcoming_data.len() as u16) {
            if upcoming_data[(cur % dist) as usize] != upcoming_data[cur as usize] {
                return cur;
            }
        }

        upcoming_data.len() as u16
    }
}

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
            Some(find_reference_score(dist, run_minus_3, candidates)
                .chain_err(|| format!("looking for {:?}", orig))?)
        }
    })
}

pub fn reduce_entropy(preroll: &[u8], data: &[u8], codes: &[Code]) -> Result<Vec<usize>> {
    let options = AllOptions::new(preroll, data);

    let mut pos = 0usize;

    codes
        .into_iter()
        .flat_map(|orig| {
            let options = options.at(pos);
            let reduced: Option<Result<usize>> = options.all_candidates().and_then(|candidates| {
                reduce_code(orig, candidates)
                    .chain_err(|| format!("looking for {:?}", options.key()))
                    .invert()
            });

            pos += usize_from(orig.emitted_bytes());

            reduced
        })
        .collect()
}

fn increase_code<I: Iterator<Item = Ref>, J: Iterator<Item = usize>>(
    candidates: I,
    mut hint: J,
) -> Option<Code> {
    let mut candidates = candidates.peekable();
    Some(match candidates.peek() {
        Some(&(dist, run_minus_3)) if 1 == dist && 255 == run_minus_3 => {
            (&(dist, run_minus_3)).into()
        }
        Some(_) => {
            let candidates = sorted_candidates(candidates);

            match hint.next()
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
    let options = AllOptions::new(preroll, data);
    let mut hints = hints.into_iter().map(|x| *x);

    let mut ret = Vec::with_capacity(data.len());
    let mut pos = 0usize;

    loop {
        let options = options.at(pos);
        let orig = match options.all_candidates() {
            Some(candidates) => {
                increase_code(candidates, &mut hints).unwrap_or_else(|| options.current_literal())
            }
            None => break,
        };
        ret.push(orig);
        pos += usize_from(orig.emitted_bytes());
    }

    while pos < options.data_len() {
        ret.push(options.at(pos).current_literal());
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
