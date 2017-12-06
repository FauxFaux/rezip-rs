#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WamsOptimisations {
    /// If we find a match of at least this length, we don't need to do any more searching.
    pub quit_search_above_length: u16,

    /// Only consider the nearest N inserted distances when searching for a run.
    pub limit_count_of_distances: usize,

    /// Update references only if the match length is fewer than this, otherwise always update
    pub insert_only_below_length: Option<u16>,

    /// Tweak lookahead, if enabled
    pub lookahead: Option<LookaheadConfig>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LookaheadConfig {
    /// If we already have a match this long, quarter the allowed number of distances to consider
    pub apathetic_above_length: u16,

    /// Abort the lookahead procedure if we've found a run at least this long.
    pub abort_above_length: u16,
}

pub const CONFIGURATIONS: [WamsOptimisations; 9] = [
    greedy(8, 4, 4),
    greedy(16, 8, 5),
    greedy(32, 32, 6),
    lookahead(16, 16, 4, 4),
    lookahead(32, 32, 8, 16),
    lookahead(128, 128, 8, 16),
    lookahead(128, 256, 8, 32),
    lookahead(258, 1024, 32, 128),
    lookahead(258, 4096, 32, 258),
];

const fn greedy(
    quit_search_above_length: u16,
    limit_count_of_distances: usize,
    insert_only_below_length: u16,
) -> WamsOptimisations {
    WamsOptimisations {
        quit_search_above_length,
        limit_count_of_distances,
        insert_only_below_length: Some(insert_only_below_length),
        lookahead: None,
    }
}

const fn lookahead(
    quit_search_above_length: u16,
    limit_count_of_distances: usize,
    apathetic_above_length: u16,
    abort_above_length: u16,
) -> WamsOptimisations {
    WamsOptimisations {
        quit_search_above_length,
        limit_count_of_distances,
        insert_only_below_length: None,
        lookahead: Some(LookaheadConfig {
            abort_above_length,
            apathetic_above_length,
        }),
    }
}
