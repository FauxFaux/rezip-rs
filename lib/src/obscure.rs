use crate::Obscure;

type Int = usize;

// TODO: O(n^2) -> O(n) by zipping or using the sorted property or something
// from will be reverse-sorted (5, 4, 2, 1). by will be forward sorted (1, 7), (12, 3), ...
pub fn obscure<'i, F: 'i, B>(from: F, by: B) -> Box<dyn Iterator<Item = Int> + 'i>
where
    F: Iterator<Item = Int>,
    B: Iterator<Item = Obscure>,
{
    let by: Vec<Obscure> = by.collect();

    //let from: Vec<Int> = from.collect();
    //assert_reverse_sorted(&from);
    //let from = from.into_iter();

    Box::new(from.filter(move |item| !contains(&by, *item)))
}

fn contains(haystack: &[Obscure], needle: Int) -> bool {
    for &(start, len) in haystack {
        if needle > start && needle < (start + len as Int) {
            #[cfg(feature = "tracing")]
            println!("S{},{} obscures {}", start, len, needle);
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::obscure;
    use super::Int;

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
