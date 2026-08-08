pub mod custom_serde;
pub mod entry;
pub mod group;
pub mod meta;
pub mod parse;
pub mod times;
pub mod timestamp;

use crate::crypt::ciphers::Cipher;
#[cfg(feature = "save_kdbx4")]
use crate::db::Database;
use crate::db::{Attachment, Group, Meta};
use base64::{Engine as _, engine::general_purpose as base64_engine};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use uuid::Uuid;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UUID(pub Uuid);

impl<'de> Deserialize<'de> for UUID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        let value = base64_engine::STANDARD.decode(input).map_err(serde::de::Error::custom)?;
        let uuid = Uuid::from_slice(&value).map_err(serde::de::Error::custom)?;
        Ok(Self(uuid))
    }
}

impl Serialize for UUID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64_engine::STANDARD.encode(self.0.as_bytes()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct KeePassFileXml {
    meta: meta::MetaXml,
    root: RootXml,
}

#[derive(Debug, Serialize, Deserialize)]
struct RootXml {
    #[serde(rename = "Group")]
    group: group::GroupXml,

    #[serde(default, rename = "DeletedObjects")]
    deleted_objects: Option<DeletedObjectsXml>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeletedObjectsXml {
    #[serde(default, rename = "DeletedObject")]
    objects: Vec<DeletedObjectXml>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeletedObjectXml {
    #[serde(rename = "UUID")]
    uuid: UUID,

    #[serde(default, with = "custom_serde::cs_opt_string")]
    deletion_time: Option<timestamp::Timestamp>,
}

pub(crate) struct ParsedXml {
    pub(crate) meta: Meta,
    pub(crate) root_group: Group,
    pub(crate) deleted_objects: HashMap<Uuid, Option<chrono::NaiveDateTime>>,
}

pub(crate) fn parse_xml_bytes(
    data: &[u8],
    header_attachments: &[Attachment],
    inner_cipher: &mut dyn Cipher,
) -> Result<ParsedXml, parse::XmlParseError> {
    let mut parsed: KeePassFileXml = quick_xml::de::from_reader(data)?;

    let attachments = if header_attachments.is_empty() {
        let mut attachments = Vec::new();
        if let Some(binaries) = parsed.meta.binaries.take() {
            for binary in binaries.binaries {
                let id = binary.id;
                let data = binary
                    .xml_to_db(inner_cipher)
                    .map_err(|error| parse::XmlParseError::Schema(error.to_string()))?;
                if attachments.len() <= id {
                    attachments.resize_with(id + 1, Attachment::default);
                }
                attachments[id] = Attachment { data };
            }
        }
        attachments
    } else {
        header_attachments.to_vec()
    };

    let custom_icons = parsed
        .meta
        .custom_icons
        .as_ref()
        .map(|icons| icons.icons.iter().map(|icon| (icon.uuid.0, icon.data.clone())).collect())
        .unwrap_or_default();
    let meta = parsed.meta.into();

    let root_xml = parsed.root.group;
    let mut root_group = Group {
        uuid: root_xml.uuid.0,
        ..Default::default()
    };
    root_xml
        .xml_to_db_handle(&mut root_group, &attachments, &custom_icons, inner_cipher)
        .map_err(|error| parse::XmlParseError::Schema(error.to_string()))?;

    let deleted_objects = parsed
        .root
        .deleted_objects
        .unwrap_or(DeletedObjectsXml { objects: Vec::new() })
        .objects
        .into_iter()
        .filter_map(|item| item.deletion_time.map(|deletion_time| (item.uuid.0, Some(deletion_time.into()))))
        .collect();

    Ok(ParsedXml {
        meta,
        root_group,
        deleted_objects,
    })
}

#[cfg(feature = "save_kdbx4")]
pub(crate) fn to_xml_bytes(db: &Database, inner_cipher: &mut dyn Cipher) -> Result<(Vec<Attachment>, Vec<u8>), parse::XmlParseError> {
    let root_group = db
        .root
        .borrow()
        .downcast_ref::<Group>()
        .ok_or_else(|| parse::XmlParseError::Schema("database root node is not a group".to_string()))?
        .clone();

    let mut attachments = Vec::new();
    let mut custom_icons = HashMap::new();
    let group = group::GroupXml::db_to_xml(&root_group, inner_cipher, &mut attachments, &mut custom_icons)
        .map_err(|error| parse::XmlParseError::Schema(error.to_string()))?;

    let mut meta_xml = meta::MetaXml::from(db.meta.clone());
    if !custom_icons.is_empty() {
        meta_xml.custom_icons = Some(meta::CustomIconsXml {
            icons: custom_icons
                .into_iter()
                .map(|(uuid, data)| meta::IconXml { uuid: UUID(uuid), data })
                .collect(),
        });
    }

    let deleted_objects = if db.deleted_objects.is_empty() {
        None
    } else {
        Some(DeletedObjectsXml {
            objects: db
                .deleted_objects
                .iter()
                .map(|(uuid, deletion_time)| DeletedObjectXml {
                    uuid: UUID(*uuid),
                    deletion_time: deletion_time.map(timestamp::Timestamp::new_base64),
                })
                .collect(),
        })
    };

    let xml = quick_xml::se::to_string_with_root(
        "KeePassFile",
        &KeePassFileXml {
            meta: meta_xml,
            root: RootXml { group, deleted_objects },
        },
    )
    .map_err(parse::XmlParseError::XmlSerialize)?;

    Ok((attachments, xml.into_bytes()))
}

#[cfg(feature = "save_kdbx4")]
#[cfg(test)]
mod tests {
    use crate::{
        config::{DatabaseConfig, InnerCipherConfig},
        db::{
            AutoType, AutoTypeAssociation, CustomDataItem, CustomDataValue, Database, Entry, Group, History, IconId, MemoryProtection,
            Meta, Times, group_get_children, node::*, node_is_equals_to, rc_refcell_node,
        },
        format::kdbx4,
        format::xml_db::group::GroupXml,
        key::DatabaseKey,
    };
    use chrono::NaiveDateTime;
    use std::collections::HashMap;
    use uuid::uuid;

    fn make_key() -> DatabaseKey {
        let mut password_bytes: Vec<u8> = vec![];
        let mut password: String = "".to_string();
        password_bytes.resize(40, 0);
        getrandom::fill(&mut password_bytes).unwrap();
        for random_char in password_bytes {
            password += &std::char::from_u32(random_char as u32).unwrap().to_string();
        }

        DatabaseKey::new().with_password(&password)
    }

    #[test]
    pub fn test_entry() {
        let mut entry = Entry::default();

        entry.set_title(Some("ASDF"));
        entry.set_username(Some("ghj"));
        entry.set_password(Some("klmno"));
        entry.tags.push("test".to_string());
        entry.tags.push("keepass-ng".to_string());
        entry.times.set_expires(true);
        entry.times.set_usage_count(42);
        entry.times.set_creation(Some(NaiveDateTime::default()));
        entry.times.set_expiry_time(Some(NaiveDateTime::default()));
        entry.times.set_last_access(Some(NaiveDateTime::default()));
        entry.times.set_location_changed(Some(Times::now()));
        entry.times.set_last_modification(Some(Times::now()));

        entry.set_autotype(Some(AutoType {
            enabled: true,
            default_sequence: Some("Autotype-sequence".to_string()),
            data_transfer_obfuscation: None,
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

        entry.custom_data.insert(
            "CDI-key".to_string(),
            CustomDataItem {
                value: Some(CustomDataValue::String("CDI-Value".to_string())),
                last_modification_time: Some(NaiveDateTime::default()),
            },
        );

        entry.icon_id = Some(IconId::KEY);
        entry.custom_icon = Some((uuid!("22222222222222222222222222222222"), Vec::new()));

        entry.foreground_color = Some("#C0FFEE".parse().unwrap());
        entry.background_color = Some("#1C1357".parse().unwrap());

        entry.override_url = Some("https://docs.rs/keepass-ng/".to_string());
        entry.quality_check = Some(true);

        let mut history = History::default();
        history.entries.push(entry.clone());

        entry.history = Some(history);

        let entry = rc_refcell_node(entry);

        let root_group = rc_refcell_node(Group::new("Root"));
        group_add_child(&root_group, entry.borrow().duplicate(), 0).unwrap();

        let mut db = Database::new(DatabaseConfig::default());
        db.root = root_group.into();

        let db_key = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &db_key, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &db_key).unwrap();

        assert_eq!(group_get_children(&decrypted_db.root).unwrap().len(), 1);

        let decrypted_entry = &group_get_children(&decrypted_db.root).unwrap()[0];
        // decrypted_entry.borrow_mut().set_parent(None);
        assert!(node_is_equals_to(decrypted_entry, &entry));
    }

    #[test]
    pub fn test_group() {
        let group = Group::new("");
        let mut inner_cipher = InnerCipherConfig::Plain.get_cipher(&[]);
        let mut attachments = Vec::new();
        let mut custom_icons = HashMap::new();
        let group_xml = GroupXml::db_to_xml(&group, &mut *inner_cipher, &mut attachments, &mut custom_icons).unwrap();
        let xml = quick_xml::se::to_string_with_root("Group", &group_xml).unwrap();
        assert!(xml.contains("<Name/>"), "serialized group XML: {xml}");

        let root_group = rc_refcell_node(Group::new("Root"));
        let entry = rc_refcell_node(Entry::default());
        let new_entry_uuid = entry.borrow().get_uuid();
        entry.borrow_mut().set_title(Some("ASDF"));

        group_add_child(&root_group, entry, 0).unwrap();

        let subgroup = rc_refcell_node(Group::new("Child group"));
        with_node_mut::<Group, _, _>(&subgroup, |subgroup| {
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
            subgroup.enable_autotype = Some(true);
            subgroup.enable_searching = Some(false);

            subgroup.last_top_visible_entry = Some(uuid!("43210000000000000000000000000000"));

            subgroup.custom_data.insert(
                "CustomOption".to_string(),
                CustomDataItem {
                    value: Some(CustomDataValue::String("CustomOption-Value".to_string())),
                    last_modification_time: Some(NaiveDateTime::default()),
                },
            );
        })
        .unwrap();

        group_add_child(&root_group, subgroup, 1).unwrap();

        let mut db = Database::new(DatabaseConfig::default());
        db.root = root_group.borrow().duplicate().into();

        let db_key = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &db_key, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &db_key).unwrap();

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
            custom_data: HashMap::from([
                (
                    "custom-data-key".to_string(),
                    CustomDataItem {
                        value: Some(CustomDataValue::String("custom-data-value".to_string())),
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
                        value: Some(CustomDataValue::String("custom-data-value".to_string())),
                        last_modification_time: Some("2000-12-31T12:35:03".parse().unwrap()),
                    },
                ),
            ]),
        };

        db.meta = meta.clone();

        let db_key = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &db_key, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &db_key).unwrap();

        assert_eq!(decrypted_db.meta, meta);
    }

    #[test]
    fn test_deleted_objects() {
        let mut db = Database::new(DatabaseConfig::default());
        db.deleted_objects.insert(
            uuid!("123e4567-e89b-12d3-a456-426655440000"),
            Some("2000-12-31T12:34:56".parse().unwrap()),
        );
        db.deleted_objects.insert(
            uuid!("00112233-4455-6677-8899-aabbccddeeff"),
            Some("2000-12-31T12:35:00".parse().unwrap()),
        );

        let db_key = make_key();

        let mut encrypted_db = Vec::new();
        kdbx4::dump_kdbx4(&db, &db_key, &mut encrypted_db).unwrap();
        let decrypted_db = kdbx4::parse_kdbx4(&encrypted_db, &db_key).unwrap();

        assert_eq!(decrypted_db, db);
    }
}
