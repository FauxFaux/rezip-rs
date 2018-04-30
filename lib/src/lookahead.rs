use Code;
use Looker;
use Ref;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Lookahead {
    Greedy,
    Gzip,
    ThreeZip,
}

impl Lookahead {
    pub fn lookahead<L: Looker>(&self, looker: &L, pos: usize) -> Vec<Code> {
        match *self {
            Lookahead::Greedy => greedy(looker, pos),
            Lookahead::Gzip => gzip(looker, pos),
            Lookahead::ThreeZip => three_zip(looker, pos),
        }
    }
}

fn greedy<L: Looker>(looker: &L, pos: usize) -> Vec<Code> {
    vec![match looker.best_candidate(pos) {
        (_, Some(r)) => Code::Reference(r),
        (b, None) => Code::Literal(b),
    }]
}

fn gzip<L: Looker>(looker: &L, mut pos: usize) -> Vec<Code> {
    let mut ret = Vec::with_capacity(3);

    let (mut curr_lit, mut curr_ref) = match looker.best_candidate(pos) {
        (lit, Some(start)) => (lit, start),
        (b, None) => return vec![Code::Literal(b)],
    };

    loop {
        pos += 1;
        match looker.best_candidate_better_than(pos, Some(curr_ref.run())) {
            (b, Some(new)) if new.run() > curr_ref.run() => {
                ret.push(Code::Literal(curr_lit));
                curr_lit = b;
                curr_ref = new;
            }
            (_, None) | (_, Some(_)) => {
                ret.push(Code::Reference(curr_ref));
                break;
            }
        };
    }

    ret
}

fn three_zip<L: Looker>(looker: &L, pos: usize) -> Vec<Code> {
    let (first_literal, first_best) = match looker.best_candidate(pos) {
        // there's a good run, use it
        (_, Some(r)) if r.run() > 3 => return vec![r.into()],

        // there's a possibly bad run
        (l, Some(r)) => (l, r),

        // there's no run, or we're at the end: only a literal
        (b, None) => return vec![Code::Literal(b)],
    };

    assert_eq!(3, first_best.run());

    let (second_literal, mut second_best) = looker.best_candidate(pos + 1);
    second_best = second_best.filter(|x| x.run() > 3);

    // optimisation:
    if let Some(r) = second_best {
        if r.run() == 258 {
            // no point searching for a third run, as this will win.
            return vec![Code::Literal(first_literal), r.into()];
        }
    }

    let (_, mut third_best) = looker.best_candidate(pos + 2);
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
