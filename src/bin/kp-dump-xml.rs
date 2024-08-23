/// utility to dump keepass database internal XML data.
use std::fs::File;
use std::io::Write;

use clap::Parser;

use keepass_ng::{BoxError, Database, DatabaseKey};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Provide a .kdbx database
    in_kdbx: String,

    /// Output XML filename
    out_xml: String,

    /// Provide a keyfile
    #[arg(short = 'k', long)]
    keyfile: Option<String>,

    /// Do not use a password to decrypt the database
    #[arg(short = 'n', long)]
    no_password: bool,
}

pub fn main() -> Result<(), BoxError> {
    let args = Args::parse();

    let mut source = File::open(args.in_kdbx)?;
    let mut key = DatabaseKey::new();

    if let Some(f) = args.keyfile {
        key = key.with_keyfile(&mut File::open(f)?)?;
    }

    if !args.no_password {
        key = key.with_password_from_prompt("Password: ")?;
    }

    if key.is_empty() {
        return Err("No database key was provided.".into());
    }

    let xml = Database::get_xml(&mut source, key)?;

    File::create(args.out_xml)?.write_all(&xml)?;

    Ok(())
}
