use all_refs::AllRefs;
use technique::Config;
use technique::Technique;
use serialise_trace;
use trace;

use decompressed_codes;
use Code;

pub fn try_gzip(level: u8, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<u8> {
    try(Config::gzip(level), preroll, data, codes)
}

fn try(config: Config, preroll: &[u8], data: &[u8], codes: &[Code]) -> Vec<u8> {
    let all_refs = match config.wams.insert_only_below_length {
        Some(limit) => AllRefs::limited_by(preroll, data, codes, limit),
        None => AllRefs::with_everything(preroll, data),
    };

    serialise_trace::verify(&trace::validate(codes, &Technique::new(config, &all_refs)))
}
