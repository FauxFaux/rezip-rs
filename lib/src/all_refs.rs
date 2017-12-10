use std::collections::HashMap;
use std::fmt;
use std::iter;

use itertools::Itertools;

use u16_from;
use usize_from;
use Code;
use Ref;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
struct Key {
    b0: u8,
    b1: u8,
    b2: u8,
}

type BackMap = HashMap<Key, Vec<usize>>;

pub struct AllRefs<'p, 'd> {
    preroll: &'p [u8],
    pub data: &'d [u8],
    map: BackMap,
}

impl<'p, 'd> AllRefs<'p, 'd> {
    pub fn with_everything(preroll: &'p [u8], data: &'d [u8]) -> Self {
        Self {
            preroll,
            data,
            map: whole_map(preroll.iter().chain(data).cloned()),
        }
    }

    // TODO: This requires prior-knowledge which isn't available during re-compression,
    // TODO: so can't actually be used in the real world. It's here because I couldn't
    // TODO: get the other way to work, so we should have a test implementation to
    // TODO: compare against
    pub fn limited_by(preroll: &'p [u8], data: &'d [u8], codes: &[Code], skip_over: u16) -> Self {
        Self {
            preroll,
            data,
            map: limited_map(preroll.iter().chain(data).cloned(), codes, skip_over),
        }
    }

    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    fn key(&self, data_pos: usize) -> Option<Key> {
        if data_pos + 2 < self.data.len() {
            Some(key_from_bytes(&self.data[data_pos..]))
        } else {
            None
        }
    }

