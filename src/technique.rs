use all_refs::AllRefs;
use lookahead::Lookahead;
use picker::Picker;

use Code;
use DataLen;
use Guesser;
use Looker;
use Ref;

pub struct Technique<'a, 'p: 'a, 'd: 'a> {
    pub all_refs: &'a AllRefs<'p, 'd>,
    pub lookahead: Lookahead,
    pub picker: Picker,
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
            candidates.and_then(|it| self.picker.picker(it)),
        )
    }
}

impl<'a, 'p, 'd> Guesser for Technique<'a, 'p, 'd> {
    fn codes_at(&self, pos: usize) -> Vec<Code> {
        self.lookahead.lookahead(self, pos)
    }
}
