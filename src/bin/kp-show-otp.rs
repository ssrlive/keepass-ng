/// utility to dump keepass database internal XML data.
use std::fs::File;

use clap::Parser;
use keepass_ng::{
    db::{with_node, Database, Entry, Group},
    BoxError, DatabaseKey,
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Provide a .kdbx database
    in_kdbx: String,

    /// Provide a keyfile
    #[arg(short = 'k', long)]
    keyfile: Option<String>,

    /// Do not use a password to decrypt the database
    #[arg(short = 'n', long)]
    no_password: bool,

    /// Provide the entry to read
    entry: String,
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

    let db = Database::open(&mut source, key)?;

    with_node::<Group, _, _>(&db.root, |root| {
        let entry = root.get(&[&args.entry]).ok_or("Could not find entry with provided name")?;
        with_node::<Entry, _, _>(&entry, |entry| {
            let totp = entry.get_otp().unwrap();
            println!("Token is {}", totp.value_now().unwrap().code);
            Ok::<(), BoxError>(())
        })
        .ok_or("Could not find entry with provided name")??;
        Ok::<(), BoxError>(())
    })
    .ok_or("Could not find entry with provided name")??;

    Ok(())
}
