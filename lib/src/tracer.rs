use std::u16;

use crate::all_refs::AllRefs;
use crate::serialise_trace;
use crate::technique::Config;
use crate::technique::Technique;
use crate::trace;
use crate::Code;
use crate::Trace;

pub fn try_gzip(level: u8, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<Trace> {
    r#try(Config::gzip(level), preroll, data, codes)
}

fn r#try(config: Config, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<Trace> {
    let limit = config.wams.insert_only_below_length.unwrap_or(u16::MAX);
    let all_refs = AllRefs::with_sixteen(preroll, data, limit);

    if config.first_byte_bug {
        // TODO: ???
    }

    let traces = trace::validate(codes, &Technique::new(config, &all_refs));
    serialise_trace::verify(&traces);
    traces
}
