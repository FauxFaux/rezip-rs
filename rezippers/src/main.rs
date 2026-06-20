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
use std::path::PathBuf;

use anyhow::Error;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Cat { file: Option<PathBuf> },
    Dump { file: Option<PathBuf> },
    Zero { file: Option<PathBuf> },
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match cli.command {
        Command::Cat { file } => cat::run(open_file(file)?),
        Command::Dump { file } => dump::run(open_file(file)?),
        Command::Zero { file } => zero::run(open_file(file)?),
    }
}

fn open_file(file: Option<PathBuf>) -> Result<impl Read, Error> {
    Ok(match file {
        Some(path) => Box::new(io::BufReader::new(fs::File::open(path)?)),
        None => Box::new(io::stdin()) as Box<dyn Read>,
    })
}
