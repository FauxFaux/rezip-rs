use all_refs::AllRefs;
use lookahead::Lookahead;
use picker::Picker;

use Code;
use DataLen;
use Guesser;
use Looker;
use Ref;

pub struct Technique<'p, 'd> {
    pub all_refs: AllRefs<'p, 'd>,
    pub lookahead: Lookahead,
    pub picker: Picker,
}

impl<'p, 'd> DataLen for Technique<'p, 'd> {
    fn data_len(&self) -> usize {
        self.all_refs.data_len()
    }
}

impl<'p, 'd> Looker for Technique<'p, 'd> {
    fn best_candidate(&self, pos: usize) -> (u8, Option<Ref>) {
        let candidates = self.all_refs.at(pos);
        (
            self.all_refs.data[pos],
            candidates.and_then(|it| self.picker.picker(it)),
        )
    }
}

impl<'p, 'd> Guesser for Technique<'p, 'd> {
    fn codes_at(&self, pos: usize) -> Vec<Code> {
        self.lookahead.lookahead(self, pos)
    }
}
