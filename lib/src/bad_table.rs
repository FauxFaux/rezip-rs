use std::fmt;

use itertools::Itertools;

use all_refs::Key;
use usize_from;

const HASH_SIZE: usize = 32 * 1024;
const HASH_MASK: usize = ((1 << 15) - 1);
const HASH_SHIFT_MAGIC: u16 = 6;
const HASH_AHEAD_LENGTH: u16 = 2;

type Pos = u16;

/// This is an efficient way to compute and store an ordered hashtable to a list of positions.
pub struct BadTable {
    /// A lookup from the current `hash` to the last `pos` we saw that hash at.
    hash_to_pos: [Pos; HASH_SIZE],

    /// A lookup from a `pos`, to the last `pos` where something had the same hash.
    pos_to_pos: [Pos; HASH_SIZE],

    /// The current hash, maintianed over [idx..idx+3], e.g. into the future.
    hash: u16,
}

impl BadTable {
    /// Insert a value into the table at the correct hash, given the idx.
    /// Observes only `window[idx]` and `window[idx+2]`, i.e. `idx` must be `< window.len() - 2`.
    /// Must be called monotonically from where `reinit_hash_at` was last called.
    pub fn insert_string(&mut self, window: &[u8], idx: Pos) -> u16 {
        self.push_from_index(window, idx + HASH_AHEAD_LENGTH);
        let head_of_chain = self.hash_to_pos[self.hash as usize];
        self.pos_to_pos[(idx as usize) & HASH_MASK] = head_of_chain;
        self.hash_to_pos[self.hash as usize] = idx;

        head_of_chain
    }

    /// Walk backwards through the chain to the last place a match was seen,
    /// given the current match.
    pub fn next_match(&self, cur_match: Pos) -> Pos {
        self.pos_to_pos[(cur_match & HASH_MASK as u16) as usize]
    }

    pub fn reinit_hash_at(&mut self, window: &[u8], start: u16) {
        self.hash = 0;
        self.push_from_index(window, start);
        self.push_from_index(window, start + 1);
    }

    fn push_from_index(&mut self, window: &[u8], idx: Pos) {
        let chr = window[usize_from(idx)];
        self.hash = ((self.hash << HASH_SHIFT_MAGIC) ^ chr as u16) & (HASH_MASK as u16)
    }
}

impl Default for BadTable {
    fn default() -> Self {
        BadTable {
            pos_to_pos: [0; HASH_SIZE],
            hash_to_pos: [0; HASH_SIZE],
            hash: 0,
        }
    }
}

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
            pos,
            pos_to_pos: &self.pos_to_pos,
        }
    }
}

pub struct Chain<'a> {
    pos: u16,
    pos_to_pos: &'a [u16],
}

impl<'a> Iterator for Chain<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        if 0 == self.pos {
            return None;
        }

        self.pos = self.pos_to_pos[usize_from(self.pos)];

        if 0 == self.pos {
            None
        } else {
            Some(self.pos)
        }
    }
}

fn write(f: &mut fmt::Formatter, hash_to_pos: &[u16], pos_to_pos: &[u16]) -> fmt::Result {
    for (hash, &pos) in hash_to_pos.iter().enumerate().filter(|&(_, &pos)| 0 != pos) {
        let mut vals = Vec::new();
        let mut current = pos;
        vals.push(current);
        loop {
            current = pos_to_pos[usize_from(current)];
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

impl fmt::Debug for BadTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write(f, &self.hash_to_pos, &self.pos_to_pos)
    }
}

impl fmt::Debug for NicerTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write(f, &self.hash_to_pos, &self.pos_to_pos)
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
