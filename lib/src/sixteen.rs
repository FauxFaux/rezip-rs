use bad_table::BadTable;

type Pos = u16;

const MAX_DIST: usize = 32 * 1024;

/// Find the longest match available by applying `table.pos_to_pos` to `cur_match` repeatedly,
/// up to `chain_limit` times. Returns (0, 0) if no matches found.
fn longest_match(mut cur_match: Pos, idx: Pos, window: &[u8], table: &BadTable) -> (Pos, u16) {
    let mut match_start: Pos = 0;

    let current_string = &window[idx as usize..];
    let mut best_len = 0;

    let chain_limit = 4;

    for _ in 0..chain_limit {
        let matched = &window[cur_match as usize..];

        if matched[0] == current_string[0] && matched[1] == current_string[1] {
            assert_eq!(matched[2], current_string[2], "hash table bug");

            let len = matching(matched, current_string);
            debug_assert!(len >= 3);

            if len > best_len {
                match_start = cur_match;
                best_len = len;
            }
        }

        cur_match = table.next_match(cur_match);
        if cur_match <= 0 {
            break;
        }
    }

    (match_start, best_len)
}

fn matching(left: &[u8], right: &[u8]) -> u16 {
    let shortest = left.len().min(right.len()).min(258) as u16;
    for i in 1..shortest {
        if left[i as usize] != right[i as usize] {
            return i;
        }
    }
    shortest
}

/// Compress like gzip(1) 1.6 --fast.
pub fn fast(window: &[u8]) {
    let mut table = BadTable::default();

    table.reinit_hash_at(window, 0);

    let mut idx = 0;

    while idx < (window.len() - 2) as u16 {
        // The pos of the last time we saw a string which hashes to the same thing.
        let prev_match_pos = table.insert_string(window, idx);

        // The location and length of the actually best match.
        let (match_start, match_length) =
            if 0 != prev_match_pos && idx - prev_match_pos <= (MAX_DIST as u16) {
                longest_match(prev_match_pos, idx, window, &table)
            } else {
                (0, 0)
            };

        if match_length < 3 {
            println!("{:3}: literal {:?}", idx, window[idx as usize] as char);
            idx += 1;
            continue;
        }

        println!("{:3}: match! {}, {}", idx, match_start, match_length);

        // If the match is short enough, add everything to the table.
        if match_length <= 4 {
            for _ in 0..match_length {
                idx += 1;

                // TODO: off-by-one somewhere here
                if idx == (window.len() - 2) as u16 {
                    break;
                }

                table.insert_string(window, idx);
            }
            idx += 1;
        } else {
            // otherwise, skip over the match, and reinitialise the hash at the other end
            idx += match_length;
            table.reinit_hash_at(window, idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fast;

    #[test]
    fn tiny_decay() {
        println!();
        fast(b"AoooAoooooOoooA");
    }

    #[test]
    fn colliding_back_miss() {
        println!();
        fast(b"1abcdef,bcdef-cdef");
    }
}
