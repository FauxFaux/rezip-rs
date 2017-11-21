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
        dictionary.push(current_byte);

        let candidates = match map.get(&key) {
            Some(val) => val,
            None => {
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

            let run = dictionary.possible_run_length_at(dist, &data[data_pos..]);

            assert!(run >= 3);

            us.push(Code::Reference {
                dist,
                run_minus_3: pack_run(run),
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
