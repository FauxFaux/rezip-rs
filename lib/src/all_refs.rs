use std::fmt;
use std::iter;

use back_map::BackMap;
use obscure::obscure;
use u16_from;
use usize_from;
use Obscure;
use Ref;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Key {
    b0: u8,
    b1: u8,
    b2: u8,
}

pub struct AllRefs<'p, 'd> {
    preroll: &'p [u8],
    pub data: &'d [u8],
    map: BackMap,
    limit: u16,
}

impl<'p, 'd> AllRefs<'p, 'd> {
    pub fn with_sixteen(preroll: &'p [u8], data: &'d [u8], limit: u16) -> Self {
        assert_eq!(&[0u8; 0], preroll, "not implemented");
        AllRefs {
            preroll,
            data,
            limit,
            map: BackMap::from_window(data),
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

        Some(Box::new(
            obscure(
                self.map.get(key).filter(move |&off| off < pos),
                obscura.iter().cloned(),
            ).take(self.limit as usize)
                .filter(move |&off| self.data[off..off + 3] == key.as_array()[..])
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

    use Code;
    use Ref;

    use Code::Literal as L;
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
