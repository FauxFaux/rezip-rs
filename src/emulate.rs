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
        let best = match cursor.all_candidates() {
            Some(candidates) => best(candidates.filter(move |r| usize_from(r.dist) != pos)),
            None => break,
        };

        println!("{}: saved: {:?} best: {:?}", pos, saved, best);

        match best {
            Some(best) => match saved {
                Some(saved_ref) if best.run() > saved_ref.run() => {
                    // there's a new ref here, and it's better.
                    // emit a literal for the saved ref, and save this instead.
                    println!(" - better: literal {:?}, saving {:?}, +1", data[pos], best);
                    ret.push(Code::Literal(data[pos - 1]));
                    saved = Some(best);
                    pos += 1;
                }
                Some(saved_ref) => {
                    // the old ref was better than this ref, so emit it directly;
                    // discarding these matches as they're bad
                    println!(" - worse: taking {:?}, dropping, +run", saved_ref);
                    ret.push(Code::Reference(saved_ref));
                    saved = None;
                    pos += usize_from(saved_ref.run() - 1);
                }
                None => {
                    println!(" - just us: saving {:?}, +1", best);
                    // nothing saved, so just save this run
                    saved = Some(best);
                    pos += 1;
                }
            },
            None => match saved {
                Some(saved_ref) => {
                    // no run here, if we have a saved ref, it's the one
                    println!(" - nothing here, using saved: {:?}, dropping, +run", saved_ref);
                    ret.push(Code::Reference(saved_ref));
                    saved = None;
                    pos += usize_from(saved_ref.run()) - 1;
                }
                None => {
                    println!(" - nothing here, nothing saved, lit: {:?}, +1", data[pos]);
                    // no run saved, and no run found, can only be a literal
                    ret.push(Code::Literal(data[pos]));
                    pos += 1;
                }
            },
        }
    }

    if let Some(saved_ref) = saved {
        ret.push(Code::Reference(saved_ref));
        pos += usize_from(saved_ref.run());
    }

    while pos < data.len() {
        ret.push(Code::Literal(data[pos]));
        pos += 1;
    }

    Ok(ret)
}

fn best<I: Iterator<Item = Ref>>(mut candidates: I) -> Option<Ref> {
    let mut best = match candidates.next() {
        Some(r) => r,
        None => return None,
    };

    for candidate in candidates {
        if candidate.run() > best.run() {
            best = candidate;
        }

        if best.run() == 258 {
            break;
        }
    }

    if best.dist > 4096 && 3 == best.run() {
        None
    } else {
        Some(best)
    }
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
    fn best_in_the_right_order() {
        use super::best;
        assert_eq!(Some(Ref::new(2, 5)), best(vec![Ref::new(1, 3), Ref::new(2, 5)].into_iter()));
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

    #[test]
    fn lazy_longer_ref() {
        // Finally, a test for this gzip behaviour.
        // It only does this with zip levels >3, including the default.

        // a123412f41234
        // 0123456789012

        // It gets to position 8, and it's ignoring the "412" (at position 6),
        // instead taking the longer run of "1234" at position 1.

        // I bet it thinks it's so smart.

        assert_eq!(&[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'4'),
            L(b'1'),
            L(b'2'),
            L(b'f'),
            L(b'4'),
            r(8, 4),
        ], gzip(&[], b"a123412f41234").unwrap().as_slice());
    }

}
