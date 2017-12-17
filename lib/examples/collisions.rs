#![feature(inclusive_range_syntax)]
extern crate itertools;
extern crate librezip;

use std::collections::HashMap;
use std::collections::HashSet;
use std::u16;

use itertools::Itertools;
use librezip::all_refs::Key;

fn main() {
    let mut map = HashMap::with_capacity(u16::MAX as usize);

    for a in b' '..=b'~' {
        for b in b' '..=b'~' {
            for c in b' '..=b'~' {
                let key = Key::from((a, b, c));
                map.entry(key.sixteen_hash_16())
                    .or_insert_with(|| HashSet::new())
                    .insert((a, b, c));
            }
        }
    }

    for (_, set) in map {
        if 1 == set.len() {
            continue;
        }
        let mut parts: Vec<String> = set.into_iter()
            .map(|(a, b, c)| format!("{}{}{}", a as char, b as char, c as char))
            .collect();
        if 2 == parts.len() {
            if parts[0][1..] == parts[1][1..] {
                continue;
            }
        }
        parts.sort();
        println!("{} {}", parts.len(), parts.join(", "));
    }
}
