use guesser::RefGuesser;
use errors::*;

use Code;
use Finder;
use Ref;
use usize_from;

pub fn greedy<F: Finder>(finder: &F, pos: usize) -> Vec<Code> {
    vec![
        match finder.best_candidate(pos) {
            (_, Some(r)) => Code::Reference(r),
            (b, None) => Code::Literal(b),
        },
    ]
}

//cursor.all_candidates().and_then(|candidates| {
//best(candidates.filter(move |r| usize_from(r.dist) != pos))
//})
pub fn gzip<F: Finder>(finder: &F, mut pos: usize) -> Vec<Code> {
    let mut ret = Vec::with_capacity(3);

    let mut current = match finder.best_candidate(pos) {
        (_, Some(start)) => start,
        (b, None) => return vec![Code::Literal(b)],
    };

    loop {
        pos += 1;
        current = match finder.best_candidate(pos) {
            (b, Some(new)) if new.run() > current.run() => {
                ret.push(Code::Literal(b));
                new
            }
            (_, None) | (_, Some(_)) => {
                ret.push(Code::Reference(current));
                break;
            }
        };
    }

    ret
}

pub fn three_zip<F: Finder>(finder: &F, pos: usize) -> Vec<Code> {
    let (first_literal, first_best) = match finder.best_candidate(pos) {
        // there's a good run, use it
        (_, Some(r)) if r.run() > 3 => return vec![r.into()],

        // there's a possibly bad run
        (l, Some(r)) => (l, r),

        // there's no run, or we're at the end: only a literal
        (b, None) => return vec![Code::Literal(b)],
    };

    assert_eq!(3, first_best.run());

    let (second_literal, mut second_best) = finder.best_candidate(pos + 1);
    second_best = second_best.filter(|x| x.run() > 3);

    // optimisation:
    if let Some(r) = second_best {
        if r.run() == 258 {
            // no point searching for a third run, as this will win.
            return vec![Code::Literal(first_literal), r.into()];
        }
    }

    let (third_literal, mut third_best) = finder.best_candidate(pos + 2);
    third_best = third_best.filter(|x| x.run() > 4);

    let third_result = |third_run: Ref| {
        vec![
            Code::Literal(first_literal),
            Code::Literal(second_literal),
            third_run.into(),
        ]
    };

    match second_best {
        Some(second_run) => match third_best {
            Some(third_run) if third_run.run() > second_run.run() => third_result(third_run),
            Some(_) | None => vec![Code::Literal(first_literal), second_run.into()],
        },
        None => match third_best {
            Some(third_run) => third_result(third_run),
            None => vec![first_best.into()],
        },
    }
}

pub fn best<I: Iterator<Item = Ref>>(mut candidates: I) -> Option<Ref> {
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
    use Code;
    use Code::Literal as L;
    use Ref;

    fn r(dist: u16, run: u16) -> Code {
        Code::Reference(Ref::new(dist, run))
    }

    #[test]
    fn best_in_the_right_order() {
        use super::best;
        assert_eq!(
            Some(Ref::new(2, 5)),
            best(vec![Ref::new(1, 3), Ref::new(2, 5)].into_iter())
        );
    }

    fn gzip(_: &[u8], _: &[u8]) -> ::errors::Result<Vec<Code>> {
        unimplemented!("TODO")
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

        assert_eq!(
            &[
                L(b'a'),
                L(b'1'),
                L(b'2'),
                L(b'3'),
                L(b'4'),
                L(b'1'),
                L(b'2'),
                L(b'f'),
                L(b'4'),
                r(8, 4)
            ],
            gzip(&[], b"a123412f41234").unwrap().as_slice()
        );
    }

}
