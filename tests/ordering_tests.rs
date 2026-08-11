//! Regression test for preserving groups and entries order across save/open.
#![cfg(feature = "save_kdbx4")]

mod common;

use crate::common::{DEMO_PASSWORD, root_entries, root_groups, save_then_open};
use keepass_ng::DatabaseKey;
use keepass_ng::db::{Database, Entry, Group, Node, with_node, with_node_mut};

#[test]
fn group_and_entry_order_should_survive_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_database();
    let key = DatabaseKey::new().with_password(DEMO_PASSWORD);

    // Write database to byte array and read it
    let actual = save_then_open(&db, key)?;

    // Read entries and groups
    let actual_group_titles = root_groups(&actual)
        .into_iter()
        .map(|group| with_node::<Group, _, _>(&group, |group| group.get_title().unwrap_or("").to_string()).unwrap())
        .collect::<Vec<String>>();
    let actual_entry_titles = root_entries(&actual)
        .into_iter()
        .map(|entry| with_node::<Entry, _, _>(&entry, |entry| entry.get_title().unwrap_or("").to_string()).unwrap())
        .collect::<Vec<String>>();

    // Check titles are in correct order
    assert_eq!(actual_group_titles, generate_group_titles());
    assert_eq!(actual_entry_titles, generate_entry_titles());

    Ok(())
}

#[test]
fn group_and_entry_order_should_survive_round_trip_after_deletion() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = setup_database();
    let key = DatabaseKey::new().with_password(DEMO_PASSWORD);

    // Keep deleted nodes out of a recycle bin so this test only measures the
    // order of the remaining root children.
    db.set_recycle_bin_enabled(false);

    // Delete some of the groups and entries
    let deleted_group_indexes = [0, 8, 19];
    let deleted_entry_indexes = [0, 7, 19];

    let group_ids = root_groups(&db)
        .into_iter()
        .map(|group| group.borrow().get_uuid())
        .collect::<Vec<_>>();
    let entry_ids = root_entries(&db)
        .into_iter()
        .map(|entry| entry.borrow().get_uuid())
        .collect::<Vec<_>>();

    for deleted_group_index in deleted_group_indexes {
        db.remove_node_by_uuid(group_ids[deleted_group_index])?;
    }

    for deleted_entry_index in deleted_entry_indexes {
        db.remove_node_by_uuid(entry_ids[deleted_entry_index])?;
    }

    // Write database to byte array and read it
    let actual = save_then_open(&db, key)?;

    // Read entries and groups
    let actual_group_titles = root_groups(&actual)
        .into_iter()
        .map(|group| with_node::<Group, _, _>(&group, |group| group.get_title().unwrap_or("").to_string()).unwrap())
        .collect::<Vec<String>>();
    let actual_entry_titles = root_entries(&actual)
        .into_iter()
        .map(|entry| with_node::<Entry, _, _>(&entry, |entry| entry.get_title().unwrap_or("").to_string()).unwrap())
        .collect::<Vec<String>>();

    // Check titles are in correct order
    assert_eq!(actual_group_titles, filter_indexes(generate_group_titles(), &deleted_group_indexes),);
    assert_eq!(actual_entry_titles, filter_indexes(generate_entry_titles(), &deleted_entry_indexes),);

    Ok(())
}

fn generate_entry_titles() -> Vec<String> {
    (0..20).into_iter().map(|index| format!("Entry_{index}")).collect()
}

fn generate_group_titles() -> Vec<String> {
    (0..20).into_iter().map(|index| format!("Group_{index}")).collect()
}

fn setup_database() -> Database {
    let db = Database::new(Default::default());
    let root_uuid = db.root.borrow().get_uuid();

    let entry_titles = generate_entry_titles();
    let group_titles = generate_group_titles();

    for (index, entry_title) in entry_titles.iter().enumerate() {
        let entry = db.create_new_entry(root_uuid, index).expect("create entry");
        with_node_mut::<Entry, _, _>(&entry, |entry| entry.set_title(Some(entry_title))).expect("configure entry");
    }
    for (index, group_title) in group_titles.iter().enumerate() {
        let group = db.create_new_group(root_uuid, entry_titles.len() + index).expect("create group");
        with_node_mut::<Group, _, _>(&group, |group| group.set_title(Some(group_title))).expect("configure group");
    }

    db
}

fn filter_indexes(titles: Vec<String>, indexes: &[usize]) -> Vec<String> {
    titles
        .into_iter()
        .enumerate()
        .filter_map(|(index, title)| (!indexes.contains(&index)).then_some(title))
        .collect()
}
