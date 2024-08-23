//! Types for representing data contained in a `KeePass` database

pub(crate) mod entry;
pub(crate) mod group;
pub(crate) mod iconid;
pub(crate) mod meta;
pub(crate) mod node;

#[cfg(feature = "totp")]
pub(crate) mod otp;

pub use crate::db::{
    entry::{AutoType, AutoTypeAssociation, Entry, History, Value},
    group::Group,
    meta::{BinaryAttachment, BinaryAttachments, CustomIcons, Icon, MemoryProtection, Meta},
    node::*,
};
use chrono::NaiveDateTime;
use std::{collections::HashMap, str::FromStr};
use uuid::Uuid;

#[cfg(feature = "totp")]
pub use crate::db::otp::{TOTPAlgorithm, TOTP};

use crate::{
    config::DatabaseConfig,
    db::iconid::IconId,
    error::{DatabaseIntegrityError, DatabaseOpenError, ParseColorError},
    format::{
        kdb::parse_kdb,
        kdbx3::{decrypt_kdbx3, parse_kdbx3},
        kdbx4::{decrypt_kdbx4, parse_kdbx4},
        DatabaseVersion,
    },
    key::DatabaseKey,
    rc_refcell_node,
};

/// A decrypted `KeePass` database
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Database {
    /// Configuration settings of the database such as encryption and compression algorithms
    pub config: DatabaseConfig,

    /// Binary attachments in the inner header
    pub header_attachments: Vec<HeaderAttachment>,

    /// Root node of the KeePass database
    pub root: SerializableNodePtr,

    /// References to previously-deleted objects
    pub deleted_objects: DeletedObjects,

    /// Metadata of the KeePass database
    pub meta: Meta,
}

impl PartialEq for Database {
    fn eq(&self, other: &Self) -> bool {
        self.config == other.config
            && self.header_attachments == other.header_attachments
            && self.deleted_objects == other.deleted_objects
            && self.meta == other.meta
            && node_is_equals_to(&self.root, &other.root)
    }
}

impl Eq for Database {}

impl Database {
    /// Parse a database from a `std::io::Read`
    pub fn open(source: &mut dyn std::io::Read, key: DatabaseKey) -> Result<Database, DatabaseOpenError> {
        let mut data = Vec::new();
        source.read_to_end(&mut data)?;

        Database::parse(data.as_ref(), key)
    }

    pub fn parse(data: &[u8], key: DatabaseKey) -> Result<Database, DatabaseOpenError> {
        let database_version = DatabaseVersion::parse(data)?;

        match database_version {
            DatabaseVersion::KDB(_) => parse_kdb(data, &key),
            DatabaseVersion::KDB2(_) => Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => parse_kdbx3(data, &key),
            DatabaseVersion::KDB4(_) => parse_kdbx4(data, &key),
        }
    }

