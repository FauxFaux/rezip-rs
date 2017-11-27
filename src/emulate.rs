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

    let mut saved = None;

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
        match saved {
            None => match best {
                Some(r) => {
                    // there aren't any on going runs, so stash this run
                    saved = Some(r);
                }
                None => {
                    // there's nothing saved and nothing here, we can just emit it
                    ret.push(Code::Literal(data[pos]));
                }
            },
            Some(saved_run) => {
                // something's saved, inspect the new run
                let new_run = best.map(|r| r.run()).unwrap_or(0);
                if new_run > saved_run.run() {
                    // it's better. Emit a literal for the old run, and save this one instead
                    saved = Some(best.unwrap());
                    ret.push(Code::Literal(data[pos - 1]));
                } else {
                    // it's not better. Emit the old run, and save this run.
                    ret.push(Code::Reference(saved_run));
                    match best {
                        Some(new_run) => {
                            saved = Some(new_run);
                        }
                        None => {
                            saved = None;
                            ret.push(Code::Literal(data[pos]));
                        }
                    }

                    pos += usize_from(saved_run.run() - 1);
                }
            }
        }

        pos += 1;
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
        if candidate.run() > best.run() {
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
            &[L(b'a'), L(b'1'), L(b'2'), L(b'3'), L(b'4'), r(4, 3), r(7, 4)],
            gzip(b"", b"a12341231234",).unwrap().as_slice()
        )
    }
}
