use std::u16;

use all_refs::AllRefs;
use lookahead::Lookahead;
use picker::Picker;
use wams;
use wams::WamsOptimisations;

use Code;
use DataLen;
use Guesser;
use Looker;
use Obscure;
use Ref;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub first_byte_bug: bool,
    pub lookahead: Lookahead,
    pub picker: Picker,
    pub wams: WamsOptimisations,
}

#[derive(Debug)]
pub struct Technique<'a, 'p: 'a, 'd: 'a> {
    config: Config,
    all_refs: &'a AllRefs<'p, 'd>,
}

impl Config {
    pub fn gzip(level: u8) -> Self {
        assert!(
            level >= 1 && level <= 9,
            "gzip levels are between 1 and 9, inclusive"
        );
        Config {
            first_byte_bug: true,
            lookahead: Lookahead::Greedy,
            picker: if level >= 4 {
                Picker::DropFarThrees
            } else {
                Picker::Longest
            },
            wams: wams::CONFIGURATIONS[usize::from(level - 1)],
        }
    }

    pub fn gzip_16_fastest() -> Self {
        Self::gzip(1)
    }

    pub fn gzip_16_default() -> Self {
        Self::gzip(6)
    }

    pub fn gzip_16_best() -> Self {
        Self::gzip(9)
    }

    pub fn spicy() -> Self {
        Config {
            first_byte_bug: false,
            lookahead: Lookahead::ThreeZip,
            picker: Picker::DropFarThrees,
            wams: wams::CONFIGURATIONS[8],
        }
    }
}

impl<'a, 'p, 'd> Technique<'a, 'p, 'd> {
    pub fn new(config: Config, all_refs: &'a AllRefs<'p, 'd>) -> Self {
        Technique { config, all_refs }
    }

    pub fn byte_at(&self, pos: usize) -> u8 {
        self.all_refs.data[pos]
    }
}

impl<'a, 'p, 'd> Technique<'a, 'p, 'd> {
    pub fn guesser(&self) -> OutOfNames {
        OutOfNames {
            technique: self,
            obscured: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct OutOfNames<'t, 'a: 't, 'p: 'a + 't, 'd: 'a + 't> {
    technique: &'t Technique<'a, 'p, 'd>,
    obscured: Vec<Obscure>,
}

impl<'t, 'a, 'p, 'd> OutOfNames<'t, 'a, 'p, 'd> {
    pub fn add_obscurer(&mut self, pos: usize, code: Code) {
        let limit = match self.technique.config.wams.insert_only_below_length {
            Some(limit) => limit,
            None => return,
        };

        let r = match code {
            Code::Reference(r) => r,
            Code::Literal(_) => return,
        };

        if r.run() <= limit {
            return;
        }

        self.obscured.push((pos, r.run()))
    }
}

impl<'t, 'a, 'p, 'd, 'o> DataLen for OutOfNames<'t, 'a, 'p, 'd> {
    fn data_len(&self) -> usize {
        self.technique.all_refs.data_len()
    }
}

impl<'t, 'a, 'p, 'd, 'o> Looker for OutOfNames<'t, 'a, 'p, 'd> {
    fn best_candidate_better_than(&self, pos: usize, other: Option<u16>) -> (u8, Option<Ref>) {
        let current_literal = self.technique.all_refs.data[pos];
        let mut limit = self.technique.config.wams.limit_count_of_distances;

        if let Some(run) = other {
            if let Some(lookahead) = self.technique.config.wams.lookahead {
                if lookahead.abort_above_length > run {
                    return (current_literal, None);
                }

                if run > lookahead.apathetic_above_length {
                    limit /= 4;
                }
            }
        }

        let candidates = self.technique.all_refs.at(pos, &self.obscured);
        (
            current_literal,
            candidates.and_then(|it| {
                self.technique.config.picker.picker(
                    it.take(limit),
                    self.technique.config.wams.quit_search_above_length,
                )
            }),
        )
    }
}

impl<'t, 'a, 'p, 'd> Guesser for OutOfNames<'t, 'a, 'p, 'd> {
    fn codes_at(&self, pos: usize) -> Vec<Code> {
        self.technique.config.lookahead.lookahead(self, pos)
    }
}
