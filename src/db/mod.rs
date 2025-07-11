//! Types for representing data contained in a `KeePass` database

pub(crate) mod entry;
pub(crate) mod group;
pub(crate) mod iconid;
#[cfg(feature = "merge")]
pub(crate) mod merge;
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
pub use crate::db::otp::{TOTP, TOTPAlgorithm};

#[cfg(feature = "merge")]
use crate::db::merge::{MergeError, MergeEvent, MergeEventType, MergeLog};

#[cfg(feature = "merge")]
use std::collections::VecDeque;

use crate::{
    config::DatabaseConfig,
    db::iconid::IconId,
    error::{DatabaseIntegrityError, DatabaseOpenError, ParseColorError},
    format::{
        DatabaseVersion,
        kdb::parse_kdb,
        kdbx3::{decrypt_kdbx3, parse_kdbx3},
        kdbx4::{decrypt_kdbx4, parse_kdbx4},
    },
    key::DatabaseKey,
};

/// A decrypted `KeePass` database
#[derive(Debug)]
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

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            header_attachments: self.header_attachments.clone(),
            root: self.root.borrow().duplicate().into(),
            deleted_objects: self.deleted_objects.clone(),
            meta: self.meta.clone(),
        }
    }
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
            root: rc_refcell_node(Group::new("Root")).into(),
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
        node_is_group(node) && self.get_recycle_bin().is_some_and(|bin| bin.borrow().get_uuid() == uuid)
    }

    pub fn node_is_in_recycle_bin(&self, node: Uuid) -> bool {
        if let Some(node) = search_node_by_uuid(&self.root, node) {
            let parents = self.node_get_parents(&node);
            self.get_recycle_bin()
                .map(|bin| bin.borrow().get_uuid())
                .is_some_and(|uuid| parents.contains(&uuid))
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
        let recycle_bin = rc_refcell_node(Group::new("Recycle Bin"));
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
        let new_node = rc_refcell_node(T::default());
        let parent = search_node_by_uuid_with_specific_type::<Group>(&self.root, parent)
            .or_else(|| Some(self.root.clone().into()))
            .ok_or("No parent node")?;
        with_node_mut::<Group, _, _>(&parent, |parent| {
            parent.add_child(new_node.clone(), index);
        });
        Ok(new_node)
    }

    pub fn create_new_entry(&self, parent: Uuid, index: usize) -> crate::Result<NodePtr> {
        self.create_new_node::<Entry>(parent, index)
    }

    pub fn create_new_group(&self, parent: Uuid, index: usize) -> crate::Result<NodePtr> {
        self.create_new_node::<Group>(parent, index)
    }

    /// Merge this database with another version of this same database.
    /// This function will use the UUIDs to detect that entries and groups are
    /// the same.
    #[cfg(feature = "merge")]
    pub fn merge(&mut self, other: &Database) -> Result<MergeLog, MergeError> {
        let mut log = MergeLog::default();
        log.append(&self.merge_group(&[], &other.root, false)?);
        log.append(&self.merge_deletions(other)?);
        Ok(log)
    }

    #[cfg(feature = "merge")]
    fn merge_deletions(&mut self, other: &Database) -> Result<MergeLog, MergeError> {
        // Utility function to search for a UUID in the VecDeque of deleted objects.
        let is_in_deleted_queue = |uuid: Uuid, deleted_groups_queue: &VecDeque<DeletedObject>| -> bool {
            for deleted_object in deleted_groups_queue {
                // This group still has a child group, but it is not going to be deleted.
                if deleted_object.uuid == uuid {
                    return true;
                }
            }
            false
        };
        let mut log = MergeLog::default();
        let mut new_deleted_objects = self.deleted_objects.clone();
        // We start by deleting the entries, since we will only remove groups if they are empty.
        for deleted_object in &other.deleted_objects.objects {
            if new_deleted_objects.contains(deleted_object.uuid) {
                continue;
            }
            let entry_location = match Self::find_node_location(&self.root, deleted_object.uuid) {
                Some(l) => l,
                None => continue,
            };
            let parent_group = Group::find_group(&self.root, &entry_location).ok_or(MergeError::FindGroupError(entry_location))?;

            let entry = match Group::find_entry(&parent_group, &[deleted_object.uuid]) {
                Some(e) => e,
                // This uuid might refer to a group, which will be handled later.
                None => continue,
            };

            let entry_last_modification = match with_node::<Entry, _, _>(&entry, |e| e.get_times().get_last_modification()).unwrap() {
                Some(t) => t,
                None => {
                    log.warnings.push(format!(
                        "Entry {} did not have a last modification timestamp",
                        entry.borrow().downcast_ref::<Entry>().unwrap().uuid
                    ));
                    Times::now()
                }
            };
            if entry_last_modification < deleted_object.deletion_time {
                with_node_mut::<Group, _, _>(&parent_group, |pg| pg.remove_node(deleted_object.uuid)).unwrap()?;
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryDeleted,
                    node_uuid: deleted_object.uuid,
                });
                new_deleted_objects.objects.push(deleted_object.clone());
            }
        }
        let mut deleted_groups_queue: VecDeque<DeletedObject> = vec![].into();
        for deleted_object in &other.deleted_objects.objects {
            if new_deleted_objects.contains(deleted_object.uuid) {
                continue;
            }
            deleted_groups_queue.push_back(deleted_object.clone());
        }
        while !deleted_groups_queue.is_empty() {
            let deleted_object = deleted_groups_queue.pop_front().unwrap();
            if new_deleted_objects.contains(deleted_object.uuid) {
                continue;
            }
            let group_location = match Self::find_node_location(&self.root, deleted_object.uuid) {
                Some(l) => l,
                None => continue,
            };
            let parent_group = Group::find_group(&self.root, &group_location).ok_or(MergeError::FindGroupError(group_location))?;

            let group = match Group::find_group(&parent_group, &[deleted_object.uuid]) {
                Some(g) => g,
                None => {
                    // The node might be an entry, since we didn't necessarily removed all the
                    // entries that were in the deleted objects of the source database.
                    continue;
                }
            };
            // Not deleting a group if it still has entries.
            if !with_node::<Group, _, _>(&group, |g| g.entries()).unwrap().is_empty() {
                continue;
            }
            // This group still has a child group that might get deleted in the future, so we delay
            // decision to delete it or not.
            if !with_node::<Group, _, _>(&group, |g| g.groups())
                .unwrap()
                .iter()
                .filter(|&g| !is_in_deleted_queue(g.borrow().get_uuid(), &deleted_groups_queue))
                .collect::<Vec<_>>()
                .is_empty()
            {
                deleted_groups_queue.push_back(deleted_object.clone());
                continue;
            }
            // This group still a groups that won't be deleted, so we don't delete it.
            if !with_node::<Group, _, _>(&group, |g| g.groups()).unwrap().is_empty() {
                continue;
            }
            let group_last_modification = match with_node::<Group, _, _>(&group, |g| g.get_times().get_last_modification()).unwrap() {
                Some(t) => t,
                None => {
                    log.warnings.push(format!(
                        "Group {} did not have a last modification timestamp",
                        group.borrow().downcast_ref::<Group>().unwrap().uuid
                    ));
                    Times::now()
                }
            };
            if group_last_modification < deleted_object.deletion_time {
                with_node_mut::<Group, _, _>(&parent_group, |pg| pg.remove_node(deleted_object.uuid)).unwrap()?;
                log.events.push(MergeEvent {
                    event_type: MergeEventType::GroupDeleted,
                    node_uuid: deleted_object.uuid,
                });
                new_deleted_objects.objects.push(deleted_object.clone());
            }
        }
        self.deleted_objects = new_deleted_objects;
        Ok(log)
    }

    #[cfg(feature = "merge")]
    pub(crate) fn find_node_location(root: &NodePtr, id: Uuid) -> Option<Vec<Uuid>> {
        // let root_uuid = root.borrow().get_uuid();
        // let mut current_location = vec![root_uuid];
        for node in &group_get_children(root).unwrap_or_default() {
            let node_uuid = node.borrow().get_uuid();
            if node_is_entry(node) {
                if node_uuid == id {
                    // current_location.push(node_uuid);
                    // return Some(current_location);
                    return Some(vec![]);
                }
            } else if node_is_group(node) {
                if node_uuid == id {
                    // current_location.push(node_uuid);
                    // return Some(current_location);
                    return Some(vec![]);
                }
                #[allow(unused_mut)]
                if let Some(mut location) = Group::find_node_location(node, id) {
                    // current_location.append(&mut location);
                    // return Some(current_location);
                    return Some(location);
                }
            }
        }
        None
    }

    #[cfg(feature = "merge")]
    fn merge_group(&self, current_group_path: &[Uuid], current_group: &NodePtr, is_in_deleted_group: bool) -> Result<MergeLog, MergeError> {
        let mut log = MergeLog::default();
        if let Some(destination_group_location) = Self::find_node_location(&self.root, current_group.borrow().get_uuid()) {
            let mut destination_group_path = destination_group_location.clone();
            destination_group_path.push(current_group.borrow().get_uuid());
            let destination_group =
                Group::find_group(&self.root, &destination_group_path).ok_or(MergeError::FindGroupError(destination_group_path))?;
            let group_update_merge_events = Group::merge_with(&destination_group, current_group)?;
            log.append(&group_update_merge_events);
        }
        for other_entry in &with_node::<Group, _, _>(current_group, |g| g.entries()).unwrap() {
            let other_entry_uuid = other_entry.borrow().get_uuid();
            // find the existing location
            let destination_entry_location = Self::find_node_location(&self.root, other_entry_uuid);
            // The group already exists in the destination database.
            if let Some(destination_entry_location) = destination_entry_location {
                let mut existing_entry_location = destination_entry_location.clone();
                existing_entry_location.push(other_entry_uuid);
                // The entry already exists but is not at the right location. We might have to
                // relocate it.
                let existing_entry = Group::find_entry(&self.root, &existing_entry_location)
                    .ok_or(MergeError::FindEntryError(existing_entry_location.clone()))?
                    .borrow()
                    .duplicate();
                // The entry already exists but is not at the right location. We might have to
                // relocate it.
                if current_group_path.last() != destination_entry_location.last() && !is_in_deleted_group {
                    let source_location_changed_time =
                        match with_node::<Entry, _, _>(other_entry, |e| e.get_times().get_location_changed()).unwrap() {
                            Some(t) => t,
                            None => {
                                log.warnings
                                    .push(format!("Entry {other_entry_uuid} did not have a location updated timestamp"));
                                Times::epoch()
                            }
                        };
                    let destination_location_changed =
                        match with_node::<Entry, _, _>(&existing_entry, |e| e.get_times().get_location_changed()).unwrap() {
                            Some(t) => t,
                            None => {
                                log.warnings
                                    .push(format!("Entry {other_entry_uuid} did not have a location updated timestamp"));
                                Times::now()
                            }
                        };
                    if source_location_changed_time > destination_location_changed {
                        log.events.push(MergeEvent {
                            event_type: MergeEventType::EntryLocationUpdated,
                            node_uuid: other_entry_uuid,
                        });
                        self.relocate_node(
                            other_entry_uuid,
                            &destination_entry_location,
                            current_group_path,
                            source_location_changed_time,
                        )?;
                        // Update the location of the current entry in case we have to update it
                        // after.
                        existing_entry_location = current_group_path.to_owned();
                        existing_entry_location.push(other_entry_uuid);
                        with_node_mut::<Entry, _, _>(&existing_entry, |e| {
                            e.get_times_mut().set_location_changed(Some(source_location_changed_time));
                        });
                    }
                }
                if !has_diverged_from(&existing_entry, other_entry) {
                    continue;
                }
                // The entry already exists and is at the right location, so we can proceed and merge
                // the two entries.
                let (merged_entry, entry_merge_log) = Entry::merge(&existing_entry, other_entry)?;
                let merged_entry = match merged_entry {
                    Some(m) => m,
                    None => continue,
                };
                if node_is_equals_to(&existing_entry, &merged_entry) {
                    continue;
                }
                let existing_entry =
                    Group::find_entry(&self.root, &existing_entry_location).ok_or(MergeError::FindEntryError(existing_entry_location))?;
                // *existing_entry = merged_entry.clone();
                with_node_mut::<Entry, _, _>(&existing_entry, |e| e.replaced_with(&merged_entry)).unwrap();
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryUpdated,
                    node_uuid: merged_entry.borrow().get_uuid(),
                });
                log.append(&entry_merge_log);
                continue;
            }
            if self.deleted_objects.contains(other_entry_uuid) {
                continue;
            }
            // We don't create new entries that exist under a deleted group.
            if is_in_deleted_group {
                continue;
            }
            // The entry doesn't exist in the destination, we create it
            // let new_entry = other_entry.to_owned().clone();
            let new_entry = other_entry.borrow().duplicate();
            let new_entry_parent_group =
                Group::find_group(&self.root, current_group_path).ok_or(MergeError::FindGroupError(current_group_path.to_owned()))?;

            // new_entry_parent_group.add_child(new_entry.clone());
            group_add_child(&new_entry_parent_group, new_entry.clone(), 0).unwrap();
            // TODO should we update the time info for the entry?
            log.events.push(MergeEvent {
                event_type: MergeEventType::EntryCreated,
                node_uuid: new_entry.borrow().get_uuid(),
            });
        }
        for other_group in &current_group.borrow().downcast_ref::<Group>().unwrap().groups() {
            let mut new_group_location = current_group_path.to_owned();
            let other_group_uuid = other_group.borrow().get_uuid();
            new_group_location.push(other_group_uuid);
            if self.deleted_objects.contains(other_group_uuid) || is_in_deleted_group {
                let new_merge_log = self.merge_group(&new_group_location, other_group, true)?;
                log.append(&new_merge_log);
                continue;
            }
            let destination_group_location = Self::find_node_location(&self.root, other_group_uuid);
            // The group already exists in the destination database.
            if let Some(destination_group_location) = &destination_group_location {
                if current_group_path != destination_group_location {
                    let mut existing_group_location = destination_group_location.clone();
                    existing_group_location.push(other_group_uuid);
                    // The group already exists but is not at the right location. We might have to
                    // relocate it.
                    let existing_group = Group::find_group(&self.root, &existing_group_location)
                        .ok_or(MergeError::FindGroupError(existing_group_location))?;
                    let existing_group_location_changed =
                        match with_node::<Group, _, _>(&existing_group, |g| g.get_times().get_location_changed()).unwrap() {
                            Some(t) => t,
                            None => {
                                let uuid = existing_group.borrow().get_uuid();
                                log.warnings.push(format!("Entry {uuid} did not have a location changed timestamp"));
                                Times::now()
                            }
                        };
                    let other_group_location_changed =
                        match with_node::<Group, _, _>(other_group, |g| g.get_times().get_location_changed()).unwrap() {
                            Some(t) => t,
                            None => {
                                log.warnings
                                    .push(format!("Entry {other_group_uuid} did not have a location changed timestamp"));
                                Times::epoch()
                            }
                        };
                    // The other group was moved after the current group, so we have to relocate it.
                    if existing_group_location_changed < other_group_location_changed {
                        self.relocate_node(
                            other_group_uuid,
                            destination_group_location,
                            current_group_path,
                            other_group_location_changed,
                        )?;
                        log.events.push(MergeEvent {
                            event_type: MergeEventType::GroupLocationUpdated,
                            node_uuid: other_group_uuid,
                        });
                        let new_merge_log = self.merge_group(&new_group_location, other_group, is_in_deleted_group)?;
                        log.append(&new_merge_log);
                        continue;
                    }
                }
                // The group already exists and is at the right location, so we can proceed and merge
                // the two groups.
                let new_merge_log = self.merge_group(&new_group_location, other_group, is_in_deleted_group)?;
                log.append(&new_merge_log);
                continue;
            }
            // The group doesn't exist in the destination, we create it
            // let mut new_group = other_group.to_owned().clone();
            let new_group = other_group.borrow().duplicate();
            // new_group.children = vec![];
            with_node_mut::<Group, _, _>(&new_group, |g| g.reset_children(vec![])).unwrap();
            log.events.push(MergeEvent {
                event_type: MergeEventType::GroupCreated,
                node_uuid: new_group.borrow().get_uuid(),
            });
            let new_group_parent_group =
                Group::find_group(&self.root, current_group_path).ok_or(MergeError::FindGroupError(current_group_path.to_owned()))?;
            with_node_mut::<Group, _, _>(&new_group_parent_group, |g| g.add_child(new_group, 0)).unwrap();
            let new_merge_log = self.merge_group(&new_group_location, other_group, is_in_deleted_group)?;
            log.append(&new_merge_log);
        }
        Ok(log)
    }
    #[cfg(feature = "merge")]
    fn relocate_node(
        &self,
        node_uuid: Uuid,
        from: &[Uuid],
        to: &[Uuid],
        new_location_changed_timestamp: NaiveDateTime,
    ) -> Result<(), MergeError> {
        let source_group = Group::find_group(&self.root, from).ok_or(MergeError::FindGroupError(from.to_owned()))?;
        let relocated_node = with_node_mut::<Group, _, _>(&source_group, |s| s.remove_node(node_uuid)).unwrap()?;
        relocated_node
            .borrow_mut()
            .get_times_mut()
            .set_location_changed(Some(new_location_changed_timestamp));

        let destination_group = Group::find_group(&self.root, to).ok_or(MergeError::FindGroupError(to.to_owned()))?;
        group_add_child(&destination_group, relocated_node, 0).unwrap();
        Ok(())
    }
}

