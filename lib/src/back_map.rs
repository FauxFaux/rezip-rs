use std::fmt;

use cast::usize;

use itertools::Itertools;

use all_refs::Key;

const HASH_SIZE: usize = 32 * 1024;

/// This is an efficient way to compute and store a hashtable to an ordered list of positions.
pub struct BackMap {
    /// A lookup from the current `hash` to the last `pos` we saw that hash at.
    hash_to_pos: [usize; HASH_SIZE],

    /// A lookup from a `pos`, to the last `pos` where something had the same hash.
    pos_to_pos: Box<[usize]>,
}

impl BackMap {
    pub fn from_window(preroll: &[u8], data: &[u8]) -> BackMap {
        let mut table = BackMap {
            hash_to_pos: [0; HASH_SIZE],
            pos_to_pos: vec![0; preroll.len() + data.len()].into_boxed_slice(),
        };

        for (pos, keys) in preroll
            .into_iter()
            .chain(data.into_iter())
            .cloned()
            .tuple_windows::<(u8, u8, u8)>()
            .enumerate()
        {
            let hash = Key::from(keys).sixteen_hash_16();
            let hash_entry = &mut table.hash_to_pos[hash as usize];
            let prev_pos = *hash_entry;
            table.pos_to_pos[pos] = prev_pos;
            *hash_entry = pos;
        }

        table
    }

    pub fn get(&self, key: Key) -> Chain {
        let pos = self.hash_to_pos[usize(key.sixteen_hash_16())];

        Chain {
            next: Some(pos),
            pos_to_pos: &self.pos_to_pos,
        }
    }
}

pub struct Chain<'a> {
    next: Option<usize>,
    pos_to_pos: &'a [usize],
}

impl<'a> Iterator for Chain<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let current = match self.next {
            Some(val) => val,
            None => return None,
        };

        match self.pos_to_pos[current] {
            0 => self.next = None,
            next => self.next = Some(next),
        };

        Some(current)
    }
}

impl fmt::Debug for BackMap {
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
                current = self.pos_to_pos[current];
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
