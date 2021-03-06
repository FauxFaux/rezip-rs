use std::iter;

use crate::technique::Technique;
use crate::Code;
use crate::Guesser;
use crate::Trace;

pub fn trace(codes: &[Code], technique: &Technique) -> Vec<Trace> {
    let mut ret = Vec::with_capacity(codes.len());

    let mut codes = codes.into_iter().peekable();
    let mut scanner = technique.scanner();

    while scanner.more_data() {
        let guesses = scanner.codes();
        assert!(!guesses.is_empty());

        let matches = shared_prefix(&guesses, &mut codes);
        for matched in matches {
            ret.push(Trace::Correct);
            scanner.feedback(*matched);
        }

        if matches.len() == guesses.len() {
            continue;
        }

        match codes.next() {
            Some(&code) => {
                ret.push(match code {
                    Code::Literal(_) => Trace::ActuallyLiteral,
                    Code::Reference(r) => Trace::Actually(r),
                });
                scanner.feedback(code);
            }
            None => panic!("the guesser guessed more than there actually are?"),
        }
    }

    ret
}

pub fn restore(trace: &[Trace], technique: &Technique) -> Vec<Code> {
    let mut ret = Vec::with_capacity(trace.len());

    let mut trace = trace.into_iter().peekable();
    let mut scanner = technique.scanner();

    while scanner.more_data() {
        let guesses = scanner.codes();
        assert!(!guesses.is_empty());

        for guess in guesses {
            let hint = *trace.next().expect("not out of data");
            let orig = match hint {
                Trace::Correct => guess,
                Trace::Actually(r) => Code::Reference(r),
                Trace::ActuallyLiteral => Code::Literal(technique.byte_at(scanner.pos)),
            };

            scanner.feedback(orig);
            ret.push(orig);

            match hint {
                Trace::ActuallyLiteral | Trace::Actually(_) => {
                    // the guesser was wrong, and we moved in a way it doesn't understand; ignore it
                    break;
                }
                Trace::Correct => {}
            }
        }
    }

    ret
}

pub fn validate(codes: &[Code], technique: &Technique) -> Vec<Trace> {
    let trace = trace(codes, technique);
    let restored = restore(&trace, technique);

    assert_eq!(codes, restored.as_slice());

    trace
}

fn shared_prefix<'l, 't, T: 't + Eq, I: Iterator<Item = &'t T>>(
    left: &'l [T],
    right: &mut iter::Peekable<I>,
) -> &'l [T] {
    for end in 0..left.len() {
        match right.peek() {
            Some(val) if **val == left[end] => {}
            None | Some(_) => return &left[..end],
        }

        right.next();
    }

    left
}

#[cfg(test)]
mod tests {
    use std::iter;

    #[test]
    fn prefix() {
        use super::shared_prefix;
        let mut it = [1, 2].iter().peekable();
        assert_eq!(&[1, 2], shared_prefix(&[1, 2], &mut it));
        assert!(it.next().is_none());

        let mut it = [1, 2].iter().peekable();
        assert_eq!(&[0usize; 0], shared_prefix(&[], &mut it));
        assert_eq!(Some(&1), it.next());

        let mut it = [1, 2].iter().peekable();
        assert_eq!(&[1], shared_prefix(&[1, 5, 7], &mut it));
        assert_eq!(Some(&2), it.next());

        assert!(shared_prefix(&[1, 5, 7], &mut iter::empty().peekable()).is_empty());
    }
}
