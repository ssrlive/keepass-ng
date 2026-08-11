//! Regression tests for sseemayer/keepass-rs#338: centralized tag
//! delimiter and (de)serialization shared between entry and group.
//!
//! Before the fix:
//!   * `Entry::xml_to_db_handle` split tags on `,`.
//!   * `Group::xml_to_db_handle` split tags on `;`.
//!   * `Entry::db_to_xml` joined tags with `,`.
//!   * `Group::db_to_xml` joined tags with `;`.
//!
//! The canonical KeePass write delimiter is `;`. KeePassXC has historically
//! also written `,` (see `tests/resources/inner_xml_with_custom_fields.xml`
//! and `test_db_kdbx4_with_password_aes.kdbx`), so reading must accept both.

#![cfg(feature = "save_kdbx4")]

use keepass_ng::db::{Database, Entry, Group, Node, NodeIterator, with_node_mut};
use keepass_ng::{DatabaseKey, db};

const PASSWORD: &str = "demopass";

fn key() -> DatabaseKey {
    DatabaseKey::new().with_password(PASSWORD)
}

fn build_db_with_tagged_entry_and_group(entry_tags: Vec<String>, group_tags: Vec<String>) -> Database {
    let db = Database::new(Default::default());
    let root_uuid = db.root.borrow().get_uuid();

    let entry = db.create_new_entry(root_uuid, 0).expect("create entry");
    with_node_mut::<Entry, _, _>(&entry, |entry| {
        entry.set_title(Some("tagged-entry"));
        *entry.get_tags_mut() = entry_tags;
    })
    .expect("configure entry");

    let group = db.create_new_group(root_uuid, 1).expect("create group");
    with_node_mut::<Group, _, _>(&group, |group| {
        group.set_title(Some("tagged-group"));
        *group.get_tags_mut() = group_tags;
    })
    .expect("configure group");

    db
}

fn node_by_title<T: Node + 'static>(db: &Database, title: &str) -> db::NodePtr {
    NodeIterator::new(&db.root)
        .find(|node| {
            node.borrow()
                .downcast_ref::<T>()
                .is_some_and(|node| node.get_title() == Some(title))
        })
        .expect("node from database")
}

fn save_and_get_xml(db: &Database) -> (Vec<u8>, String) {
    let mut buf = Vec::new();
    db.save(&mut buf, key()).expect("save db");

    let xml = Database::get_xml(&mut std::io::Cursor::new(&buf), key()).expect("decrypt re-save");
    let xml = String::from_utf8(xml).expect("xml is utf-8");

    (buf, xml)
}

#[test]
fn writer_uses_canonical_semicolon_delimiter_for_entry_and_group() {
    let db = build_db_with_tagged_entry_and_group(
        vec!["alpha".into(), "beta".into(), "gamma".into()],
        vec!["one".into(), "two".into(), "three".into()],
    );

    let (_, xml) = save_and_get_xml(&db);

    assert!(
        xml.contains("<Tags>alpha;beta;gamma</Tags>"),
        "expected entry tags joined with ';' (canonical KeePass delimiter). \
         Before sseemayer/keepass-rs#338 the entry writer used ',' which \
         diverged from the group writer. Got XML:\n{}",
        xml,
    );
    assert!(
        xml.contains("<Tags>one;two;three</Tags>"),
        "expected group tags joined with ';'. Got XML:\n{}",
        xml,
    );
    assert!(
        !xml.contains("<Tags>alpha,beta,gamma</Tags>"),
        "entry tags must not use ',' as the canonical write delimiter. \
         Got XML:\n{}",
        xml,
    );
}

#[test]
fn entry_and_group_round_trip_tags_identically() {
    let entry_tags = vec!["alpha".to_string(), "beta".to_string()];
    let group_tags = vec!["one".to_string(), "two".to_string()];

    let db = build_db_with_tagged_entry_and_group(entry_tags.clone(), group_tags.clone());
    let (bytes, _) = save_and_get_xml(&db);

    let parsed = Database::open(&mut bytes.as_slice(), key()).expect("re-open");

    let entry = node_by_title::<Entry>(&parsed, "tagged-entry");
    let group = node_by_title::<Group>(&parsed, "tagged-group");

    let entry_tags_after = entry.borrow().downcast_ref::<Entry>().unwrap().get_tags().to_vec();
    let group_tags_after = group.borrow().downcast_ref::<Group>().unwrap().tags().to_vec();
    assert_eq!(entry_tags_after, entry_tags, "entry tags lost during round trip");
    assert_eq!(group_tags_after, group_tags, "group tags lost during round trip");
}

#[test]
fn reader_accepts_comma_delimited_entry_tags() {
    // Regression: the entry-tag splitter previously used ',' exclusively.
    // This test verifies that with the centralized splitter, ','-delimited
    // tags (as produced by older KeePassXC writers) still parse correctly.
    let path = std::path::Path::new("tests/resources/test_db_kdbx4_with_password_aes.kdbx");
    let db = Database::open(&mut std::fs::File::open(path).expect("open fixture"), key()).expect("parse fixture");

    let entry = node_by_title::<Entry>(&db, "ASDF");

    assert_eq!(
        entry.borrow().downcast_ref::<Entry>().unwrap().get_tags().to_vec(),
        vec!["keepass-rs".to_string(), "test".to_string()],
        "',' delimited entry tags from the KeePassXC fixture must parse into a list",
    );
}

#[test]
fn reader_accepts_semicolon_delimited_group_tags() {
    // Group tags in `test_db_kdbx41_with_password_aes.kdbx` are stored
    // `;`-delimited (`a;b;c`). The centralized splitter must keep that
    // working.
    let path = std::path::Path::new("tests/resources/test_db_kdbx41_with_password_aes.kdbx");
    let db = Database::open(&mut std::fs::File::open(path).expect("open fixture"), key()).expect("parse fixture");

    let group = node_by_title::<Group>(&db, "Group with tags");

    assert_eq!(
        group.borrow().downcast_ref::<Group>().unwrap().tags(),
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        "';' delimited group tags from the KeePass fixture must parse into a list",
    );
}
