/// utility to dump keepass database internal XML data.
use std::fs::File;

use clap::Parser;
use keepass::{
    db::{Entry, Group},
    BoxError, Database, DatabaseKey,
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Provide a .kdbx database
    in_kdbx: String,

    /// Provide a keyfile
    #[arg(short = 'k', long)]
    keyfile: Option<String>,

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

    let password = rpassword::prompt_password("Password (or blank for none): ").expect("Read password");

    if !password.is_empty() {
        key = key.with_password(&password);
    };

    let db = Database::open(&mut source, key)?;

    if let Some(e) = Group::get(&db.root, &[&args.entry]) {
        let e = e.borrow();
        let e = e.as_any().downcast_ref::<Entry>().unwrap();
        let totp = e.get_otp().unwrap();
        println!("Token is {}", totp.value_now().unwrap().code);
        Ok(())
    } else {
        panic!("Could not find entry with provided name")
    }
}
