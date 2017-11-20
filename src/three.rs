use std::mem;

/// Unlike Peekable, this is not lazy.
pub struct ThreePeek<I: Iterator> {
    inner: I,
    first: Option<I::Item>,
    second: Option<I::Item>,
    third: Option<I::Item>,
}

impl<I: Iterator> Iterator for ThreePeek<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        mem::replace(
            &mut self.first,
            mem::replace(
                &mut self.second,
                mem::replace(&mut self.third, self.inner.next()),
            ),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: Iterator> ThreePeek<I>
where
    I::Item: Copy,
{
    pub fn new(mut inner: I) -> Self {
        let first = inner.next();
        let second = inner.next();
        let third = inner.next();
        ThreePeek {
            inner,
            first,
            second,
            third,
        }
    }

    pub fn next_three(&mut self) -> Option<(I::Item, I::Item, I::Item)> {
        if let Some(first) = self.first {
            if let Some(second) = self.second {
                if let Some(third) = self.third {
                    self.next().unwrap();
                    return Some((first, second, third));
                }
            }
        }

        return None;
    }

    pub fn peek(&self) -> Option<I::Item> {
        self.first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three() {
        let a: Vec<u8> = (0..7).collect();
        let mut it = ThreePeek::new(a.into_iter());
        assert_eq!(Some(0), it.next());
        assert_eq!(Some((1, 2, 3)), it.next_three());
        assert_eq!(Some(2), it.next());
        assert_eq!(Some(3), it.next());
        assert_eq!(Some((4, 5, 6)), it.next_three());
        assert_eq!(None, it.next_three());
        assert_eq!(Some(5), it.next());
        assert_eq!(Some(6), it.next());
        assert_eq!(None, it.next());
    }
}
