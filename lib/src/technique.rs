use all_refs::AllRefs;
use lookahead::Lookahead;
use picker::Picker;
use wams;
use wams::WamsOptimisations;

use Code;
use DataLen;
use Guesser;
use Looker;
use Ref;
use usize_from;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub first_byte_bug: bool,
    pub lookahead: Lookahead,
    pub picker: Picker,
    pub wams: WamsOptimisations,
}

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

impl<'a, 'p, 'd> DataLen for Technique<'a, 'p, 'd> {
    fn data_len(&self) -> usize {
        self.all_refs.data_len()
    }
}

impl<'a, 'p, 'd> Looker for Technique<'a, 'p, 'd> {
    fn best_candidate_better_than(&self, pos: usize, other: Option<u16>) -> (u8, Option<Ref>) {
        let current_literal = self.all_refs.data[pos];
        let mut limit = self.config.wams.limit_count_of_distances;

        if let Some(run) = other {
            if let Some(lookahead) = self.config.wams.lookahead {
                if lookahead.abort_above_length > run {
                    return (current_literal, None);
                }

                if run > lookahead.apathetic_above_length {
                    limit /= 4;
                }
            }
        }

        let candidates = self.all_refs.at(pos);
        (
            current_literal,
            candidates.and_then(|it| {
                self.config
                    .picker
                    .picker(it.take(limit), self.config.wams.quit_search_above_length)
            }),
        )
    }
}

impl<'a, 'p, 'd> Guesser for Technique<'a, 'p, 'd> {
    fn codes_at(&self, pos: usize) -> Vec<Code> {
        self.config.lookahead.lookahead(self, pos)
    }
}
