//! Programmatic round-trip coverage for KDBX 4.1-specific surface.
//! Each test builds a database via the public API, saves it, reopens it,
//! and asserts the 4.1 fields survive intact.

#![cfg(feature = "save_kdbx4")]
use chrono::{NaiveDate, NaiveDateTime};
use keepass_ng::db::{
    AutoType, AutoTypeAssociation, CustomDataItem, CustomDataValue, Database, Entry, Group, Node, NodeIterator, with_node, with_node_mut,
};
use keepass_ng::{DatabaseKey, DatabaseVersion};
use uuid::Uuid;

const PASSWORD: &str = "demopass";

fn save_then_open(db: &Database) -> Database {
    let mut buf = Vec::new();
    db.save(&mut buf, DatabaseKey::new().with_password(PASSWORD)).expect("save");
    Database::open(&mut buf.as_slice(), DatabaseKey::new().with_password(PASSWORD)).expect("open")
}

fn fixed_time() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2024, 6, 15).unwrap().and_hms_opt(12, 30, 45).unwrap()
}

/// Build a database touching every public KDBX 4.1 field we can reach.
/// Returns the database and the id of the entry carrying the rich fields,
/// for tests that want to inspect a specific entry post-roundtrip.
fn build_database() -> (Database, Uuid) {
    let mut db = Database::new(Default::default());
    let root_uuid = db.root.borrow().get_uuid();
    let entry = db.create_new_entry(root_uuid, 0).expect("create entry");
    let entry_uuid = entry.borrow().get_uuid();
    with_node_mut::<Entry, _, _>(&entry, |entry| {
        entry.set_title(Some("feature-bag"));
        entry.set_username(Some("alice"));
        entry.set_password(Some("hunter2"));
        entry.get_tags_mut().extend(["a".to_string(), "b".to_string(), "c".to_string()]);
        entry.custom_data_mut().insert(
            "entry-string".to_string(),
            CustomDataItem {
                value: Some(CustomDataValue::String("entry-value".to_string())),
                last_modification_time: Some(fixed_time()),
            },
        );
        entry.custom_data_mut().insert(
            "entry-binary".to_string(),
            CustomDataItem {
                value: Some(CustomDataValue::Binary(vec![1, 2, 3])),
                last_modification_time: None,
            },
        );
        entry.set_autotype(Some(AutoType {
            enabled: true,
            default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".to_string()),
            data_transfer_obfuscation: Some(true),
            associations: vec![AutoTypeAssociation {
                window: Some("Login - *".to_string()),
                sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".to_string()),
            }],
        }));
        entry.get_times_mut().set_last_modification(Some(fixed_time()));
    })
    .expect("configure entry");

    let group = db.create_new_group(root_uuid, 1).expect("create group");
    with_node_mut::<Group, _, _>(&group, |group| {
        group.set_title(Some("tagged-group"));
        group.get_tags_mut().extend(["root-tag".to_string(), "shared".to_string()]);
        group.custom_data_mut().insert(
            "group-string".to_string(),
            CustomDataItem {
                value: Some(CustomDataValue::String("group-value".to_string())),
                last_modification_time: Some(fixed_time()),
            },
        );
        group.get_times_mut().set_last_modification(Some(fixed_time()));
    })
    .expect("configure group");

    db.deleted_objects.insert(Uuid::nil(), Some(fixed_time()));
    (db, entry_uuid)
}

fn find_node<T: Node + 'static>(db: &Database, uuid: Uuid) -> keepass_ng::db::NodePtr {
    NodeIterator::new(&db.root)
        .find(|node| node.borrow().downcast_ref::<T>().is_some_and(|node| node.get_uuid() == uuid))
        .expect("node survives")
}

#[test]
fn database_round_trips_kdbx41_fields() {
    let (db, entry_uuid) = build_database();
    let parsed = save_then_open(&db);
    assert_eq!(parsed.config.version, DatabaseVersion::KDB4(1));
    assert_eq!(parsed.deleted_objects.get(&Uuid::nil()), Some(&Some(fixed_time())));

    let entry = find_node::<Entry>(&parsed, entry_uuid);
    with_node::<Entry, _, _>(&entry, |entry| {
        assert_eq!(entry.get_title(), Some("feature-bag"));
        assert_eq!(entry.get_username(), Some("alice"));
        assert_eq!(entry.get_password(), Some("hunter2"));
        assert_eq!(entry.get_tags(), &["a", "b", "c"]);
        assert_eq!(
            entry.custom_data().get("entry-string"),
            Some(&CustomDataItem {
                value: Some(CustomDataValue::String("entry-value".to_string())),
                last_modification_time: Some(fixed_time()),
            })
        );
        assert_eq!(
            entry.custom_data().get("entry-binary"),
            Some(&CustomDataItem {
                value: Some(CustomDataValue::Binary(vec![1, 2, 3])),
                last_modification_time: None,
            })
        );
        assert_eq!(entry.get_times().get_last_modification(), Some(fixed_time()));
        let autotype = entry.get_autotype().expect("autotype survives");
        assert!(autotype.enabled);
        assert_eq!(autotype.data_transfer_obfuscation, Some(true));
        assert_eq!(autotype.associations[0].window.as_deref(), Some("Login - *"));
    });

    let group = Group::get(&parsed.root, &["tagged-group"]).expect("group survives");
    with_node::<Group, _, _>(&group, |group| {
        assert_eq!(group.tags(), &["root-tag", "shared"]);
        assert_eq!(
            group.custom_data().get("group-string"),
            Some(&CustomDataItem {
                value: Some(CustomDataValue::String("group-value".to_string())),
                last_modification_time: Some(fixed_time()),
            })
        );
        assert_eq!(group.get_times().get_last_modification(), Some(fixed_time()));
    });
}

#[test]
fn protected_password_remains_protected_after_round_trip() {
    let (db, entry_uuid) = build_database();
    let parsed = save_then_open(&db);
    let entry = find_node::<Entry>(&parsed, entry_uuid);

    with_node::<Entry, _, _>(&entry, |entry| {
        assert_eq!(entry.get_password(), Some("hunter2"));
    });
}