#[cfg(feature = "merge")]
pub(crate) fn has_diverged_from(node: &NodePtr, other_node: &NodePtr) -> bool {
    if let Some(entry) = node.borrow().downcast_ref::<Entry>()
        && let Some(other_entry) = other_node.borrow().downcast_ref::<Entry>()
    {
        return entry._has_diverged_from(other_entry);
    }
    if let Some(group) = node.borrow().downcast_ref::<Group>()
        && let Some(other_group) = other_node.borrow().downcast_ref::<Group>()
    {
        return group._has_diverged_from(other_group);
    }
    false
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
        let now = chrono::Utc::now().timestamp();
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
    pub fn contains(&self, uuid: Uuid) -> bool {
        self.objects.iter().any(|deleted_object| deleted_object.uuid == uuid)
    }
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
    use crate::{
        Result,
        db::{Database, DatabaseKey},
    };
    #[cfg(feature = "save_kdbx4")]
    use crate::{config::DatabaseConfig, db::Entry};
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
        use crate::{
            db::{Group, group_add_child, rc_refcell_node},
            variant_dictionary::VariantDictionary,
        };

        let mut db = Database::new(DatabaseConfig::default());

        let mut public_custom_data = VariantDictionary::new();
        public_custom_data.set("example", 42);

        db.config.public_custom_data = Some(public_custom_data);

        group_add_child(&db.root, rc_refcell_node(Entry::default()), 0).unwrap();
        group_add_child(&db.root, rc_refcell_node(Entry::default()), 1).unwrap();
        group_add_child(&db.root, rc_refcell_node(Entry::default()), 2).unwrap();

        let group = rc_refcell_node(Group::new("my group"));
        group_add_child(&group, rc_refcell_node(Entry::default()), 0).unwrap();
        group_add_child(&group, rc_refcell_node(Entry::default()), 1).unwrap();
        group_add_child(&db.root, group, 3).unwrap();

        let mut buffer = Vec::new();
        let key = DatabaseKey::new().with_password("testing");

        db.save(&mut buffer, key.clone())?;

        let db_loaded = Database::open(&mut buffer.as_slice(), key)?;

        assert_eq!(db, db_loaded);
        Ok(())
    }
}
