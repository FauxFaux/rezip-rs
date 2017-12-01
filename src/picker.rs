use iters;
use Ref;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Picker {
    Longest,
    DropFarThrees,
}

impl Picker {
    pub fn picker<I: Iterator<Item = Ref>>(&self, candidates: I) -> Option<Ref> {
        match *self {
            Picker::Longest => longest(candidates),
            Picker::DropFarThrees => drop_far_threes(candidates),
        }
    }
}

fn longest<I: Iterator<Item = Ref>>(candidates: I) -> Option<Ref> {
    iters::capped_max_by(candidates, 258, |r| r.run())
}

fn drop_far_threes<I: Iterator<Item = Ref>>(candidates: I) -> Option<Ref> {
    longest(candidates).filter(|r| r.run() > 3 || r.dist <= 4096)
}

#[cfg(test)]
mod tests {
    use Ref;

    #[test]
    fn longest_in_the_right_order() {
        use super::longest;
        assert_eq!(
            Some(Ref::new(2, 5)),
            longest(vec![Ref::new(1, 3), Ref::new(2, 5)].into_iter())
        );
    }
}
