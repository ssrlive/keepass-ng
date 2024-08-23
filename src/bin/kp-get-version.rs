/// utility to get the version of a `KeePass` database.
use clap::Parser;
use keepass_ng::{BoxError, Database};
use std::fs::File;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Provide a .kdbx database
    in_kdbx: String,
}

pub fn main() -> Result<(), BoxError> {
    let args = Args::parse();

    let mut source = File::open(args.in_kdbx)?;

    let version = Database::get_version(&mut source)?;
    println!("{}", version);
    Ok(())
}
