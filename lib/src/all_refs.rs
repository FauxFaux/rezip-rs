use std::convert::TryFrom;
use std::fmt;
use std::iter;

use crate::back_map::BackMap;
use crate::obscure::obscure;
use crate::Obscure;
use crate::Ref;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Key {
    b0: u8,
    b1: u8,
    b2: u8,
}

pub struct AllRefs<'p, 'd> {
    pub preroll: &'p [u8],
    pub data: &'d [u8],
    map: BackMap,
    limit: u16,
}

impl<'p, 'd> AllRefs<'p, 'd> {
    pub fn with_sixteen(preroll: &'p [u8], data: &'d [u8], limit: u16) -> Self {
        AllRefs {
            preroll,
            data,
            limit,
            map: BackMap::from_window(preroll, data),
        }
    }

    pub fn data_len(&self) -> usize {
        self.preroll.len() + self.data.len()
    }

    fn key(&self, data_pos: usize) -> Option<Key> {
        if data_pos + 2 < self.data_len() {
            Some(Key {
                b0: self.get(data_pos),
                b1: self.get(data_pos + 1),
                b2: self.get(data_pos + 2),
            })
        } else {
            None
        }
    }

    /// None if we are out of possible keys, or Some(possibly empty list)
    pub fn at<'m>(
        &'m self,
        pos: usize,
        obscura: &[Obscure],
    ) -> Option<Box<dyn Iterator<Item = Ref> + 'm>> {
        let key = match self.key(pos) {
            Some(key) => key,
            None => return None,
        };

        // we can only find ourselves, which is invalid, and not handled by (inclusive) range code
        // Maybe I should fix the inclusive range code? Or pretend this is an optimisation.
        if 0 == pos {
            return Some(Box::new(iter::empty()));
        }

        Some(Box::new(
            obscure(
                self.map.get(key).filter(move |&off| off < pos),
                obscura.iter().cloned(),
            )
            .take(usize::try_from(self.limit).expect("todo: usize"))
            .filter(move |&off| pos - off <= 32_768)
            .filter(move |&off| {
                self.get(off) == key.b0
                    && self.get(off + 1) == key.b1
                    && self.get(off + 2) == key.b2
            })
            .map(move |off| {
                let dist = u16::try_from(pos - off).expect("logically sound");
                let run = self.possible_run_length_at(pos, dist);
                Ref::new(dist, run)
            }),
        ))
    }

    pub fn get(&self, pos: usize) -> u8 {
        if pos < self.preroll.len() {
            self.preroll[pos]
        } else {
            self.data[pos - self.preroll.len()]
        }
    }

    fn get_at_dist(&self, data_pos: usize, dist: u16) -> u8 {
        self.get(data_pos - usize::from(dist))
    }

    fn possible_run_length_at(&self, pos: usize, dist: u16) -> u16 {
        let upcoming_data_len =
            u16::try_from(258.min(self.data_len() - pos)).expect("logically sound");
        let upcoming_data: Vec<u8> = (0..upcoming_data_len)
            .map(|i| self.get(pos + usize::try_from(i).expect("todo: usize")))
            .collect();

        for cur in 3..dist.min(upcoming_data_len) {
            if upcoming_data[usize::try_from(cur).expect("todo: usize")]
                != self.get_at_dist(pos, dist - cur)
            {
                return cur;
            }
        }

        for cur in dist..upcoming_data_len {
            if upcoming_data[usize::try_from(cur % dist).expect("todo: usize")]
                != upcoming_data[usize::try_from(cur).expect("todo: usize")]
            {
                return cur;
            }
        }

        upcoming_data_len
    }
}

fn key_from_bytes(from: &[u8]) -> Key {
    Key {
        b0: from[0],
        b1: from[1],
        b2: from[2],
    }
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
        hash <<= 5;
        hash ^= u16::from(self.b1);
        hash <<= 5;
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
        writeln!(f, "{:?}", self.map)
    }
}

#[cfg(test)]
mod tests {
    use super::Key;

    use crate::Code;
    use crate::Ref;

    fn r(dist: u16, run: u16) -> Code {
        Code::Reference(Ref::new(dist, run))
    }

    #[test]
    fn hash_sixteen_16_collisions() {
        assert_eq!(0b0000_1100_0010_0001, k(&[3, 1, 1]).sixteen_hash_16());
        assert_eq!(
            k(&[b'O', b'o', b'o']).sixteen_hash_16(),
            k(&[b'o', b'o', b'o']).sixteen_hash_16()
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