    /// Save a database to a `std::io::Write`
    #[cfg(feature = "save_kdbx4")]
    pub fn save(&self, destination: &mut dyn std::io::Write, key: DatabaseKey) -> Result<(), crate::error::DatabaseSaveError> {
        use crate::error::DatabaseSaveError;
        use crate::format::kdbx4::dump_kdbx4;

        match self.config.version {
            DatabaseVersion::KDB(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB2(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB4(_) => dump_kdbx4(self, &key, destination),
        }
    }

    /// Helper function to load a database into its internal XML chunks
    pub fn get_xml(source: &mut dyn std::io::Read, key: DatabaseKey) -> Result<Vec<u8>, DatabaseOpenError> {
        let mut data = Vec::new();
        source.read_to_end(&mut data)?;

        let database_version = DatabaseVersion::parse(data.as_ref())?;

        let data = match database_version {
            DatabaseVersion::KDB(_) => return Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB2(_) => return Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => decrypt_kdbx3(data.as_ref(), &key)?.2,
            DatabaseVersion::KDB4(_) => decrypt_kdbx4(data.as_ref(), &key)?.3,
        };

        Ok(data)
    }

    /// Get the version of a database without decrypting it
    pub fn get_version(source: &mut dyn std::io::Read) -> Result<DatabaseVersion, DatabaseIntegrityError> {
        let mut data = vec![0; DatabaseVersion::get_version_header_size()];
        _ = source.read(&mut data)?;
        DatabaseVersion::parse(data.as_ref())
    }

    /// Create a new, empty database
    pub fn new(config: DatabaseConfig) -> Database {
        Self {
            config,
            header_attachments: Vec::new(),
            root: rc_refcell_node!(Group::new("Root")).into(),
            deleted_objects: DeletedObjects::default(),
            meta: Meta::new(),
        }
    }

    pub fn node_get_parents(&self, node: &NodePtr) -> Vec<Uuid> {
        let mut parents = Vec::new();
        let mut parent_uuid = node.borrow().get_parent();
        while let Some(uuid) = parent_uuid {
            parents.push(uuid);
            let parent_node = search_node_by_uuid_with_specific_type::<Group>(&self.root, uuid);
            parent_uuid = parent_node.and_then(|node| node.borrow().get_parent());
        }
        parents
    }

    pub fn set_recycle_bin_enabled(&mut self, enabled: bool) {
        self.meta.set_recycle_bin_enabled(enabled);
    }

    pub fn recycle_bin_enabled(&self) -> bool {
        self.meta.recycle_bin_enabled()
    }

    pub fn node_is_recycle_bin(&self, node: &NodePtr) -> bool {
        let uuid = node.borrow().get_uuid();
        node_is_group(node) && self.get_recycle_bin().map_or(false, |bin| bin.borrow().get_uuid() == uuid)
    }

    pub fn node_is_in_recycle_bin(&self, node: Uuid) -> bool {
        if let Some(node) = search_node_by_uuid(&self.root, node) {
            let parents = self.node_get_parents(&node);
            self.get_recycle_bin()
                .map(|bin| bin.borrow().get_uuid())
                .map_or(false, |uuid| parents.contains(&uuid))
        } else {
            false
        }
    }

    pub fn get_recycle_bin(&self) -> Option<NodePtr> {
        if !self.recycle_bin_enabled() {
            return None;
        }
        let uuid = self.meta.recyclebin_uuid?;
        group_get_children(&self.root).and_then(|children| {
            children
                .into_iter()
                .find(|child| child.borrow().get_uuid() == uuid && node_is_group(child))
        })
    }

    pub fn create_recycle_bin(&mut self) -> crate::Result<NodePtr> {
        use crate::error::Error;
        if !self.recycle_bin_enabled() {
            return Err(Error::RecycleBinDisabled);
        }
        if self.get_recycle_bin().is_some() {
            return Err(Error::RecycleBinAlreadyExists);
        }
        let recycle_bin = rc_refcell_node!(Group::new("Recycle Bin"));
        recycle_bin.borrow_mut().set_icon_id(Some(IconId::RECYCLE_BIN));
        self.meta.recyclebin_uuid = Some(recycle_bin.borrow().get_uuid());
        let count = group_get_children(&self.root).ok_or("")?.len();
        group_add_child(&self.root, recycle_bin.clone(), count)?;
        Ok(recycle_bin)
    }

    pub fn remove_node_by_uuid(&mut self, uuid: Uuid) -> crate::Result<NodePtr> {
        if !self.recycle_bin_enabled() {
            let node = group_remove_node_by_uuid(&self.root, uuid)?;
            self.deleted_objects.add(uuid);
            return Ok(node);
        }
        let node_in_recycle_bin = self.node_is_in_recycle_bin(uuid);
        let recycle_bin = self.get_recycle_bin().ok_or("").or_else(|_| self.create_recycle_bin())?;
        let recycle_bin_uuid = recycle_bin.borrow().get_uuid();
        // This can remove the recycle bin itself, or node in the recycle bin, or node not in the recycle bin
        let node = group_remove_node_by_uuid(&self.root, uuid)?;
        self.deleted_objects.add(uuid);
        if uuid != recycle_bin_uuid && !node_in_recycle_bin {
            group_add_child(&recycle_bin, node.clone(), 0)?;
        }
        self.meta.set_recycle_bin_changed();
        Ok(node)
    }

    pub fn search_node_by_uuid(&self, uuid: Uuid) -> Option<NodePtr> {
        search_node_by_uuid(&self.root, uuid)
    }

    fn create_new_node<T: Node + Default>(&self, parent: Uuid, index: usize) -> crate::Result<NodePtr> {
        let new_node = rc_refcell_node!(T::default());
        let parent = search_node_by_uuid_with_specific_type::<Group>(&self.root, parent)
            .or_else(|| Some(self.root.clone().into()))
            .ok_or("No parent node")?;
        if let Some(parent) = parent.borrow_mut().as_any_mut().downcast_mut::<Group>() {
            parent.add_child(new_node.clone(), index);
        };
        Ok(new_node)
    }

    pub fn create_new_entry(&self, parent: Uuid, index: usize) -> crate::Result<NodePtr> {
        self.create_new_node::<Entry>(parent, index)
    }

    pub fn create_new_group(&self, parent: Uuid, index: usize) -> crate::Result<NodePtr> {
        self.create_new_node::<Group>(parent, index)
    }
}

/// Timestamps for a Group or Entry
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Times {
    /// Does this node expire
    pub(crate) expires: bool,

    /// Number of usages
    pub(crate) usage_count: usize,

    /// Using chrono::NaiveDateTime which does not include timezone
    /// or UTC offset because KeePass clients typically store timestamps
    /// relative to the local time on the machine writing the data without
    /// including accurate UTC offset or timezone information.
    pub(crate) times: HashMap<String, NaiveDateTime>,
}

pub const EXPIRY_TIME_TAG_NAME: &str = "ExpiryTime";
pub const LAST_MODIFICATION_TIME_TAG_NAME: &str = "LastModificationTime";
pub const CREATION_TIME_TAG_NAME: &str = "CreationTime";
pub const LAST_ACCESS_TIME_TAG_NAME: &str = "LastAccessTime";
pub const LOCATION_CHANGED_TAG_NAME: &str = "LocationChanged";

impl Times {
    fn get(&self, key: &str) -> Option<NaiveDateTime> {
        self.times.get(key).copied()
    }

    fn set(&mut self, key: &str, time: Option<NaiveDateTime>) {
        if let Some(time) = time {
            self.times.insert(key.to_string(), time);
        } else {
            self.times.remove(key);
        }
    }

    pub fn get_expires(&self) -> bool {
        self.expires
    }

    pub fn set_expires(&mut self, expires: bool) {
        self.expires = expires;
    }

    pub fn get_usage_count(&self) -> usize {
        self.usage_count
    }

    pub fn set_usage_count(&mut self, usage_count: usize) {
        self.usage_count = usage_count;
    }

    /// Convenience method for getting the time that the entry expires.
    /// This value is usually only meaningful/useful when expires == true
    pub fn get_expiry_time(&self) -> Option<NaiveDateTime> {
        self.get(EXPIRY_TIME_TAG_NAME)
    }

    pub fn set_expiry_time(&mut self, time: Option<NaiveDateTime>) {
        self.set(EXPIRY_TIME_TAG_NAME, time);
    }

    pub fn get_last_modification(&self) -> Option<NaiveDateTime> {
        self.get(LAST_MODIFICATION_TIME_TAG_NAME)
    }

    pub fn set_last_modification(&mut self, time: Option<NaiveDateTime>) {
        self.set(LAST_MODIFICATION_TIME_TAG_NAME, time);
    }

    pub fn get_creation(&self) -> Option<NaiveDateTime> {
        self.get(CREATION_TIME_TAG_NAME)
    }

    pub fn set_creation(&mut self, time: Option<NaiveDateTime>) {
        self.set(CREATION_TIME_TAG_NAME, time);
    }

    pub fn get_last_access(&self) -> Option<NaiveDateTime> {
        self.get(LAST_ACCESS_TIME_TAG_NAME)
    }

    pub fn set_last_access(&mut self, time: Option<NaiveDateTime>) {
        self.set(LAST_ACCESS_TIME_TAG_NAME, time);
    }

    pub fn get_location_changed(&self) -> Option<NaiveDateTime> {
        self.get(LOCATION_CHANGED_TAG_NAME)
    }

    pub fn set_location_changed(&mut self, time: Option<NaiveDateTime>) {
        self.set(LOCATION_CHANGED_TAG_NAME, time);
    }

    // Returns the current time, without the nanoseconds since
    // the last leap second.
    pub fn now() -> NaiveDateTime {
        let now = chrono::Utc::now().naive_utc().and_utc().timestamp();
        chrono::DateTime::from_timestamp(now, 0).unwrap().naive_utc()
    }

    pub fn epoch() -> NaiveDateTime {
        chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
    }

    pub fn new() -> Times {
        let mut response = Times::default();
        let now = Some(Times::now());
        response.set_creation(now);
        response.set_last_modification(now);
        response.set_last_access(now);
        response.set_location_changed(now);
        response.set_expiry_time(now);
        response.set_expires(false);
        response
    }
}

/// Collection of custom data fields for an entry or metadata
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct CustomData {
    pub items: HashMap<String, CustomDataItem>,
}

/// Custom data field for an entry or metadata for internal use
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct CustomDataItem {
    pub value: Option<Value>,
    pub last_modification_time: Option<NaiveDateTime>,
}

/// Custom data field for an entry or metadata from XML data
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct CustomDataItemDenormalized {
    pub key: String,
    pub custom_data_item: CustomDataItem,
}

/// Binary attachments stored in a database inner header
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct HeaderAttachment {
    pub flags: u8,
    pub content: Vec<u8>,
}

/// Elements that have been previously deleted
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct DeletedObjects {
    pub objects: Vec<DeletedObject>,
}

impl DeletedObjects {
    pub fn add(&mut self, uuid: Uuid) {
        let deletion_time = Times::now();
        if let Some(item) = self.objects.iter_mut().find(|item| item.uuid == uuid) {
            item.deletion_time = deletion_time;
        } else {
            self.objects.push(DeletedObject { uuid, deletion_time });
        }
    }
}

/// A reference to a deleted element
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct DeletedObject {
    pub uuid: Uuid,
    pub deletion_time: NaiveDateTime,
}

/// A color value for the Database, or Entry
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[cfg(feature = "serialization")]
impl serde::Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        if !str.starts_with('#') || str.len() != 7 {
            return Err(ParseColorError(str.to_string()));
        }

        let var = u64::from_str_radix(str.trim_start_matches('#'), 16).map_err(|_e| ParseColorError(str.to_string()))?;

        let r = ((var >> 16) & 0xff) as u8;
        let g = ((var >> 8) & 0xff) as u8;
        let b = (var & 0xff) as u8;

        Ok(Self { r, g, b })
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:0x}{:0x}{:0x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod database_tests {
    #[cfg(feature = "save_kdbx4")]
    use crate::{config::DatabaseConfig, db::Entry, db::NodePtr};
    use crate::{Database, DatabaseKey, Result};
    use std::fs::File;

    #[test]
    fn test_xml() -> Result<()> {
        let key = DatabaseKey::new().with_password("demopass");
        let mut f = File::open("tests/resources/test_db_with_password.kdbx")?;
        let xml = Database::get_xml(&mut f, key)?;

        assert!(xml.len() > 100);

        Ok(())
    }

    #[test]
    fn test_open_invalid_version_header_size() {
        assert!(Database::parse(&[], DatabaseKey::new().with_password("testing")).is_err());
        assert!(Database::parse(&[0, 0, 0, 0, 0, 0, 0, 0], DatabaseKey::new().with_password("testing")).is_err());
        assert!(Database::parse(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], DatabaseKey::new().with_password("testing")).is_err());
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn test_save() -> Result<()> {
        use crate::{db::group_add_child, db::Group, rc_refcell_node};

        let db = Database::new(DatabaseConfig::default());

        group_add_child(&db.root, rc_refcell_node!(Entry::default()), 0).unwrap();
        group_add_child(&db.root, rc_refcell_node!(Entry::default()), 1).unwrap();
        group_add_child(&db.root, rc_refcell_node!(Entry::default()), 2).unwrap();

        let group = rc_refcell_node!(Group::new("my group"));
        group_add_child(&group, rc_refcell_node!(Entry::default()), 0).unwrap();
        group_add_child(&group, rc_refcell_node!(Entry::default()), 1).unwrap();
        group_add_child(&db.root, group, 3).unwrap();

        let mut buffer = Vec::new();
        let key = DatabaseKey::new().with_password("testing");

        db.save(&mut buffer, key.clone())?;

        let db_loaded = Database::open(&mut buffer.as_slice(), key)?;

        assert_eq!(db, db_loaded);
        Ok(())
    }
}
