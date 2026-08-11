//! Regression tests for empty entry fields.
//! Some KeePass implementations may add or remove empty entry fields.
//! To preserve the original database structure, a round trip should retain
//! explicitly present empty fields and shouldn't
//! add empty default fields (UserName, Password, etc.).
#![cfg(feature = "save_kdbx4")]

mod common;

use crate::common::{DEMO_PASSWORD, save_then_open};
use keepass_ng::DatabaseKey;
use keepass_ng::db::{Database, Entry, Group, Node, with_node, with_node_mut};

const ENTRY_TITLE: &str = "DemoEntry";

#[test]
fn empty_entry_fields_should_survive_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let db = create_database_with_empty_fields();
    let key = DatabaseKey::new().with_password(DEMO_PASSWORD);

    // Write database to byte array and read it
    let actual = save_then_open(&db, key)?;

    // Empty standard and custom fields should be present after parsing
    let entry = Group::get(&actual.root, &[ENTRY_TITLE]).ok_or("entry should be present in database")?;

    with_node::<Entry, _, _>(&entry, |entry| {
        assert_eq!(entry.get_username(), Some(""));
        assert_eq!(entry.get_additional_attribute("CustomField"), Some(""));
    })
    .ok_or("node is not an entry")?;

    Ok(())
}

#[test]
fn empty_default_fields_should_not_be_added_after_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let db = create_database_without_empty_fields();
    let key = DatabaseKey::new().with_password(DEMO_PASSWORD);

    // Write database to byte array and read it
    let actual = save_then_open(&db, key)?;

    // Entry should not have empty default fields
    let entry = Group::get(&actual.root, &[ENTRY_TITLE]).ok_or("entry should be present in database")?;

    with_node::<Entry, _, _>(&entry, |entry| {
        assert_eq!(entry.get_username(), None);
        assert_eq!(entry.get_password(), None);
        assert_eq!(entry.get_url(), None);
        assert_eq!(entry.get_notes(), None);
    })
    .ok_or("node is not an entry")?;

    Ok(())
}

fn create_database_with_empty_fields() -> Database {
    let db = Database::new(Default::default());
    let root_uuid = db.root.borrow().get_uuid();
    let entry = db.create_new_entry(root_uuid, 0).expect("create entry");

    // Add standard KeePass field and custom field.
    with_node_mut::<Entry, _, _>(&entry, |entry| {
        entry.set_title(Some(ENTRY_TITLE));
        entry.set_username(Some(""));
        entry.set_additional_attribute("CustomField", Some("")).expect("set custom field");
    })
    .expect("configure entry");

    db
}

fn create_database_without_empty_fields() -> Database {
    let db = Database::new(Default::default());
    let root_uuid = db.root.borrow().get_uuid();
    let entry = db.create_new_entry(root_uuid, 0).expect("create entry");

    with_node_mut::<Entry, _, _>(&entry, |entry| {
        entry.set_title(Some(ENTRY_TITLE));
    })
    .expect("configure entry");

    db
}
