use all_refs::AllRefs;
use lookahead::Lookahead;
use picker::Picker;

use Code;
use DataLen;
use Guesser;
use Looker;
use Ref;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Config {
    lookahead: Lookahead,
    picker: Picker,
}

pub struct Technique<'a, 'p: 'a, 'd: 'a> {
    config: Config,
    all_refs: &'a AllRefs<'p, 'd>,
}

impl Config {
    pub fn gzip_16_fast() -> Self {
        Config {
            lookahead: Lookahead::Greedy,
            picker: Picker::DropFarThrees,
        }
    }

    pub fn gzip_16_good() -> Self {
        Config {
            lookahead: Lookahead::Gzip,
            picker: Picker::DropFarThrees,
        }
    }

    pub fn spicy() -> Self {
        Config {
            lookahead: Lookahead::ThreeZip,
            picker: Picker::DropFarThrees,
        }
    }
}

impl<'a, 'p, 'd> Technique<'a, 'p, 'd> {
    pub fn new(config: Config, all_refs: &'a AllRefs<'p, 'd>) -> Self {
        Technique { config, all_refs }
    }
}

impl<'a, 'p, 'd> DataLen for Technique<'a, 'p, 'd> {
    fn data_len(&self) -> usize {
        self.all_refs.data_len()
    }
}

impl<'a, 'p, 'd> Looker for Technique<'a, 'p, 'd> {
    fn best_candidate(&self, pos: usize) -> (u8, Option<Ref>) {
        let candidates = self.all_refs.at(pos);
        (
            self.all_refs.data[pos],
            candidates.and_then(|it| self.config.picker.picker(it)),
        )
    }
}

impl<'a, 'p, 'd> Guesser for Technique<'a, 'p, 'd> {
    fn codes_at(&self, pos: usize) -> Vec<Code> {
        self.config.lookahead.lookahead(self, pos)
    }
}
