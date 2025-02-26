#![warn(clippy::all)]
#![allow(dead_code)]

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
use clap::Parser;
use cli::Cli;

mod cli;

fn main() {
    let program: Cli = Cli::parse();
    if let Err(e) = program.run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
