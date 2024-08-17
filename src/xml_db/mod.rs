pub mod dump;
pub mod parse;

/// In KDBX4, timestamps are stored as seconds, Base64 encoded, since 0001-01-01 00:00:00.
/// This function returns the epoch baseline used by KDBX for date serialization.
pub fn get_epoch_baseline() -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str("0001-01-01T00:00:00", "%Y-%m-%dT%H:%M:%S").unwrap()
}

#[cfg(test)]
mod tests {
    use crate::{
        config::DatabaseConfig,
        db::{
            entry::History,
            group_get_children,
            iconid::IconId,
            meta::{BinaryAttachments, CustomIcons, Icon, MemoryProtection},
            node::*,
            node_is_equals_to, AutoType, AutoTypeAssociation, BinaryAttachment, CustomData, CustomDataItem, Database, DeletedObject, Entry,
            Group, Meta, NodePtr, Times, Value,
        },
        format::kdbx4,
        key::DatabaseKey,
        rc_refcell_node,
    };
    use chrono::NaiveDateTime;
    use secstr::SecStr;
    use std::collections::HashMap;
    use uuid::uuid;

    fn make_key() -> Vec<Vec<u8>> {
        let mut password_bytes: Vec<u8> = vec![];
        let mut password: String = "".to_string();
        password_bytes.resize(40, 0);
        getrandom::getrandom(&mut password_bytes).unwrap();
        for random_char in password_bytes {
            password += &std::char::from_u32(random_char as u32).unwrap().to_string();
        }

        DatabaseKey::new().with_password(&password).get_key_elements().unwrap()
    }

    #[test]
    pub fn test_entry() {
        let mut entry = Entry::default();

        entry.set_title(Some("ASDF"));
        entry.fields.insert("UserName".to_string(), Value::Unprotected("ghj".to_string()));
        entry.fields.insert(
            "Password".to_string(),
            Value::Protected(std::str::from_utf8(b"klmno").unwrap().into()),
        );
        entry.tags.push("test".to_string());
        entry.tags.push("keepass-rs".to_string());
        entry.times.set_expires(true);
        entry.times.set_usage_count(42);
        entry.times.set_creation(Some(NaiveDateTime::default()));
        entry.times.set_expiry_time(Some(NaiveDateTime::default()));
        entry.times.set_last_access(Some(NaiveDateTime::default()));
        entry.times.set_location_changed(Some(Times::now()));
        entry.times.set_last_modification(Some(Times::now()));

        entry.set_autotype(Some(AutoType {
            enabled: true,
            sequence: Some("Autotype-sequence".to_string()),
            associations: vec![
                AutoTypeAssociation {
                    window: Some("window-1".to_string()),
                    sequence: Some("sequence-1".to_string()),
                },
                AutoTypeAssociation {
                    window: None,
                    sequence: None,
                },
            ],
        }));

        entry.custom_data.items.insert(
            "CDI-key".to_string(),
            CustomDataItem {
                value: Some(Value::Unprotected("CDI-Value".to_string())),
                last_modification_time: Some(NaiveDateTime::default()),
            },
        );

        entry.icon_id = Some(IconId::KEY);
        entry.custom_icon_uuid = Some(uuid!("22222222222222222222222222222222"));

        entry.foreground_color = Some("#C0FFEE".parse().unwrap());
        entry.background_color = Some("#1C1357".parse().unwrap());

        entry.override_url = Some("https://docs.rs/keepass-rs/".to_string());
        entry.quality_check = Some(true);

        let mut history = History::default();
        history.entries.push(entry.clone());

        entry.history = Some(history);

        let entry = rc_refcell_node!(entry);

        let root_group = rc_refcell_node!(Group::new("Root"));
        group_add_child(&root_group, entry.borrow().duplicate(), 0).unwrap();

        let mut db = Database::new(DatabaseConfig::default());
        db.root = root_group.into();

        let key_elements = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &key_elements, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &key_elements).unwrap();

        assert_eq!(group_get_children(&decrypted_db.root).unwrap().len(), 1);

