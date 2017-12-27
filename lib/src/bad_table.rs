use std::fmt;

use itertools::Itertools;

use all_refs::Key;
use usize_from;

const HASH_SIZE: usize = 32 * 1024;
const HASH_MASK: usize = ((1 << 15) - 1);
const HASH_SHIFT_MAGIC: u16 = 6;
const HASH_AHEAD_LENGTH: u16 = 2;

type Pos = u16;

pub struct NicerTable {
    /// A lookup from the current `hash` to the last `pos` we saw that hash at.
    hash_to_pos: [Pos; HASH_SIZE],

    /// A lookup from a `pos`, to the last `pos` where something had the same hash.
    pos_to_pos: [Pos; HASH_SIZE],
}

impl NicerTable {
    pub fn from_window(data: &[u8]) -> NicerTable {
        let mut table = NicerTable {
            hash_to_pos: [0; HASH_SIZE],
            pos_to_pos: [0; HASH_SIZE],
        };

        for (pos, keys) in data.into_iter()
            .cloned()
            .tuple_windows::<(u8, u8, u8)>()
            .enumerate()
        {
            let hash = Key::from(keys).sixteen_hash_16();
            let hash_entry = &mut table.hash_to_pos[hash as usize];
            let prev_pos = *hash_entry;
            table.pos_to_pos[pos] = prev_pos;
            *hash_entry = pos as u16;
        }

        table
    }

    pub fn get(&self, key: Key) -> Chain {
        let pos = self.hash_to_pos[usize_from(key.sixteen_hash_16())];

        Chain {
            next: Some(pos),
            pos_to_pos: &self.pos_to_pos,
        }
    }
}

pub struct Chain<'a> {
    next: Option<u16>,
    pos_to_pos: &'a [u16],
}

impl<'a> Iterator for Chain<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let current = match self.next {
            Some(val) => val,
            None => return None,
        };

        match self.pos_to_pos[usize_from(current)] {
            0 => self.next = None,
            next => self.next = Some(next),
        };

        Some(current)
    }
}

impl fmt::Debug for NicerTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (hash, &pos) in self.hash_to_pos
            .iter()
            .enumerate()
            .filter(|&(_, &pos)| 0 != pos)
        {
            let mut vals = Vec::new();
            let mut current = pos;
            vals.push(current);
            loop {
                current = self.pos_to_pos[usize_from(current)];
                if 0 == current {
                    break;
                }

                vals.push(current);
            }
            vals.reverse();
            writeln!(f, " - {:04x}: {:?}", hash, vals)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NicerTable;
    use super::BadTable;

    #[test]
    fn nicer() {
        //                       0123456
        let window: &[u8; 7] = b"oabcabc";
        let mut old = BadTable::default();
        old.reinit_hash_at(window, 0);
        for i in 0..(window.len() - 2) {
            old.insert_string(window, i as u16);
        }

        println!("{:?}", old);
        println!("{:?}", NicerTable::from_window(window));
    }
}
