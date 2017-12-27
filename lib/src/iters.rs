pub fn capped_max_by<F, T, C: Eq + Ord, I: Iterator<Item = T>>(
    mut it: I,
    cap: &C,
    func: F,
) -> Option<T>
where
    F: Fn(&T) -> C,
{
    let mut max = match it.next() {
        Some(val) => val,
        None => return None,
    };

    let mut max_score = func(&max);

    if max_score >= *cap {
        return Some(max);
    }

    for candidate in it {
        let candidate_score = func(&candidate);
        if candidate_score > max_score {
            max = candidate;
            max_score = candidate_score;

            if max_score >= *cap {
                break;
            }
        }
    }

    Some(max)
}

#[cfg(test)]
mod tests {
    #[test]
    fn first_item() {
        use super::capped_max_by;
        let data = [5u64, 6, 7];
        assert_eq!(Some(5), capped_max_by(data.iter().cloned(), &4, |&x| x));
        assert_eq!(Some(5), capped_max_by(data.iter().cloned(), &5, |&x| x));
        assert_eq!(Some(6), capped_max_by(data.iter().cloned(), &6, |&x| x));
        assert_eq!(Some(7), capped_max_by(data.iter().cloned(), &7, |&x| x));
        assert_eq!(Some(7), capped_max_by(data.iter().cloned(), &128, |&x| x));
    }
}