        let decrypted_entry = &group_get_children(&decrypted_db.root).unwrap()[0];
        // decrypted_entry.borrow_mut().set_parent(None);
        assert!(node_is_equals_to(decrypted_entry, &entry));
    }

    #[test]
    pub fn test_group() {
        let root_group = rc_refcell_node!(Group::new("Root"));
        let entry = rc_refcell_node!(Entry::default());
        let new_entry_uuid = entry.borrow().get_uuid();
        entry.borrow_mut().set_title(Some("ASDF"));

        group_add_child(&root_group, entry, 0).unwrap();

        let subgroup = rc_refcell_node!(Group::new("Child group"));
        if let Some(subgroup) = subgroup.borrow_mut().as_any_mut().downcast_mut::<Group>() {
            subgroup.notes = Some("I am a subgroup".to_string());
            subgroup.icon_id = Some(IconId::FOLDER);
            subgroup.custom_icon_uuid = Some(uuid!("11111111111111111111111111111111"));
            subgroup.times.set_expires(true);
            subgroup.times.set_usage_count(100);
            subgroup.times.set_creation(Some(NaiveDateTime::default()));
            subgroup.times.set_expiry_time(Some(NaiveDateTime::default()));
            subgroup.times.set_last_access(Some(NaiveDateTime::default()));
            subgroup.times.set_location_changed(Some(Times::now()));
            subgroup.times.set_last_modification(Some(Times::now()));
            subgroup.is_expanded = true;
            subgroup.default_autotype_sequence = Some("{UP}{UP}{DOWN}{DOWN}{LEFT}{RIGHT}{LEFT}{RIGHT}BA".to_string());
            subgroup.enable_autotype = Some("yes".to_string());
            subgroup.enable_searching = Some("sure".to_string());

            subgroup.last_top_visible_entry = Some(uuid!("43210000000000000000000000000000"));

            subgroup.custom_data.items.insert(
                "CustomOption".to_string(),
                CustomDataItem {
                    value: Some(Value::Unprotected("CustomOption-Value".to_string())),
                    last_modification_time: Some(NaiveDateTime::default()),
                },
            );
        }

        group_add_child(&root_group, subgroup, 1).unwrap();

        let mut db = Database::new(DatabaseConfig::default());
        db.root = root_group.borrow().duplicate().into();

        let key_elements = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &key_elements, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &key_elements).unwrap();

        assert_eq!(group_get_children(&decrypted_db.root).unwrap().len(), 2);

        let decrypted_entry = &group_get_children(&decrypted_db.root).unwrap()[0];
        assert_eq!(decrypted_entry.borrow().get_title(), Some("ASDF"));
        assert_eq!(decrypted_entry.borrow().get_uuid(), new_entry_uuid);

        assert!(node_is_equals_to(&decrypted_db.root, &root_group));
    }

    #[test]
    pub fn test_meta() {
        let mut db = Database::new(DatabaseConfig::default());

        let meta = Meta {
            generator: Some("test-generator".to_string()),
            database_name: Some("test-database-name".to_string()),
            database_name_changed: Some("2000-12-31T12:34:56".parse().unwrap()),
            database_description: Some("test-database-description".to_string()),
            database_description_changed: Some("2000-12-31T12:34:57".parse().unwrap()),
            default_username: Some("test-default-username".to_string()),
            default_username_changed: Some("2000-12-31T12:34:58".parse().unwrap()),
            maintenance_history_days: Some(123),
            color: Some("#C0FFEE".parse().unwrap()),
            master_key_changed: Some("2000-12-31T12:34:59".parse().unwrap()),
            master_key_change_rec: Some(-1),
            master_key_change_force: Some(42),
            memory_protection: Some(MemoryProtection {
                protect_title: true,
                protect_username: false,
                protect_password: true,
                protect_url: false,
                protect_notes: true,
            }),
            custom_icons: CustomIcons {
                icons: vec![Icon {
                    uuid: uuid!("a1a2a3a4b1bffffffffffff4d5d6d7d8"),
                    data: b"fake-data".to_vec(),
                }],
            },
            recyclebin_enabled: Some(true),
            recyclebin_uuid: Some(uuid!("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8")),
            recyclebin_changed: Some("2000-12-31T12:35:00".parse().unwrap()),
            entry_templates_group: Some(uuid!("123456789abcdef0d1d2d3d4d5d6d7d8")),
            entry_templates_group_changed: Some("2000-12-31T12:35:01".parse().unwrap()),
            last_selected_group: Some(uuid!("fffffffffffff1c2d1d2d3d4d5d6d7d8")),
            last_top_visible_group: Some(uuid!("a1a2a3a4b1b2c1c2d1d2d3ffffffffff")),
            history_max_items: Some(456),
            history_max_size: Some(789),
            settings_changed: Some("2000-12-31T12:35:02".parse().unwrap()),
            binaries: BinaryAttachments {
                binaries: vec![
                    BinaryAttachment {
                        identifier: Some("1".to_string()),
                        compressed: false,
                        content: b"i am binary data".to_vec(),
                    },
                    BinaryAttachment {
                        identifier: Some("2".to_string()),
                        compressed: true,
                        content: b"i am compressed binary data".to_vec(),
                    },
                    BinaryAttachment {
                        identifier: None,
                        compressed: true,
                        content: b"i am compressed binary data without an identifier".to_vec(),
                    },
                ],
            },
            custom_data: CustomData {
                items: HashMap::from([
                    (
                        "custom-data-key".to_string(),
                        CustomDataItem {
                            value: Some(Value::Unprotected("custom-data-value".to_string())),
                            last_modification_time: Some("2000-12-31T12:35:03".parse().unwrap()),
                        },
                    ),
                    (
                        "custom-data-key-without-value".to_string(),
                        CustomDataItem {
                            value: None,
                            last_modification_time: None,
                        },
                    ),
                    (
                        "custom-data-protected-key".to_string(),
                        CustomDataItem {
                            value: Some(Value::Protected(SecStr::new(b"custom-data-value".to_vec()))),
                            last_modification_time: Some("2000-12-31T12:35:03".parse().unwrap()),
                        },
                    ),
                ]),
            },
        };

        db.meta = meta.clone();

        let key_elements = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &key_elements, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &key_elements).unwrap();

        assert_eq!(decrypted_db.meta, meta);
    }

    #[test]
    fn test_deleted_objects() {
        let mut db = Database::new(DatabaseConfig::default());
        db.deleted_objects.objects = vec![
            DeletedObject {
                uuid: uuid!("123e4567-e89b-12d3-a456-426655440000"),
                deletion_time: "2000-12-31T12:34:56".parse().unwrap(),
            },
            DeletedObject {
                uuid: uuid!("00112233-4455-6677-8899-aabbccddeeff"),
                deletion_time: "2000-12-31T12:35:00".parse().unwrap(),
            },
        ];

        let key_elements = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &key_elements, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &key_elements).unwrap();

        assert_eq!(decrypted_db, db);
    }
}
