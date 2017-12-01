fn capped_max_by<F, T, C: Eq + Ord, I: Iterator<Item = T>>(mut it: I, cap: C, func: F) -> Option<T>
    where F:
    Fn(&T) -> C,
{
    let mut max = match it.next() {
        Some(val) => val,
        None => return None,
    };

    let mut max_score = func(&max);

    for candidate in it {
        let candidate_score = func(&candidate);
        if candidate_score > max_score {
            max = candidate;
            max_score = candidate_score;

            if cap == max_score {
                break;
            }
        }
    }

    Some(max)
}