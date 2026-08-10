//! tests for round-tripping of generated databases
#![cfg(feature = "save_kdbx4")]
#![forbid(unsafe_code)]
#![allow(clippy::indexing_slicing, clippy::expect_used, clippy::unwrap_used)]

mod common;

use common::{Combo, baseline_combo, entry_with_attachment, root_entries, root_groups, round_trip_combos, total_attachments};
use keepass_ng::db::{Database, Entry, Node, with_node};

#[test]
fn matrix_round_trip_minimal_database() {
    for combo in &round_trip_combos() {
        let db = combo.minimal_database();
        let bytes = common::save_to_vec(&db, combo.get_key());
        assert!(
            bytes.len() > 32,
            "combo {} produced suspiciously small output ({} bytes)",
            combo.label,
            bytes.len()
        );
        assert_eq!(&bytes[..4], &[0x03, 0xd9, 0xa2, 0x9a], "combo {} missing kdbx magic", combo.label);
        let parsed = Database::open(&mut bytes.as_slice(), combo.get_key())
            .unwrap_or_else(|e| panic!("combo {} reopen failed: {:?}", combo.label, e));
        assert_eq!(parsed, db, "combo {} round-trip mismatch", combo.label);
    }
}

#[test]
fn matrix_round_trip_rich_database_subset() {
    let combos = round_trip_combos();
    let subset: Vec<&Combo> = combos
        .iter()
        .filter(|c| {
            c.label.contains("aes256+gz+inner-chacha20+argon2d")
                || c.label.contains("chacha20+gz+inner-chacha20+argon2id")
                || c.label.contains("aes256+none+inner-salsa20+aeskdf")
        })
        .collect();
    for combo in &subset {
        let db = combo.rich_database();
        let bytes = common::save_to_vec(&db, combo.get_key());

        let parsed = Database::open(&mut bytes.as_slice(), combo.get_key())
            .unwrap_or_else(|e| panic!("combo {} reopen failed: {:?}", combo.label, e));

        assert_eq!(root_entries(&parsed).len(), 10, "{} root entry count", combo.label);
        assert_eq!(root_groups(&parsed).len(), 1, "{} root group count", combo.label);
        assert_eq!(total_attachments(&parsed), 3, "{} num_attachments mismatch", combo.label);

        let entry = entry_with_attachment(&parsed, "small.bin").unwrap_or_else(|| panic!("{}: no entry holds small.bin", combo.label));
        with_node::<Entry, _, _>(&entry, |entry| {
            assert_eq!(
                entry.attachments["small.bin"].data.get().as_slice(),
                b"small",
                "{} small.bin",
                combo.label
            );
            assert_eq!(
                entry.attachments["noise.bin"].data.get().len(),
                4096,
                "{} noise.bin len",
                combo.label
            );
            assert_eq!(
                entry.attachments["nonutf8.bin"].data.get().as_slice(),
                &[0xFF, 0xFE, 0xFD, 0x80, 0x81, 0x82, 0x00, 0x01],
                "{} nonutf8 bytes",
                combo.label
            );
        })
        .unwrap();

        let names: Vec<String> = with_node::<Entry, _, _>(&entry, |entry| entry.attachments.keys().cloned().collect()).unwrap();
        for n in ["small.bin", "noise.bin", "nonutf8.bin"] {
            assert!(names.contains(&n.to_string()), "{} {} name", combo.label, n);
        }

        assert!(root_groups(&parsed).iter().any(|group| {
            with_node::<keepass_ng::db::Group, _, _>(group, |group| group.get_title() == Some("Recycle Bin")).unwrap_or(false)
        }));
    }
}

#[test]
fn second_save_is_self_consistent() {
    let combo = baseline_combo();
    let db = combo.rich_database();
    let bytes_a = common::save_to_vec(&db, combo.get_key());
    let parsed_a = Database::open(&mut bytes_a.as_slice(), combo.get_key()).unwrap();

    let bytes_b = common::save_to_vec(&parsed_a, combo.get_key());
    let parsed_b = Database::open(&mut bytes_b.as_slice(), combo.get_key()).unwrap();

    assert_eq!(parsed_a, parsed_b);
}

#[test]
fn protected_field_decrypts_after_round_trip() {
    let combo = baseline_combo();
    let db = combo.rich_database();
    let bytes = common::save_to_vec(&db, combo.get_key());
    let parsed = Database::open(&mut bytes.as_slice(), combo.get_key()).unwrap();

    let entry = root_entries(&parsed)
        .into_iter()
        .find(|e| with_node::<Entry, _, _>(e, |entry| entry.get_title().is_some_and(|t| t.starts_with("entry-"))).unwrap_or(false))
        .expect("at least one entry-NN");
    let pw = with_node::<Entry, _, _>(&entry, |entry| entry.get("Password").map(str::to_owned))
        .flatten()
        .expect("password is decryptable");
    assert!(pw.starts_with("pw-"));

    let mut found = false;
    for e in root_entries(&parsed) {
        if with_node::<Entry, _, _>(&e, |entry| entry.get_additional_attribute("custom.protected.0").is_some()).unwrap_or(false) {
            found = true;
            break;
        }
    }
    assert!(found, "no Protected custom field round-tripped");
}
