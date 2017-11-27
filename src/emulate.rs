use guesser::RefGuesser;
use errors::*;

use Code;
use Ref;
use usize_from;

fn gzip(preroll: &[u8], data: &[u8]) -> Result<Vec<Code>> {
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
        let best = cursor.all_candidates().and_then(best);
        match saved {
            None => match best {
                Some(r) => {
                    saved = Some(r);
                }
                None => {
                    ret.push(Code::Literal(data[pos]));
                }
            },
            Some(r) => {
                let new_run = best.map(|r| r.run()).unwrap_or(0);
                if new_run > r.run() {
                    saved = Some(r);
                    ret.push(Code::Literal(data[pos]));
                } else {
                    ret.push(Code::Reference(r));
                    pos += usize_from(r.run() - 1);
                }
            }
        }

        pos += 1;
    }

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
    fn simple() {
        assert_eq!(
            &[L(b'a'), L(b'a'), L(b'b'), L(b'c'), r(3, 3)],
            gzip(b"", b"aabcabc",).unwrap().as_slice()
        )
    }

    #[test]
    fn first_byte_bug() {
        assert_eq!(
            &[L(b'a'), L(b'b'), L(b'c'), L(b'a'), L(b'b'), L(b'c')],
            gzip(b"", b"abcabc",).unwrap().as_slice()
        )
    }
}
