use std::u16;

use all_refs::AllRefs;
use serialise_trace;
use technique::Config;
use technique::Technique;
use trace;
use Code;
use Trace;

pub fn try_gzip(level: u8, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<Trace> {
    try(Config::gzip(level), preroll, data, codes)
}

fn try(config: Config, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<Trace> {
    let limit = config.wams.insert_only_below_length.unwrap_or(u16::MAX);
    let all_refs = AllRefs::with_sixteen(preroll, data, limit);

    if config.first_byte_bug {
        // TODO: ???
    }

    let traces = trace::validate(codes, &Technique::new(config, &all_refs));
    serialise_trace::verify(&traces);
    traces
}
