// Work out the set of supported algorithms:
// * Fail at first byte.
// * Seek back up to N.
// * Miss encoding a location, and encode the next one.
// * Skip further back to find a longer code. Maintain the code lengths lengths?
// decode the thing symbol by symbol
// If a mode's decision isn't taken, drop that mode from the possible set of modes.
// If no modes are left, we didn't work.
// If any modes are left, pick the "simplest", and return it.

// Still need to fully decode the input, and store the whole backref search buffer.
// Can we use the same buffer? Probably too complex for first pass.

// Do we need to rearrange the api so we can process a sequence and its decoded bytes?

use circles::CircularBuffer;
use guess;
use errors::*;
use Code;

trait Algo {
    fn accept(&mut self, code: &Code, dictionary: &CircularBuffer) -> Result<bool>;
}

pub fn trace(preroll: &[u8], codes: &[Code]) -> Result<()> {
    ensure!(!codes.is_empty(), "unexpected empty block");

    let window_size = guess::max_distance(codes).unwrap();
    let (outside, hits_first_byte) = guess::outside_range_or_hit_zero(codes);

    let first_byte_bug = preroll.is_empty() && !hits_first_byte;

    let mut dictionary = CircularBuffer::with_capacity(32 * 1024);
    dictionary.extend(preroll);

    let mut it = codes.iter().peekable();

    let first = it.next().unwrap();

    let target = match *first {
        Code::Literal(byte) => {
            dictionary.push(byte);

            // if we'd find a Reference here, then we're in trouble and we need to enable SKIPPY
            // return and try another method?

            vec![byte]
        }
        Code::Reference { dist, run_minus_3 } => {
            let run = ::unpack_run(run_minus_3);
            let mut v = vec![];
            dictionary.copy(dist, run, &mut v)?;
            v
        }
    };


    unimplemented!()
}
