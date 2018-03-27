use std::u16;

use Code;
use all_refs::AllRefs;
use serialise_trace;
use technique::Config;
use technique::Technique;
use trace;

pub fn try_gzip(level: u8, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<u8> {
    try(Config::gzip(level), preroll, data, codes)
}

fn try(config: Config, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<u8> {
    let limit = config.wams.insert_only_below_length.unwrap_or(u16::MAX);
    let all_refs = AllRefs::with_sixteen(preroll, data, limit);

    if config.first_byte_bug {
        // TODO: ???
    }

    serialise_trace::verify(&trace::validate(codes, &Technique::new(config, &all_refs)))
}