    // None if we are out of possible keys, or Some(possibly empty list)
    pub fn at<'m>(&'m self, data_pos: usize) -> Option<Box<Iterator<Item = Ref> + 'm>> {
        let key = match self.key(data_pos) {
            Some(key) => key,
            None => return None,
        };

        let pos = self.preroll.len() + data_pos;

        // we can only find ourselves, which is invalid, and not handled by (inclusive) range code
        // Maybe I should fix the inclusive range code? Or pretend this is an optimisation.
        if 0 == pos {
            return Some(Box::new(iter::empty()));
        }

        Some(Box::new(
            self.map
                .get(&key)
                .map(|v| {
                    sub_range_inclusive(pos.saturating_sub(32 * 1024), pos.saturating_sub(1), v)
                })
                .unwrap_or(&[])
                .into_iter()
                .rev()
                .map(move |off| {
                    let dist = u16_from(pos - off);
                    let run = self.possible_run_length_at(data_pos, dist);
                    Ref::new(dist, run)
                }),
        ))
    }

    fn get_at_dist(&self, data_pos: usize, dist: u16) -> u8 {
        debug_assert!(dist > 0);
        let pos = data_pos;
        let dist = usize_from(dist);

        if dist <= pos {
            self.data[pos - dist]
        } else {
            self.preroll[self.preroll.len() - (dist - pos)]
        }
    }

    fn possible_run_length_at(&self, data_pos: usize, dist: u16) -> u16 {
        let upcoming_data = &self.data[data_pos..];
        let upcoming_data = &upcoming_data[..258.min(upcoming_data.len())];

        for cur in 3..dist.min(upcoming_data.len() as u16) {
            if upcoming_data[cur as usize] != self.get_at_dist(data_pos, dist - cur) {
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

fn sorted_back_map(map: &BackMap) -> Vec<(&Key, &Vec<usize>)> {
    let mut values: Vec<(&Key, &Vec<usize>)> = map.iter().collect();
    values.sort_unstable_by_key(|&(_, poses)| poses);
    values
}

fn key_from_bytes(from: &[u8]) -> Key {
    Key {
        b0: from[0],
        b1: from[1],
        b2: from[2],
    }
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

fn whole_map<I: Iterator<Item = u8>>(data: I) -> BackMap {
    let mut map = BackMap::with_capacity(32 * 1024);

    for (pos, keys) in data.tuple_windows::<(u8, u8, u8)>().enumerate() {
        map.entry(keys.into())
            .or_insert_with(|| Vec::new())
            .push(pos);
    }

    map
}

// TODO: see note on AllRefs::limited_by, the only caller of this method.
fn limited_map<I: Iterator<Item = u8>>(data: I, codes: &[Code], skip_over: u16) -> BackMap {
    let mut map = BackMap::with_capacity(32 * 1024);

    // Argh.
    // At each pos, we want to know if we're:
    // * in a literal, so to add a ref
    // * at the start of a code, so add a ref
    // in the middle of a short-enough code, so add a ref
    // in the middle of a longer ref, so not to do anything

    // conceptually, this could be converting:
    // L, L, R(.., 4), L, R(.., 3),
    // with a cut-off of allowing the 3, into:
    // t, t, t, f,f,f, t, t, t, t,
    // i.e. ignoring everything but the first true in the R(.., 4),
    // then zip-with that and skip based on !x.

    let mut skip = 0u16;
    let mut code_pos = 0usize;
    let mut codes = codes.iter();

    for (pos, keys) in data.tuple_windows::<(u8, u8, u8)>().enumerate() {
        if pos > code_pos {
            assert_eq!(pos, code_pos + 1);

            let run_len = codes.next().map(|code| code.emitted_bytes()).unwrap_or(0);

            if run_len > skip_over {
                skip = run_len - 1;
            }

            code_pos += usize_from(run_len);
        }

        if skip > 0 {
            skip -= 1;
            continue;
        }

        map.entry(keys.into())
            .or_insert_with(|| Vec::new())
            .push(pos);
    }

    map
}

impl From<(u8, u8, u8)> for Key {
    fn from(tuple: (u8, u8, u8)) -> Self {
        Key {
            b0: tuple.0,
            b1: tuple.1,
            b2: tuple.2,
        }
    }
}

fn normal_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c.is_ascii_graphic() || c.is_ascii_punctuation()
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if normal_char(self.b0) && normal_char(self.b1) && normal_char(self.b2) {
            write!(
                f,
                "\"{}{}{}\"",
                self.b0 as char,
                self.b1 as char,
                self.b2 as char
            )
        } else {
            write!(
                f,
                "{:?}{:?}{:?}",
                self.b0 as char,
                self.b1 as char,
                self.b2 as char
            )
        }
    }
}

impl<'p, 'd> fmt::Debug for AllRefs<'p, 'd> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (key, val) in sorted_back_map(&self.map) {
            writeln!(f, " - {:?}: {:?}", key, val)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::sorted_back_map;
    use super::Key;

    use Code;
    use Ref;

    use Code::Literal as L;
    fn r(dist: u16, run: u16) -> Code {
        Code::Reference(Ref::new(dist, run))
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
    fn whole() {
        use super::whole_map;
        assert_eq!(
            hashmap! {
                k(b"abc") => vec![0, 3],
                k(b"bca") => vec![1],
                k(b"cab") => vec![2],
            },
            whole_map(b"abcabc".iter().cloned())
        )
    }

    #[test]
    fn limited() {
        use super::limited_map;

        // the central "bcdef" is detected as a run,
        // but is long enough to trigger the map corruption,
        // so entries (9...12) inclusive ('c' -> 'f') don't end
        // up in the map. In `gzip -1`, this looks like the
        // compression we see below, where the 4-length run is at
        // dist 11, because it can't see the version at position 8.

        // sorting just for better error messages
        assert_eq!(
            sorted_back_map(&hashmap! {
                k(b"1ab") => vec![0],
                k(b"abc") => vec![1],
                k(b"bcd") => vec![2, 8],
                k(b"cde") => vec![3, 14],
                k(b"def") => vec![4, 15],
                k(b"ef,") => vec![5],
                k(b"f,b") => vec![6],
                k(b",bc") => vec![7],
                k(b"-cd") => vec![13],
            }),
            sorted_back_map(&limited_map(
                b"1abcdef,bcdef-cdef".iter().cloned(),
                &[
                    L(b'1'),
                    L(b'a'),
                    L(b'b'),
                    L(b'c'),
                    L(b'd'),
                    L(b'e'),
                    L(b'f'),
                    L(b','),
                    r(6, 5),
                    L(b'-'),
                    r(11, 4)
                ],
                4
            ))
        )
    }

    fn k(from: &[u8]) -> Key {
        assert_eq!(3, from.len());
        Key {
            b0: from[0],
            b1: from[1],
            b2: from[2],
        }
    }
}
