use usize_from;

const HASH_SIZE: usize = 32 * 1024;
const HASH_MASK: usize = ((1 << 15) - 1);
const HASH_SHIFT_MAGIC: u16 = 6;
const HASH_AHEAD_LENGTH: u16 = 2;

type Pos = u16;

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
