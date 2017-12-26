type Int = u16;
type Len = u16;
type Pair = (Int, Len);

// TODO: O(n^2) -> O(n) by zipping or using the sorted property or something
pub fn obscure<'i, F: 'i, B>(from: F, by: B) -> Box<Iterator<Item = Int> + 'i>
where
    F: Iterator<Item = Int>,
    B: Iterator<Item = Pair>,
{
    let by: Vec<Pair> = by.collect();

    //let from: Vec<Int> = from.collect();
    //assert_reverse_sorted(&from);
    //let from = from.into_iter()

    Box::new(from.filter(move |item| !contains(&by, *item)))
}

fn contains(haystack: &[Pair], needle: Int) -> bool {
    for &(start, len) in haystack {
        if needle >= start && needle <= (start + len as Int) {
            return true;
        }
    }

    return false;
}

fn assert_reverse_sorted(list: &[Int]) {
    let mut list = list.into_iter();
    let mut last = match list.next() {
        Some(x) => x,
        None => return,
    };

    for item in list {
        assert_lt!(item, last);
        last = item;
    }
}

#[cfg(test)]
mod tests {
    use super::assert_reverse_sorted;
    use super::obscure;
    use super::Int;

    #[test]
    fn reverse() {
        assert_reverse_sorted(&[4, 2, 1]);
    }

    #[test]
    #[should_panic]
    fn reverse_collide() {
        assert_reverse_sorted(&[4, 4, 1]);
    }

    #[test]
    #[should_panic]
    fn reverse_increase() {
        assert_reverse_sorted(&[4, 1, 3]);
    }

    #[test]
    fn obscured() {
        assert_eq!(
            &[6, 2],
            obscure([6, 4, 2].iter().cloned(), [(3, 2)].iter().cloned())
                .collect::<Vec<Int>>()
                .as_slice()
        );
    }
}
