extern crate byteorder;
extern crate clap;
extern crate crc;

extern crate flate2;
extern crate librezip;

mod cat;
mod dump;
mod zero;

use std::fs;
use std::io;
use std::io::Read;

use clap::App;
use clap::Arg;
use failure::Error;

fn main() -> Result<(), Error> {
    let matches = App::new("rezippers")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            clap::SubCommand::with_name("cat").arg(Arg::with_name("file").index(1).required(false)),
        )
        .subcommand(
            clap::SubCommand::with_name("dump")
                .arg(Arg::with_name("file").index(1).required(false)),
        )
        .subcommand(
            clap::SubCommand::with_name("zero")
                .arg(Arg::with_name("file").index(1).required(false)),
        )
        .get_matches();

    match matches.subcommand() {
        ("cat", Some(matches)) => cat::run(open_file(matches)?),
        ("dump", Some(matches)) => dump::run(open_file(matches)?),
        ("zero", Some(matches)) => zero::run(open_file(matches)?),
        _ => unreachable!(),
    }
}

fn open_file(matches: &clap::ArgMatches) -> Result<impl Read, Error> {
    Ok(match matches.value_of_os("file") {
        Some(path) => Box::new(io::BufReader::new(fs::File::open(path)?)),
        None => Box::new(io::stdin()) as Box<dyn Read>,
    })
}
