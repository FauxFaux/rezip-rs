use std::iter;

use circles::CircularBuffer;
use guesser::RefGuesser;
use serialise;
use Code;
use usize_from;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Trace {
    Correct,
    Actual(Code),
}

pub fn trace<F>(preroll: &[u8], codes: &[Code], guesser: F) -> Vec<Trace>
where
    F: Fn(&RefGuesser, usize) -> Vec<Code>,
{
    let mut ret = Vec::with_capacity(codes.len());

    let mut data = Vec::with_capacity(codes.len());
    {
        let mut prebuf = CircularBuffer::with_capacity(32 * 1024);
        prebuf.extend(preroll);
        serialise::decompressed_codes(&mut data, &mut prebuf, codes).unwrap();
    }

    let rg = RefGuesser::new(preroll, &data);
    let mut pos = 0;

    let mut codes = codes.into_iter().peekable();

    while pos < rg.data_len() {
        let guesses = guesser(&rg, pos);
        assert!(!guesses.is_empty());

        let matches = shared_prefix(&guesses, &mut codes);
        for matched in matches {
            ret.push(Trace::Correct);
            pos += usize_from(matched.emitted_bytes());
        }

        if matches.len() == guesses.len() {
            continue;
        }

        match codes.next() {
            Some(code) => {
                ret.push(Trace::Actual(*code));
                pos += usize_from(code.emitted_bytes());
            }
            None => panic!("the guesser guessed more than there actually are?"),
        }
    }

    ret
}

pub fn restore<F>(preroll: &[u8], data: &[u8], trace: &[Trace], guesser: F) -> Vec<Code>
where
    F: Fn(&RefGuesser, usize) -> Vec<Code>,
{
    let mut ret = Vec::with_capacity(trace.len());

    let rg = RefGuesser::new(preroll, data);
    let mut pos = 0;

    let mut trace = trace.into_iter().peekable();

    while pos < rg.data_len() {
        let guesses = guesser(&rg, pos);
        assert!(!guesses.is_empty());

        for guess in guesses {
            let hint = *trace.next().expect("not out of data");
            let orig = match hint {
                Trace::Correct => guess,
                Trace::Actual(other) => other,
            };

            pos += usize_from(orig.emitted_bytes());
            ret.push(orig);

            if let Trace::Actual(_) = hint {
                // the guesser was wrong, and we moved in a way it doesn't understand; ignore it
                break;
            }
        }
    }

    ret
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
        let mut it = [1, 2].into_iter().peekable();
        assert_eq!(&[1, 2], shared_prefix(&[1, 2], &mut it));
        assert!(it.next().is_none());

        let mut it = [1, 2].into_iter().peekable();
        assert_eq!(&[0usize; 0], shared_prefix(&[], &mut it));
        assert_eq!(Some(&1), it.next());

        let mut it = [1, 2].into_iter().peekable();
        assert_eq!(&[1], shared_prefix(&[1, 5, 7], &mut it));
        assert_eq!(Some(&2), it.next());

        assert!(shared_prefix(&[1, 5, 7], &mut iter::empty().peekable()).is_empty());
    }

    #[test]
    fn trace_dumb() {
        use Code;
        use super::trace;
        use super::Trace::Correct as C;
        use super::Trace::Actual as A;
        assert_eq!(
            vec![A(Code::Literal(b'a'))],
            trace(
                &[],
                &[Code::Literal(b'a')],
                |_, _| vec![Code::Literal(b'N')]
            )
        );

        assert_eq!(
            vec![C],
            trace(
                &[],
                &[Code::Literal(b'a')],
                |_, _| vec![Code::Literal(b'a')]
            )
        );
    }
}
