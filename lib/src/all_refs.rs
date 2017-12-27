use std::collections::HashMap;
use std::fmt;
use std::iter;

use itertools::Itertools;

use bad_table::NicerTable;
use obscure::obscure;
use u16_from;
use usize_from;
use Code;
use Obscure;
use Ref;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Key {
    b0: u8,
    b1: u8,
    b2: u8,
}

struct SixteenDetails {
    limit: u16,
    table: NicerTable,
}

enum Mappy {
    Full(BackMap),
    Sixteen(SixteenDetails),
}

type BackMap = HashMap<Key, Vec<usize>>;

pub struct AllRefs<'p, 'd> {
    preroll: &'p [u8],
    pub data: &'d [u8],
    map: Mappy,
}

impl<'p, 'd> AllRefs<'p, 'd> {
    pub fn with_everything(preroll: &'p [u8], data: &'d [u8]) -> Self {
        Self {
            preroll,
            data,
            map: Mappy::Full(whole_map(preroll.iter().chain(data).cloned())),
        }
    }

    pub fn with_sixteen(preroll: &'p [u8], data: &'d [u8], limit: u16) -> Self {
        assert_eq!(&[0u8; 0], preroll, "not implemented");
        AllRefs {
            preroll,
            data,
            map: Mappy::Sixteen(SixteenDetails {
                table: NicerTable::from_window(data),
                limit,
            }),
        }
    }

    pub fn apply_first_byte_bug_rule(&mut self) {
        if let Some(ref k) = self.key(0) {
            // TODO: ???
            match self.map {
                Mappy::Full(ref mut map) => if let Some(v) = map.get_mut(&k) {
                    v.remove_item(&self.preroll.len());
                },
                Mappy::Sixteen(_) => {
                    // TODO: unimplemented
                }
            }
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
    pub fn at<'m>(
        &'m self,
        data_pos: usize,
        obscura: &[Obscure],
    ) -> Option<Box<Iterator<Item = Ref> + 'm>> {
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

        match self.map {
            Mappy::Full(ref map) => Some(Box::new(
                map.get(&key)
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
            )),
            Mappy::Sixteen(SixteenDetails { ref table, limit }) => Some(Box::new(
                obscure(
                    table
                        .get(key)
                        .filter(move |off| (*off as usize) < pos)
                        .filter(move |&off| {
                            self.data[usize_from(off)..usize_from(off) + 3] == key.as_array()[..]
                        }),
                    obscura.iter().map(|&(k, v)| {
                        assert_lt!(k, 65536);
                        (k as u16, v)
                    }),
                ).take(limit as usize)
                    .map(move |off| {
                        let dist = u16_from(pos) - off;
                        let run = self.possible_run_length_at(data_pos, dist);
                        Ref::new(dist, run)
                    }),
            )),
        }
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
        map.entry(Key::from(keys))
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

impl Key {
    pub fn sixteen_hash_16(&self) -> u16 {
        let mut hash = 0u16;

        hash ^= u16::from(self.b0);
        hash <<= 6;
        hash ^= u16::from(self.b1);
        hash <<= 6;
        hash ^= u16::from(self.b2);

        hash &= 0x7fff;
        hash
    }

    fn as_array(&self) -> [u8; 3] {
        [self.b0, self.b1, self.b2]
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
                self.b0 as char, self.b1 as char, self.b2 as char
            )
        } else {
            write!(
                f,
                "{:?}{:?}{:?}",
                self.b0 as char, self.b1 as char, self.b2 as char
            )
        }
    }
}

impl<'p, 'd> fmt::Debug for AllRefs<'p, 'd> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.map {
            Mappy::Full(ref map) => for (key, val) in sorted_back_map(map) {
                writeln!(f, " - {:?}: {:?}", key, val)?;
            },
            Mappy::Sixteen(SixteenDetails { ref table, .. }) => {
                writeln!(f, "{:?}", table)?;
            }
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
    fn hash_sixteen_16_collisions() {
        assert_eq!(0x73cf, Key::from((15, 15, 15)).sixteen_hash_16());
        assert_eq!(0x73cf, Key::from((79, 15, 15)).sixteen_hash_16());

        assert_eq!(
            0b0100_1111_0011_1111,
            Key::from((0xff, 0xff, 0xff)).sixteen_hash_16()
        );
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
