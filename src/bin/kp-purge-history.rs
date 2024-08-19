/// utility to purge the history of the entries in the database
use clap::Parser;
use keepass::{db::Entry, group_get_children, node_is_group, BoxError, Database, DatabaseKey, Node, NodePtr};
use std::fs::File;

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
}

pub fn main() -> Result<(), BoxError> {
    let args = Args::parse();

    let mut source = File::open(&args.in_kdbx)?;
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

    let db = Database::open(&mut source, key.clone())?;

    purge_history(&db.root)?;

    db.save(&mut File::options().write(true).open(&args.in_kdbx)?, key)?;

    Ok(())
}

fn purge_history_for_entry(entry: &NodePtr) -> Result<(), BoxError> {
    if let Some(entry) = entry.borrow_mut().as_any_mut().downcast_mut::<Entry>() {
        if let Some(history) = entry.get_history() {
            let history_size = history.get_entries().len();
            if history_size != 0 {
                println!("Removing {} history entries from {}", history_size, entry.get_uuid());
            }
        }
        entry.purge_history();
    }
    Ok(())
}

fn purge_history(group: &NodePtr) -> Result<(), BoxError> {
    if let Some(children) = group_get_children(group) {
        for node in children {
            match node_is_group(&node) {
                true => purge_history(&node)?,
                false => purge_history_for_entry(&node)?,
            }
        }
    }
    Ok(())
}
