use guesser::RefGuesser;
use errors::*;

use Code;
use Ref;
use usize_from;

pub fn gzip(preroll: &[u8], data: &[u8]) -> Result<Vec<Code>> {
    let mut ret = Vec::with_capacity(data.len() / 258);
    let guesser = RefGuesser::new(preroll, data);

    // if there's no saved run:
    //  * if there are no possible runs, emit a literal
    //  * if there is a run, save it
    // if there's a saved run:
    //  * if our best run is longer, emit a literal for the saved run, and overwrite it
    //  * if our best run is shorter (possibly zero), emit the saved run;
    //       it's guaranteed to squash this byte

    let mut saved: Option<Ref> = None;

    let mut pos = 0usize;
    while pos < data.len() {
        let cursor = guesser.at(pos);
        let best = cursor
            .all_candidates()
            .map(|candidates| {
                // first byte bug
                candidates.filter(move |r| usize_from(r.dist) != pos)
            })
            .and_then(best);

        println!("{}: saved: {:?} best: {:?}", pos, saved, best);

        match best {
            Some(best) => match saved {
                Some(saved_ref) if best.run() > saved_ref.run()  => {
                    // there's a new ref here, and it's better.
                    // emit a literal for the saved ref, and save this instead.
                    ret.push(Code::Literal(data[pos]));
                    saved = Some(best);
                    pos += 1;
                }
                Some(saved_ref) => {
                    // the old ref was better than this ref, so emit it directly;
                    // discarding these matches as they're bad
                    ret.push(Code::Reference(saved_ref));
                    saved = None;
                    pos += usize_from(saved_ref.run());
                }
                None => {
                    // nothing saved, so just save this run
                    saved = Some(best);
                    pos += 1;
                }
            }
            None => match saved {
                Some(saved_ref) => {
                    // no run here, if we have a saved ref, it's the one
                    ret.push(Code::Reference(saved_ref));
                    saved = None;
                    pos += usize_from(saved_ref.run());
                }
                None => {
                    // no run saved, and no run found, can only be a literal
                    ret.push(Code::Literal(data[pos]));
                    pos += 1;
                }
            },
        }
    }

    assert!(saved.is_none());

    Ok(ret)
}

fn best<I: Iterator<Item = Ref>>(mut candidates: I) -> Option<Ref> {
    let mut best = match candidates.next() {
        Some(r) => r,
        None => return None,
    };

    if best.run() == 258 {
        return Some(best);
    }

    for candidate in candidates {
        if best.run() > candidate.run() {
            best = candidate;
        }

        if best.run() == 258 {
            return Some(best);
        }
    }

    Some(best)
}

#[cfg(test)]
mod tests {
    use super::gzip;
    use Code;
    use Code::Literal as L;
    use Ref;

    fn r(dist: u16, run: u16) -> Code {
        Code::Reference(Ref::new(dist, run))
    }

    #[test]
    fn gzip_simple_ref() {
        assert_eq!(
            &[L(b'a'), L(b'a'), L(b'b'), L(b'c'), r(3, 3)],
            gzip(b"", b"aabcabc",).unwrap().as_slice()
        )
    }

    #[test]
    fn gzip_simple_ref_then() {
        assert_eq!(
            &[L(b'a'), L(b'a'), L(b'b'), L(b'c'), r(3, 3), L(b'd')],
            gzip(b"", b"aabcabcd",).unwrap().as_slice()
        )
    }

    #[test]
    fn gzip_first_byte_bug() {
        assert_eq!(
            &[L(b'a'), L(b'b'), L(b'c'), L(b'a'), L(b'b'), L(b'c')],
            gzip(b"", b"abcabc",).unwrap().as_slice()
        )
    }

    #[test]
    fn gzip_longer() {
        assert_eq!(
            &[
                L(b'a'),
                L(b'1'),
                L(b'2'),
                L(b'3'),
                L(b'4'),
                r(4, 3),
                r(7, 4)
            ],
            gzip(b"", b"a12341231234",).unwrap().as_slice()
        )
    }
}
