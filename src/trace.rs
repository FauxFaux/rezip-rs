// Work out the set of supported algorithms:
// * Fail at first byte.
// * Seek back up to N.
// * Miss encoding a location, and encode the next one.
// * Skip further back to find a longer code. Maintain the code lengths lengths?
// decode the thing symbol by symbol
// If a mode's decision isn't taken, drop that mode from the possible set of modes.
// If no modes are left, we didn't work.
// If any modes are left, pick the "simplest", and return it.

// Still need to fully decode the input, and store the whole backref search buffer.
// Can we use the same buffer? Probably too complex for first pass.

// Do we need to rearrange the api so we can process a sequence and its decoded bytes?

// How about we output a big list of:
// [Run where dumb algo was correct:u[16|32]] [lits:u8] [dist:u16, run:u8]?

use std::collections::HashMap;

use circles::CircularBuffer;
use errors::*;
use guess;
use three::ThreePeek;
use Code;
use u16_from;
use pack_run;

type Key = (u8, u8, u8);
type BackMap = HashMap<Key, Vec<usize>>;

pub fn whole_map<I: Iterator<Item = u8>>(data: I) -> BackMap {
    let mut map = BackMap::with_capacity(32 * 1024);
    let mut it = ThreePeek::new(data);

    let mut pos = 0;
    while let Some(keys) = it.next_three() {
        map.entry(keys).or_insert_with(|| Vec::new()).push(pos);
        pos += 1;
    }

    map
}

pub fn all_options(preroll: &[u8], data: &[u8], map: &BackMap) -> Vec<Vec<Code>> {
    let mut ret = Vec::with_capacity(data.len());
    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
    dictionary.extend(preroll);

    let mut it = ThreePeek::new(data.into_iter());

    while let Some(key) = it.next_three() {
        // TODO: This shouldn't really be full of &u8s, should it?
        let key = (*key.0, *key.1, *key.2);

        let data_pos = ret.len();
        let pos = data_pos + preroll.len();

        let candidates = match map.get(&key) {
            Some(val) => val,
            None => {
                ret.push(vec![Code::Literal(key.0)]);
                continue;
            }
        };
        assert!(!candidates.is_empty());

        let mut us = Vec::with_capacity(candidates.len());

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

            let run = dictionary.possible_run_length_at(dist, &data[data_pos..]);

            assert!(run >= 3);

            us.push(Code::Reference {
                dist: u16_from(dist), run_minus_3: pack_run(run)
            })
        }

        us.shrink_to_fit();
        ret.push(us);
    }

    for remaining_byte in it {
        ret.push(vec![Code::Literal(*remaining_byte)]);
    }

    assert_eq!(data.len(), ret.len());
    ret
}

trait Algo {
    fn accept(&mut self, code: &Code, dictionary: &CircularBuffer) -> Result<bool>;
}

pub fn trace(preroll: &[u8], codes: &[Code]) -> Result<()> {
    ensure!(!codes.is_empty(), "unexpected empty block");

    let window_size = guess::max_distance(codes).unwrap();
    let (outside, hits_first_byte) = guess::outside_range_or_hit_zero(codes);

    let first_byte_bug = preroll.is_empty() && !hits_first_byte;

    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
    dictionary.extend(preroll);

    let mut it = codes.iter().peekable();

    let first = it.next().unwrap();

    let target = match *first {
        Code::Literal(byte) => {
            dictionary.push(byte);

            // if we'd find a Reference here, then we're in trouble and we need to enable SKIPPY
            // return and try another method?

            vec![byte]
        }
        Code::Reference { dist, run_minus_3 } => {
            let run = ::unpack_run(run_minus_3);
            let mut v = vec![];
            dictionary.copy(dist, run, &mut v)?;
            v
        }
    };


    unimplemented!()
}
