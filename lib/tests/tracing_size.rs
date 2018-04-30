extern crate itertools;
extern crate librezip;

use std::io;

use itertools::Itertools;

use librezip::Block;
use librezip::CircularBuffer;
use librezip::Trace;

fn run_gzip(level: u8, file: &[u8]) -> Vec<Vec<Trace>> {
    let mut reader = io::Cursor::new(file);
    librezip::gzip::discard_header(&mut reader).unwrap();

    let mut dictionary = CircularBuffer::new();
    let mut parts = Vec::new();

    for block in librezip::parse_deflate(&mut reader) {
        let codes = match block.unwrap() {
            Block::Uncompressed(_) => unimplemented!(),
            Block::DynamicHuffman { codes, .. } | Block::FixedHuffman(codes) => codes,
        };

        let preroll = &dictionary.vec();
        let mut data: Vec<u8> = Vec::with_capacity(codes.len());
        librezip::decompressed_codes(&mut data, &mut dictionary, &codes).unwrap();

        parts.push(librezip::tracer::try_gzip(level, preroll, &data, &codes));
    }

    parts
}

fn try_gzip(level: u8, file: &[u8]) {
    let parts = run_gzip(level, file);
    for (id, part) in parts.iter().enumerate() {
        if !part.iter().all(|&x| Trace::Correct == x) {
            panic!(
                "part {}: must be fully complete: {}",
                id,
                part.iter().map(|x| format!("{:?}", x)).join("")
            );
        }
    }
}

// tiny-decay:
// 1abcdef,bcdef-cdef
// 012345678901234567
// LLLLLLLLSRRRRLSRRR
// 1: -----[6,5]-[11,4]
// 3: -----[6,5]-[5,4]
#[test]
fn tiny_decay_1_1() {
    try_gzip(1, include_bytes!("data/tiny-decay-sixteen-1.gz"))
}

#[test]
fn tiny_decay_3_3() {
    try_gzip(3, include_bytes!("data/tiny-decay-sixteen-3.gz"))
}

// decaying: S='defghijklm'; printf "0.abcdefg_hijklm,1.abc$S,2.bc$S,3.c$S,4.$S"
// decaying: 0.abcdefg_hijklm,1.abcdefghijklm,2.bcdefghijklm,3.cdefghijklm,4.defghijklm
//           01234567890123456789012345678901234567890123456789012345678901234567890123
//           0         1         2         3         4         5         6         7
//           LLLLLLLLLLLLLLLLL1[ 17,8 ][16, 7]2.[32,6][15, 7]3.[46,5[14, 7]4.59,4[13,6]
//                               1      10        2    26        3   41       4   54
//                     ^----------------`              |             |            |
//                                     ^---------------`             |            |
//                                                    ^--------------`            |
//                                                                  ^-------------`
#[test]
fn decaying_1_1() {
    try_gzip(1, include_bytes!("data/decaying-sixteen-1.gz"))
}

#[test]
fn decaying_1_2() {
    //    assert_eq!(7,
    try_gzip(2, include_bytes!("data/decaying-sixteen-1.gz"))
}

#[test]
fn decaying_1_3() {
    //    assert_eq!(
    //        12,
    try_gzip(3, include_bytes!("data/decaying-sixteen-1.gz"))
}

#[test]
fn decaying_2_2() {
    try_gzip(2, include_bytes!("data/decaying-sixteen-2.gz"))
}

#[test]
fn decaying_3_3() {
    try_gzip(3, include_bytes!("data/decaying-sixteen-3.gz"))
}

// four-ref:
// gzip -1 has maximum chain length of 4, so can't see the cat1 5 'cat' steps back,
// so has to go with the shorter match in more recent history.
// 0cat1cat2cat3cat4cat5cat1
// 0123456789012345678901234
// LLLLLR:4LR:4LR:4LR:4LR:20
// 1: ------------------[4,3]
// 2: ------------------[20,4]
#[test]
fn four_ref_1_1() {
    try_gzip(1, include_bytes!("data/four-ref-sixteen-1.gz"))
}

#[test]
fn four_ref_2_2() {
    try_gzip(2, include_bytes!("data/four-ref-sixteen-2.gz"))
}

// ten-nine:
// gzip -1 misses the backref further back, possibly due to chain length?
// I would expect the dict to only contain "aaa" at 1[L] and 2[the first run],
// which is significantly less than four.
//   aaaaaaaaaabaaaaaaaaa
//   01234567890123456789
// 1 LLR[-1, 8]LR[-9, 8]L
// 2 LLR[-1, 8]LR[-10, 9]
#[test]
fn ten_nine_1_1() {
    try_gzip(1, include_bytes!("data/ten-nine-sixteen-1.gz"))
}

#[test]
fn ten_nine_2_2() {
    try_gzip(2, include_bytes!("data/ten-nine-sixteen-2.gz"))
}

// dumb-collision
// We had a bug where we would not check hash collisions for actual equality.
// '1' and 'a' happen to collide.
//   _1aaa
// 1 LLLLL
#[test]
fn dumb_collision() {
    try_gzip(1, include_bytes!("data/dumb-collision-1.gz"))
}

// colliding-back-miss
// 'Ooo' collides with 'ooo', which causes some unspecified chaos,
// changing the 'O' to nearly any other character causes the 'oooA'
// second backreference to be correctly detected as a 10,4, instead
// of the 4,3 shown here:
// 012345678901234
// AoooAoooooOoooA
// LLLLL4,3LLL4,3L
// _123411156112__
// As there are no long runs, everything gets inserted into the dictionary? !(3 > 4).
// Maximum dictionary depth: 4. Unclear when this rule is applied; before or after de-collisions.
// According to the above understanding, it would work if you changed 4's 'A' to a non-colliding
// character, but it doesn't, so clearly something else is missing.
#[test]
fn colliding_back_miss() {
    try_gzip(1, include_bytes!("data/colliding-back-miss-sixteen-1.gz"))
}

// lots of collisions here, but gzip looks back further than we think it should
//  input: woooooOooogooooo
//    idx: 0123456789012345
//   coll: _XXX__XX__XXXX--
//  gz -1: LL1, 4L4,3L9, 4L
//          ^_^___`   |
//           ^________`
//  trace thinks this ^ reference should be to idx 7, only 3 bytes long,
// but gzip can see further back. To get from idx 11 to idx 2, you have
// to skip:
//  * 10: not an actual match (goo)
//  *  7: valid
//  *  6: not an actual match (Ooo)
//  *  3: valid
//  *  2: found it!
//  *  1: this would be even better, but we can't see it
// .. but we're only allowed four inspections. Does it pre-filter for not actual matches,
// or are we hashing wrong? Or are we off-by-one?
//
// Or is the [1, 4] run not getting inserted into the dictionary,
// so the actual search goes 10, 7, 6, 2? No, as idx 3, definitely occluded by the match,
// is found before we hit the other bug.
#[test]
fn woo_goo() {
    try_gzip(1, include_bytes!("data/woo-goo-1.gz"))
}

// First 37,848 bytes of hard.rs; which results in running 7 records (11 bytes?)
// past the end of a block
#[test]
fn blockandabit_hard() {
    try_gzip(1, include_bytes!("data/blockandabit-sixteen-1.gz"))
}

#[ignore]
#[test]
fn blockandabit_newlines() {
    try_gzip(1, include_bytes!("data/blockandabitnewlines-sixteen-1.gz"))
}
