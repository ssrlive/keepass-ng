/// utility to recover a Yubikey-protected database using the HMAC-SHA1 secret
use clap::Parser;
use keepass_ng::{BoxError, Database, DatabaseKey};
use std::fs::File;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Provide a .kdbx database
    in_kdbx: String,

    /// Output file to write
    out_kdbx: String,

    /// Provide a keyfile
    #[arg(short = 'k', long)]
    keyfile: Option<String>,

    /// Do not use a password to decrypt the database
    #[arg(long)]
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

    let key_without_yubikey = key.clone();

    key = key.with_hmac_sha1_secret_from_prompt("HMAC-SHA1 secret: ")?;

    let db = Database::open(&mut source, key.clone())?;

    let mut out_file = File::create(args.out_kdbx)?;

    db.save(&mut out_file, key_without_yubikey)?;

    println!("Yubikey was removed from the database key.");

    Ok(())
}
