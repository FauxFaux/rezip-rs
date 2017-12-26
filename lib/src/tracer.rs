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
    let mut all_refs = match config.wams.insert_only_below_length {
        Some(limit) => unimplemented!(),
        None => AllRefs::with_everything(preroll, data),
    };

    if config.first_byte_bug {
        all_refs.apply_first_byte_bug_rule();
    }

    serialise_trace::verify(&trace::validate(codes, &Technique::new(config, &all_refs)))
}
