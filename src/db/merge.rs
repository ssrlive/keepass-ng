use crate::db::*;
use chrono::NaiveDateTime;
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum MergeEventType {
    EntryCreated,
    EntryDeleted,
    EntryLocationUpdated,
    EntryUpdated,

    GroupCreated,
    GroupDeleted,
    GroupLocationUpdated,
    GroupUpdated,
}

#[derive(Debug, Clone)]
pub struct MergeEvent {
    /// The uuid of the node (entry or group) affected by
    /// the merge event.
    pub node_uuid: Uuid,

    pub event_type: MergeEventType,
}

#[derive(Debug, Default, Clone)]
pub struct MergeLog {
    pub warnings: Vec<String>,
    pub events: Vec<MergeEvent>,
}

/// Errors while merge two databases
#[derive(thiserror::Error, Debug)]
pub enum MergeError {
    #[error("{0}")]
    GenericError(String),

    #[error("Could not find group at {0:?}")]
    FindGroupError(Vec<Uuid>),

    #[error("Could not find entry at {0:?}")]
    FindEntryError(Vec<Uuid>),

    #[error("Entries with UUID {0} have the same modification time but have diverged.")]
    EntryModificationTimeNotUpdated(String),

    #[error("Groups with UUID {0} have the same modification time but have diverged.")]
    GroupModificationTimeNotUpdated(String),

    #[error("Found history entries with the same timestamp ({0}) for entry {1}.")]
    DuplicateHistoryEntries(String, String),
}

impl MergeLog {
    pub fn merge_with(&self, other: &MergeLog) -> MergeLog {
        let mut response = MergeLog::default();
        response.warnings.append(self.warnings.clone().as_mut());
        response.warnings.append(other.warnings.clone().as_mut());
        response.events.append(self.events.clone().as_mut());
        response.events.append(other.events.clone().as_mut());
        response
    }

    pub fn append(&mut self, other: &MergeLog) {
        self.warnings.append(other.warnings.clone().as_mut());
        self.events.append(other.events.clone().as_mut());
    }
}

impl Database {
    /// Merge this database with another version of this same database.
    /// This function will use the UUIDs to detect that entries and groups are
    /// the same.
    pub fn merge(&mut self, other: &Database) -> Result<MergeLog, MergeError> {
        let mut log = MergeLog::default();
        log.append(&self.merge_group(&[], &other.root, false)?);
        log.append(&self.merge_deletions(other)?);
        Ok(log)
    }

    fn merge_deletions(&mut self, other: &Database) -> Result<MergeLog, MergeError> {
        // Utility function to search for a UUID in the deletion queue.
        let is_in_deleted_queue = |uuid: Uuid, deleted_groups_queue: &VecDeque<(Uuid, NaiveDateTime)>| -> bool {
            for (deleted_uuid, _) in deleted_groups_queue {
                // This group still has a child group, but it is not going to be deleted.
                if *deleted_uuid == uuid {
                    return true;
                }
            }
            false
        };
        let mut log = MergeLog::default();
        let mut new_deleted_objects = self.deleted_objects.clone();
        // We start by deleting the entries, since we will only remove groups if they are empty.
        for (uuid, deletion_time) in &other.deleted_objects {
            let deletion_time = deletion_time.unwrap_or_else(Times::now);
            if new_deleted_objects.contains_key(uuid) {
                continue;
            }
            let entry_location = match Self::find_node_location(&self.root, *uuid) {
                Some(l) => l,
                None => continue,
            };
            let parent_group = Group::find_group(&self.root, &entry_location).ok_or(MergeError::FindGroupError(entry_location))?;

            let entry = match Group::find_entry(&parent_group, &[*uuid]) {
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
            if entry_last_modification < deletion_time {
                with_node_mut::<Group, _, _>(&parent_group, |pg| pg.remove_node(*uuid)).unwrap()?;
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryDeleted,
                    node_uuid: *uuid,
                });
                new_deleted_objects.insert(*uuid, Some(deletion_time));
            }
        }
        let mut deleted_groups_queue: VecDeque<(Uuid, NaiveDateTime)> = VecDeque::new();
        for (uuid, deletion_time) in &other.deleted_objects {
            if new_deleted_objects.contains_key(uuid) {
                continue;
            }
            deleted_groups_queue.push_back((*uuid, deletion_time.unwrap_or_else(Times::now)));
        }
        while !deleted_groups_queue.is_empty() {
            let (deleted_uuid, deletion_time) = deleted_groups_queue.pop_front().unwrap();
            if new_deleted_objects.contains_key(&deleted_uuid) {
                continue;
            }
            let group_location = match Self::find_node_location(&self.root, deleted_uuid) {
                Some(l) => l,
                None => continue,
            };
            let parent_group = Group::find_group(&self.root, &group_location).ok_or(MergeError::FindGroupError(group_location))?;

            let group = match Group::find_group(&parent_group, &[deleted_uuid]) {
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
            if with_node::<Group, _, _>(&group, |g| {
                g.groups()
                    .iter()
                    .any(|child| is_in_deleted_queue(child.borrow().get_uuid(), &deleted_groups_queue))
            })
            .unwrap()
            {
                deleted_groups_queue.push_back((deleted_uuid, deletion_time));
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
            if group_last_modification < deletion_time {
                with_node_mut::<Group, _, _>(&parent_group, |pg| pg.remove_node(deleted_uuid)).unwrap()?;
                log.events.push(MergeEvent {
                    event_type: MergeEventType::GroupDeleted,
                    node_uuid: deleted_uuid,
                });
                new_deleted_objects.insert(deleted_uuid, Some(deletion_time));
            }
        }
        self.deleted_objects = new_deleted_objects;
        Ok(log)
    }

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
            if self.deleted_objects.contains_key(&other_entry_uuid) {
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
            if self.deleted_objects.contains_key(&other_group_uuid) || is_in_deleted_group {
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

impl Group {
    pub(crate) fn find_group(group: &NodePtr, path: &[Uuid]) -> Option<NodePtr> {
        let path: Vec<String> = path.iter().map(|p| p.to_string()).collect();
        let node_ref = Self::get_by_uuid(group, &path)?;
        if node_is_group(&node_ref) { Some(node_ref) } else { None }
    }

    pub(crate) fn find_entry(group: &NodePtr, path: &[Uuid]) -> Option<NodePtr> {
        let path: Vec<String> = path.iter().map(|p| p.to_string()).collect();
        let node_ref = Self::get_by_uuid(group, &path)?;
        if node_is_entry(&node_ref) { Some(node_ref) } else { None }
    }

    pub(crate) fn remove_node(&mut self, uuid: Uuid) -> Result<NodePtr, MergeError> {
        let mut removed_node = None;
        self.children.retain(|c| {
            if c.borrow().get_uuid() == uuid {
                removed_node = Some(NodePtr::from(c));
                return false;
            }
            true
        });

        let title = self.get_title().unwrap_or("No title").to_string();
        let node = removed_node.ok_or(MergeError::GenericError(format!("Could not find node {uuid} in group \"{title}\"")))?;
        Ok(node)
    }

    pub(crate) fn find_node_location(parent: &NodePtr, id: Uuid) -> Option<Vec<Uuid>> {
        let parent_uuid = parent.borrow().get_uuid();
        let mut current_location = vec![parent_uuid];
        for node in &group_get_children(parent).unwrap_or_default() {
            let node_uuid = node.borrow().get_uuid();
            if node_is_entry(node) {
                if node_uuid == id {
                    // current_location.push(id);
                    return Some(current_location);
                }
            } else if node_is_group(node) {
                if node_uuid == id {
                    // current_location.push(id);
                    return Some(current_location);
                }
                if let Some(mut location) = Self::find_node_location(node, id) {
                    current_location.append(&mut location);
                    return Some(current_location);
                }
            }
        }
        None
    }

    pub(crate) fn merge_with(group: &NodePtr, other: &NodePtr) -> Result<MergeLog, MergeError> {
        let mut log = MergeLog::default();

        let group_uuid = group.borrow().get_uuid();

        let other = other.borrow();
        let other = other
            .downcast_ref::<Group>()
            .ok_or(MergeError::GenericError("Could not downcast node to group".to_string()))?;
        let source_last_modification = match other.times.get_last_modification() {
            Some(t) => t,
            None => {
                log.warnings
                    .push(format!("Group {group_uuid} did not have a last modification timestamp"));
                Times::epoch()
            }
        };
        let destination_last_modification = match group.borrow().get_times().get_last_modification() {
            Some(t) => t,
            None => {
                log.warnings
                    .push(format!("Group {group_uuid} did not have a last modification timestamp"));
                Times::now()
            }
        };
        if destination_last_modification == source_last_modification {
            if group.borrow().downcast_ref::<Group>().unwrap()._has_diverged_from(other) {
                // This should never happen.
                // This means that a group was updated without updating the last modification
                // timestamp.
                return Err(MergeError::GroupModificationTimeNotUpdated(other.uuid.to_string()));
            }
            return Ok(log);
        }
        if destination_last_modification > source_last_modification {
            return Ok(log);
        }
        with_node_mut::<Group, _, _>(group, |group| {
            group.name = other.name.clone();
            group.notes = other.notes.clone();
            group.icon_id = other.icon_id;
            group.custom_icon_uuid = other.custom_icon_uuid;
            group.custom_data = other.custom_data.clone();
            // The location changed timestamp is handled separately when merging two databases.
            let current_times = group.times.clone();
            group.times = other.times.clone();
            if let Some(t) = current_times.get_location_changed() {
                group.times.set_location_changed(Some(t));
            }
            group.is_expanded = other.is_expanded;
            group.default_autotype_sequence = other.default_autotype_sequence.clone();
            group.enable_autotype = other.enable_autotype;
            group.enable_searching = other.enable_searching;
            group.last_top_visible_entry = other.last_top_visible_entry;
        })
        .unwrap();
        log.events.push(MergeEvent {
            event_type: MergeEventType::GroupUpdated,
            node_uuid: group_uuid,
        });
        Ok(log)
    }

    pub(crate) fn _has_diverged_from(&self, other: &Group) -> bool {
        let new_times = Times::new();
        let mut self_purged = self.clone();
        self_purged.times = new_times.clone();
        self_purged.children = vec![];
        let mut other_purged = other.clone();
        other_purged.times = new_times.clone();
        other_purged.children = vec![];
        !self_purged.eq(&other_purged)
    }

    fn replace_entry(root: &NodePtr, entry: &NodePtr) -> bool {
        let uuid = entry.borrow().get_uuid();
        if let Some(target_entry) = search_node_by_uuid_with_specific_type::<Entry>(root, uuid) {
            return with_node_mut::<Entry, _, _>(&target_entry, |e| e.replaced_with(entry)).unwrap_or(false);
        }
        false
    }

    pub(crate) fn has_group(&self, uuid: Uuid) -> bool {
        self.children.iter().any(|n| n.borrow().get_uuid() == uuid && node_is_group(n))
    }

    fn get_or_create_group(group: &NodePtr, location: &[Uuid], create_groups: bool) -> crate::Result<NodePtr> {
        if location.is_empty() {
            return Err("Empty location.".into());
        }

        let mut remaining_location = location.to_owned();
        remaining_location.remove(0);

        if remaining_location.is_empty() {
            return Ok(group.clone());
        }

        let next_location = &remaining_location[0];
        let mut next_location_uuid = *next_location;

        if !with_node::<Group, _, _>(group, |g| g.has_group(next_location_uuid)).unwrap() && create_groups {
            let mut current_group: Option<NodePtr> = None;
            for i in (0..(remaining_location.len())).rev() {
                let mut new_group = Group::new(&remaining_location[i].to_string());
                new_group.set_uuid(remaining_location[i]);
                if let Some(current_group) = current_group {
                    let count = group_get_children(group).map(|c| c.len()).unwrap_or(0);
                    new_group.add_child(current_group, count);
                }
                current_group = Some(rc_refcell_node(new_group));
            }

            if let Some(current_group) = current_group {
                next_location_uuid = current_group.borrow().get_uuid();
                let count = group_get_children(group).map_or(0, |c| c.len());
                group_add_child(group, current_group, count)?;
            } else {
                return Err("Could not create group.".into());
            }
        }

        let mut target = None;
        for node in group_get_children(group).unwrap_or_default().iter() {
            if node_is_group(node) && node.borrow().get_uuid() == next_location_uuid {
                target = Some(node.clone());
                break;
            }
        }

        match &target {
            Some(target) => Self::get_or_create_group(target, &remaining_location, create_groups),
            None => Err("The group was not found.".into()),
        }
    }

    pub(crate) fn insert_entry(group: &NodePtr, entry: NodePtr, location: &[Uuid]) -> crate::Result<()> {
        let group = Self::get_or_create_group(group, location, true)?;
        with_node_mut::<Group, _, _>(&group, |g| {
            let count = g.children.len();
            g.add_child(entry, count);
            Ok::<(), crate::Error>(())
        })
        .ok_or("Could not add entry")??;
        Ok(())
    }

    pub(crate) fn remove_entry(group: &NodePtr, uuid: Uuid, location: &[Uuid]) -> crate::Result<NodePtr> {
        let group = Self::get_or_create_group(group, location, false)?;

        let mut removed_entry: Option<NodePtr> = None;
        let mut new_nodes: Vec<NodePtr> = vec![];
        println!(
            "Searching for entry {} in {}",
            uuid,
            group.borrow().get_title().unwrap_or("No title")
        );

        with_node::<Group, _, _>(&group, |g| {
            for node in g.children.iter() {
                if node_is_entry(node) {
                    let node_uuid = node.borrow().get_uuid();
                    println!("Saw entry {node_uuid}");
                    if node_uuid != uuid {
                        new_nodes.push(NodePtr::from(node));
                        continue;
                    }
                    removed_entry = Some(NodePtr::from(node));
                } else if node_is_group(node) {
                    new_nodes.push(NodePtr::from(node));
                }
            }
        });

        if let Some(entry) = removed_entry {
            with_node_mut::<Group, _, _>(&group, |g| g.reset_children(new_nodes)).ok_or("Could not reset children")?;
            Ok(entry)
        } else {
            let title = group.borrow().get_title().unwrap_or("No title").to_string();
            Err(format!("Could not find entry {uuid} in group \"{title}\".").into())
        }
    }

    pub(crate) fn find_entry_location(&self, uuid: Uuid) -> Option<Vec<Uuid>> {
        let mut current_location = vec![self.uuid];
        for node in &self.children {
            if node_is_entry(node) {
                if node.borrow().get_uuid() == uuid {
                    return Some(current_location);
                }
            } else if let Some(g) = node.borrow().downcast_ref::<Group>()
                && let Some(mut location) = g.find_entry_location(uuid)
            {
                current_location.append(&mut location);
                return Some(current_location);
            }
        }
        None
    }

    pub(crate) fn add_entry(parent: &NodePtr, entry: NodePtr, location: &[Uuid]) -> crate::Result<()> {
        if location.is_empty() {
            panic!("TODO handle this with a Response.");
        }

        let mut remaining_location = location.to_owned();
        remaining_location.remove(0);

        if remaining_location.is_empty() {
            with_node_mut::<Group, _, _>(parent, |g| {
                let count = g.children.len();
                g.add_child(entry, count);
                Ok::<(), crate::Error>(())
            })
            .ok_or("Could not add entry")??;
            return Ok(());
        }

        let next_location = remaining_location[0];

        println!("Searching for group {next_location:?}");
        for node in group_get_children(parent).unwrap_or_default() {
            if node_is_group(&node) {
                if node.borrow().get_uuid() != next_location {
                    continue;
                }
                Self::add_entry(&node, entry, &remaining_location)?;
                return Ok(());
            }
        }

        // The group was not found, so we create it.
        let new_group = rc_refcell_node(Group::new(&next_location.to_string()));
        new_group.borrow_mut().set_uuid(next_location);
        Self::add_entry(&new_group, entry, &remaining_location)?;
        let count = group_get_children(parent).map_or(0, |c| c.len());
        group_add_child(parent, new_group, count)?;
        Ok(())
    }

    /// Merge this group with another group
    #[allow(clippy::too_many_lines)]
    pub fn merge(root: &NodePtr, other_group: &NodePtr) -> crate::Result<MergeLog> {
        let mut log = MergeLog::default();

        let other_entries = with_node::<Group, _, _>(other_group, |g| Ok(g.get_all_entries(&[])))
            .unwrap_or(Err(crate::Error::from("Could not downcast other group to group")))?;

        // Handle entry relocation.
        for (entry, entry_location) in &other_entries {
            let entry_uuid = entry.borrow().get_uuid();
            let the_entry = search_node_by_uuid_with_specific_type::<Entry>(root, entry_uuid);

            let existing_entry = match the_entry {
                Some(e) => e,
                None => continue,
            };

            let the_entry_location = with_node::<Group, _, _>(root, |g| Ok(g.find_entry_location(entry_uuid)))
                .unwrap_or(Err("Could not downcast root to group"))?;

            let existing_entry_location = match the_entry_location {
                Some(l) => l,
                None => continue,
            };

            let source_location_changed_time = if let Some(t) = entry.borrow().get_times().get_location_changed() {
                t
            } else {
                log.warnings
                    .push(format!("Entry {entry_uuid} did not have a location updated timestamp"));
                Times::epoch()
            };
            let destination_location_changed = if let Some(t) = existing_entry.borrow().get_times().get_location_changed() {
                t
            } else {
                log.warnings
                    .push(format!("Entry {entry_uuid} did not have a location updated timestamp"));
                Times::now()
            };
            if source_location_changed_time > destination_location_changed {
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryLocationUpdated,
                    node_uuid: entry_uuid,
                });
                Self::remove_entry(root, entry_uuid, &existing_entry_location)?;
                Self::insert_entry(root, entry.borrow().duplicate(), entry_location)?;
            }
        }

        // Handle entry updates
        for (entry, entry_location) in &other_entries {
            let entry_uuid = entry.borrow().get_uuid();
            let the_entry = search_node_by_uuid_with_specific_type::<Entry>(root, entry_uuid);
            if let Some(existing_entry) = the_entry {
                if node_is_equals_to(&existing_entry, entry) {
                    continue;
                }

                let source_last_modification = if let Some(t) = entry.borrow().get_times().get_last_modification() {
                    t
                } else {
                    log.warnings
                        .push(format!("Entry {entry_uuid} did not have a last modification timestamp"));
                    Times::epoch()
                };
                let destination_last_modification = if let Some(t) = existing_entry.borrow().get_times().get_last_modification() {
                    t
                } else {
                    log.warnings
                        .push(format!("Entry {entry_uuid} did not have a last modification timestamp"));
                    Times::now()
                };

                if destination_last_modification == source_last_modification {
                    if !node_is_equals_to(&existing_entry, entry) {
                        // This should never happen.
                        // This means that an entry was updated without updating the last modification
                        // timestamp.
                        return Err("Entries have the same modification time but are not the same!".into());
                    }
                    continue;
                }

                let (merged_entry, entry_merge_log) = if destination_last_modification > source_last_modification {
                    Entry::merge(&existing_entry, entry)?
                } else {
                    Entry::merge(entry, &existing_entry)?
                };
                let Some(merged_entry) = merged_entry else {
                    continue;
                };
                // merged_entry.borrow_mut().set_parent(existing_entry.borrow().get_parent());
                if node_is_equals_to(&existing_entry, &merged_entry) {
                    continue;
                }

                Group::replace_entry(root, &merged_entry);

                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryUpdated,
                    node_uuid: merged_entry.borrow().get_uuid(),
                });
                log = log.merge_with(&entry_merge_log);
            } else {
                Self::add_entry(root, entry.borrow().duplicate(), entry_location)?;
                // TODO should we update the time info for the entry?
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryCreated,
                    node_uuid: entry.borrow().get_uuid(),
                });
            }
        }

        // TODO handle deleted objects
        Ok(log)
    }

    // Recursively get all the entries in the group, along with their
    // location.
    pub(crate) fn get_all_entries(&self, current_location: &[Uuid]) -> Vec<(NodePtr, Vec<Uuid>)> {
        let mut response: Vec<(NodePtr, Vec<Uuid>)> = vec![];
        let mut new_location = current_location.to_owned();
        new_location.push(self.uuid);

        for node in &self.children {
            if node_is_entry(node) {
                response.push((node.into(), new_location.clone()));
            }
            with_node::<Group, _, _>(node, |g| {
                let mut new_entries = g.get_all_entries(&new_location);
                response.append(&mut new_entries);
            });
        }
        response
    }
}

#[cfg(test)]
pub fn entry_set_field_and_commit(entry: &NodePtr, field_name: &str, field_value: &str) -> crate::Result<()> {
    with_node_mut::<Entry, _, _>(entry, |entry| {
        entry.set_field_and_commit(field_name, field_value);
        Ok(())
    })
    .unwrap_or(Err("node is not an Entry.".to_string()))?;
    Ok(())
}

impl Entry {
    pub(crate) fn merge(entry: &NodePtr, other: &NodePtr) -> Result<(Option<NodePtr>, MergeLog), MergeError> {
        let mut log = MergeLog::default();
        let source_last_modification = match with_node::<Entry, _, _>(other, |e| e.get_times().get_last_modification()).unwrap() {
            Some(t) => t,
            None => {
                let info = format!("Entry {} did not have a last modification timestamp", other.borrow().get_uuid());
                log.warnings.push(info);
                Times::epoch()
            }
        };

        let destination_last_modification = match with_node::<Entry, _, _>(entry, |e| e.get_times().get_last_modification()).unwrap() {
            Some(t) => t,
            None => {
                let info = format!("Entry {} did not have a last modification timestamp", entry.borrow().get_uuid());
                log.warnings.push(info);
                Times::epoch()
            }
        };

        if destination_last_modification == source_last_modification {
            if !crate::db::merge::has_diverged_from(entry, other) {
                // This should never happen. This means that an entry was updated without updating the last modification timestamp.
                return Err(MergeError::EntryModificationTimeNotUpdated(other.borrow().get_uuid().to_string()));
            }
            return Ok((None, log));
        }
        let (mut merged_entry, entry_merge_log) = with_node::<Entry, _, _>(entry, |entry| {
            with_node::<Entry, _, _>(other, |other| {
                if destination_last_modification > source_last_modification {
                    entry.merge_history(other)
                } else {
                    other.merge_history(entry)
                }
            })
            .unwrap()
        })
        .unwrap()?;

        if let location_changed_timestamp @ Some(_) = entry.borrow().get_times().get_location_changed() {
            merged_entry.get_times_mut().set_location_changed(location_changed_timestamp);
        }
        Ok((Some(rc_refcell_node(merged_entry)), entry_merge_log))
    }

    pub(crate) fn merge_history(&self, other: &Entry) -> Result<(Entry, MergeLog), MergeError> {
        let mut log = MergeLog::default();
        let mut source_history = match &other.history {
            Some(h) => h.clone(),
            None => {
                log.warnings
                    .push(format!("Entry {} from source database had no history.", self.uuid));
                History::default()
            }
        };
        let mut destination_history = match &self.history {
            Some(h) => h.clone(),
            None => {
                log.warnings
                    .push(format!("Entry {} from destination database had no history.", self.uuid));
                History::default()
            }
        };
        let mut response = self.clone();
        if other.has_uncommited_changes() {
            log.warnings
                .push(format!("Entry {} from source database has uncommitted changes.", self.uuid));
            source_history.add_entry(other.clone());
        }
        // TODO we should probably check for uncommitted changes in the destination
        // database here too for consistency.
        let history_merge_log = destination_history.merge_with(&source_history)?;
        response.history = Some(destination_history);
        Ok((response, log.merge_with(&history_merge_log)))
    }

    // Convenience function used in when merging two entries
    pub(crate) fn _has_diverged_from(&self, other_entry: &Entry) -> bool {
        let new_times = Times::default();

        let mut self_without_times = self.clone();
        self_without_times.times = new_times.clone();

        let mut other_without_times = other_entry.clone();
        other_without_times.times = new_times;
        !self_without_times.eq(&other_without_times)
    }

    /*
    pub(crate) fn merge_(entry: &NodePtr, other: &NodePtr) -> Result<(NodePtr, MergeLog), String> {
        let mut log = MergeLog::default();

        let mut source_history = match &other.borrow().as_any().downcast_ref::<Entry>().ok_or("Error")?.history {
            Some(h) => h.clone(),
            None => {
                log.warnings.push(format!("Entry {} had no history.", entry.borrow().get_uuid()));
                History::default()
            }
        };
        let _destination_history = with_node::<Entry, _, _>(entry, |entry| entry.history.clone()).unwrap_or_default();
        let mut destination_history = _destination_history.unwrap_or_else(|| {
            log.warnings.push(format!("Entry {} had no history.", entry.borrow().get_uuid()));
            History::default()
        });

        let other = other.borrow().duplicate();
        source_history.add_entry(other.borrow().as_any().downcast_ref::<Entry>().ok_or("Error")?.clone());
        let history_merge_log = destination_history.merge_with(&source_history)?;
        let response = entry.borrow().duplicate();
        with_node_mut::<Entry, _, _>(&response, |entry| {
            entry.history = Some(destination_history);
        });

        Ok((response, log.merge_with(&history_merge_log)))
    }
    */

    // Convenience function used in unit tests, to make sure that:
    // 1. The history gets updated after changing a field
    // 2. We wait a second before commiting the changes so that the timestamp is not the same
    //    as it previously was. This is necessary since the timestamps in the KDBX format
    //    do not preserve the msecs.
    #[cfg(test)]
    pub(crate) fn set_field_and_commit(&mut self, field_name: &str, field_value: &str) {
        self.set_unprotected_field_pair(field_name, Some(field_value));
        std::thread::sleep(std::time::Duration::from_secs(1));
        self.update_history();
    }

    pub(crate) fn replaced_with(&mut self, other: &NodePtr) -> bool {
        let mut success = false;
        with_node::<Entry, _, _>(other, |other| {
            self.uuid = other.uuid;
            self.fields = other.fields.clone();
            self.autotype = other.autotype.clone();
            self.tags = other.tags.clone();
            self.times = other.times.clone();
            self.custom_data = other.custom_data.clone();
            self.icon_id = other.icon_id;
            self.custom_icon = other.custom_icon.clone();
            self.foreground_color = other.foreground_color;
            self.background_color = other.background_color;
            self.override_url = other.override_url.clone();
            self.quality_check = other.quality_check;
            self.history = other.history.clone();
            // self.parent = other.parent;
            success = true;
        });
        success
    }
}

impl History {
    // Determines if the entries of the history are
    // ordered by last modification time.
    #[cfg(test)]
    pub(crate) fn is_ordered(&self) -> bool {
        let mut last_modification_time: Option<chrono::NaiveDateTime> = None;
        for entry in &self.entries {
            if last_modification_time.is_none() {
                last_modification_time = entry.times.get_last_modification();
            }

            let entry_modification_time = entry.times.get_last_modification().unwrap();
            // FIXME should we also handle equal modification times??
            if last_modification_time.unwrap() < entry_modification_time {
                return false;
            }
            last_modification_time = Some(entry_modification_time);
        }
        true
    }

    // Merge both histories together.
    pub(crate) fn merge_with(&mut self, other: &History) -> Result<MergeLog, MergeError> {
        let mut log = MergeLog::default();
        let mut new_history_entries: HashMap<chrono::NaiveDateTime, Entry> = HashMap::new();

        for history_entry in &self.entries {
            let modification_time = history_entry.times.get_last_modification().unwrap();
            if new_history_entries.contains_key(&modification_time) {
                return Err(MergeError::DuplicateHistoryEntries(
                    modification_time.to_string(),
                    history_entry.uuid.to_string(),
                ));
            }
            new_history_entries.insert(modification_time, history_entry.clone());
        }

        for history_entry in &other.entries {
            let modification_time = history_entry.times.get_last_modification().unwrap();
            let existing_history_entry = new_history_entries.get(&modification_time);
            if let Some(existing_history_entry) = existing_history_entry {
                if existing_history_entry._has_diverged_from(history_entry) {
                    log.warnings.push(format!(
                        "History entries for {} have the same modification timestamp but were not the same.",
                        existing_history_entry.uuid
                    ));
                }
            } else {
                new_history_entries.insert(modification_time, history_entry.clone());
            }
        }

        let mut all_modification_times: Vec<&chrono::NaiveDateTime> = new_history_entries.keys().collect();
        all_modification_times.sort();
        all_modification_times.reverse();
        let mut new_entries: Vec<Entry> = vec![];
        for modification_time in &all_modification_times {
            new_entries.push(new_history_entries.get(modification_time).unwrap().clone());
        }
        self.entries = new_entries;
        Ok(log)
    }
}

#[cfg(test)]
mod merge_tests {
    use std::{thread, time};
    use uuid::Uuid;

    use crate::db::{
        Database, Entry, Group, Node, NodePtr, Times, group_add_child, group_get_children, node_is_group, rc_refcell_node, with_node,
        with_node_mut,
    };

    fn get_entry(db: &Database, path: &[&str]) -> NodePtr {
        Group::get(&db.root, path).unwrap()
    }

    fn get_group(db: &Database, path: &[&str]) -> NodePtr {
        Group::get(&db.root, path).unwrap()
    }

    fn get_all_groups(group: &NodePtr) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in group_get_children(group).unwrap() {
            if node_is_group(&node) {
                let mut new_groups = get_all_groups(&node);
                response.append(&mut new_groups);
                response.push(node);
            }
        }

        response
    }

    fn get_all_entries(group: &NodePtr) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in group_get_children(group).unwrap() {
            if node_is_group(&node) {
                let mut new_entries = get_all_entries(&node);
                response.append(&mut new_entries);
            } else {
                response.push(node);
            }
        }
        response
    }

    const ROOT_GROUP_ID: &str = "00000000-0000-0000-0000-000000000001";
    const GROUP1_ID: &str = "00000000-0000-0000-0000-000000000002";
    const GROUP2_ID: &str = "00000000-0000-0000-0000-000000000003";
    const SUBGROUP1_ID: &str = "00000000-0000-0000-0000-000000000004";
    const SUBGROUP2_ID: &str = "00000000-0000-0000-0000-000000000005";

    const ENTRY1_ID: &str = "00000000-0000-0000-0000-000000000006";
    const ENTRY2_ID: &str = "00000000-0000-0000-0000-000000000007";

    fn create_test_database() -> Database {
        let mut db = Database::new(Default::default());
        let mut root_group = Group::new("root");
        root_group.uuid = Uuid::parse_str(ROOT_GROUP_ID).unwrap();

        let mut group1 = Group::new("group1");
        group1.uuid = Uuid::parse_str(GROUP1_ID).unwrap();
        let mut group2 = Group::new("group2");
        group2.uuid = Uuid::parse_str(GROUP2_ID).unwrap();

        let mut subgroup1 = Group::new("subgroup1");
        subgroup1.uuid = Uuid::parse_str(SUBGROUP1_ID).unwrap();
        let mut subgroup2 = Group::new("subgroup2");
        subgroup2.uuid = Uuid::parse_str(SUBGROUP2_ID).unwrap();

        // Placing the first entry in the root group
        let mut entry1 = Entry::default();
        entry1.set_uuid(Uuid::parse_str(ENTRY1_ID).unwrap());
        entry1.set_field_and_commit("Title", "entry1");
        root_group.add_child(rc_refcell_node(entry1), 0);

        // Placing the second entry in a subgroup
        let mut entry2 = Entry::default();
        entry2.set_uuid(Uuid::parse_str(ENTRY2_ID).unwrap());
        entry2.set_field_and_commit("Title", "entry2");
        subgroup1.add_child(rc_refcell_node(entry2), 0);

        group1.add_child(rc_refcell_node(subgroup1), 0);
        group2.add_child(rc_refcell_node(subgroup2), 0);

        root_group.add_child(rc_refcell_node(group1), 1);
        root_group.add_child(rc_refcell_node(group2), 2);

        db.root = rc_refcell_node(root_group).into();
        db
    }

    #[test]
    fn test_idempotence() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
        assert_eq!(group_get_children(&destination_db.root).unwrap().len(), 3);

        // The 2 groups should be exactly the same after merging, since
        // nothing was performed during the merge.
        assert_eq!(destination_db, source_db);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let entry = get_all_entries(&destination_db.root)[0].clone();
        with_node_mut::<Entry, _, _>(&entry, |entry| {
            entry.set_field_and_commit("Title", "entry1_updated");
        });

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
        let destination_db_just_after_merge = destination_db.clone();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
        // Merging twice in a row, even if the first merge updated the destination group,
        // should not create more changes.
        assert_eq!(destination_db_just_after_merge, destination_db);
    }

    #[test]
    fn test_add_new_entry() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let mut new_entry = Entry::default();
        new_entry.set_field_and_commit("Title", "new_entry");
        group_add_child(&source_db.root, rc_refcell_node(new_entry), 0).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before + 1);
        assert_eq!(group_count_after, group_count_before);

        let root_entries = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap();
        assert_eq!(root_entries.len(), 2);

        let new_entry = get_entry(&destination_db, &["new_entry"]);
        assert_eq!(new_entry.borrow().get_title().unwrap(), "new_entry".to_string());

        // Merging the same group again should not create a duplicate entry.
        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before + 1);
        assert_eq!(group_count_after, group_count_before);
    }

    #[test]
    fn test_deleted_entry_in_destination() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let mut deleted_entry = Entry::default();
        let deleted_entry_uuid = deleted_entry.uuid;
        deleted_entry.set_field_and_commit("Title", "deleted_entry");
        group_add_child(&source_db.root, rc_refcell_node(deleted_entry), 0).unwrap();

        destination_db.deleted_objects.insert(deleted_entry_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let new_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(new_entry.is_none());
    }

    #[test]
    fn test_updated_entry_under_deleted_group() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let mut modified_entry = Entry::default();
        modified_entry.set_field_and_commit("Title", "original_title");
        group_add_child(&destination_db.root, modified_entry.duplicate(), 0).unwrap();

        let mut deleted_group = Group::new("deleted_group");
        let deleted_group_uuid = deleted_group.uuid;
        let modified_entry_uuid = modified_entry.uuid;
        modified_entry.set_field_and_commit("Title", "modified_title");
        deleted_group.add_child(rc_refcell_node(modified_entry), 0);
        group_add_child(&source_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        destination_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_none());

        let modified_entry_location = Group::find_node_location(&destination_db.root, modified_entry_uuid);
        assert!(modified_entry_location.is_some());

        let modified_entry = Group::find_entry(&destination_db.root, &[modified_entry_uuid]).unwrap();
        assert_eq!(modified_entry.borrow().get_title(), Some("modified_title"));
    }

    #[test]
    fn test_deleted_group_in_destination() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let deleted_group = Group::new("deleted_group");
        let deleted_group_uuid = deleted_group.uuid;
        group_add_child(&source_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        destination_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_none());
    }

    #[test]
    fn test_deleted_entry_in_source() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let mut deleted_entry = Entry::default();
        let deleted_entry_uuid = deleted_entry.uuid;
        deleted_entry.set_field_and_commit("Title", "deleted_entry");
        group_add_child(&destination_db.root, rc_refcell_node(deleted_entry), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_entry_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before - 1);
        assert_eq!(group_count_after, group_count_before);

        let new_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(new_entry.is_none());

        assert!(destination_db.deleted_objects.contains_key(&deleted_entry_uuid));
    }

    #[test]
    fn test_deleted_group_in_source() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_group = Group::new("deleted_group");
        let deleted_group_uuid = deleted_group.uuid;
        group_add_child(&destination_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before - 1);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_none());

        assert!(destination_db.deleted_objects.contains_key(&deleted_group_uuid));
    }

    #[test]
    fn test_deleted_entry_in_source_modified_in_destination() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_entry_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_entry_uuid, Some(Times::now()));

        thread::sleep(time::Duration::from_secs(1));
        let mut deleted_entry = Entry::default();
        deleted_entry.set_uuid(deleted_entry_uuid);
        deleted_entry.set_field_and_commit("Title", "deleted_entry");
        group_add_child(&destination_db.root, rc_refcell_node(deleted_entry), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let new_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(new_entry.is_some());

        assert!(!destination_db.deleted_objects.contains_key(&deleted_entry_uuid));
    }

    #[test]
    fn test_group_subtree_deletion() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_entry_uuid = Uuid::new_v4();
        let deleted_group_uuid = Uuid::new_v4();
        let deleted_subgroup_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        let mut deleted_entry = Entry::default();
        deleted_entry.set_uuid(deleted_entry_uuid);
        deleted_entry.set_field_and_commit("Title", "deleted_entry");

        let mut deleted_subgroup = Group::new("deleted_subgroup");
        deleted_subgroup.uuid = deleted_subgroup_uuid;
        deleted_subgroup.add_child(rc_refcell_node(deleted_entry), 0);

        let mut deleted_group = Group::new("deleted_group");
        deleted_group.uuid = deleted_group_uuid;
        deleted_group.add_child(rc_refcell_node(deleted_subgroup), 0);

        group_add_child(&destination_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_entry_uuid, Some(Times::now()));
        source_db.deleted_objects.insert(deleted_subgroup_uuid, Some(Times::now()));
        source_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 3);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before - 1);
        assert_eq!(group_count_after, group_count_before - 2);

        let deleted_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(deleted_entry.is_none());
        let deleted_subgroup = Group::find_node_location(&destination_db.root, deleted_subgroup_uuid);
        assert!(deleted_subgroup.is_none());
        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_none());

        assert!(destination_db.deleted_objects.contains_key(&deleted_entry_uuid));
        assert!(destination_db.deleted_objects.contains_key(&deleted_subgroup_uuid));
        assert!(destination_db.deleted_objects.contains_key(&deleted_group_uuid));
    }

    #[test]
    fn test_group_subtree_partial_deletion() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_entry_uuid = Uuid::new_v4();
        let deleted_group_uuid = Uuid::new_v4();
        let deleted_subgroup_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        let mut deleted_entry = Entry::default();
        deleted_entry.set_uuid(deleted_entry_uuid);
        deleted_entry.set_field_and_commit("Title", "deleted_entry");

        let mut deleted_subgroup = Group::new("deleted_subgroup");
        deleted_subgroup.uuid = deleted_subgroup_uuid;
        deleted_subgroup.add_child(rc_refcell_node(deleted_entry), 0);

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_entry_uuid, Some(Times::now()));
        source_db.deleted_objects.insert(deleted_subgroup_uuid, Some(Times::now()));
        source_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        thread::sleep(time::Duration::from_secs(1));
        let mut deleted_group = Group::new("deleted_group");
        deleted_group.uuid = deleted_group_uuid;
        deleted_group.add_child(rc_refcell_node(deleted_subgroup), 0);

        group_add_child(&destination_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 2);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before - 1);
        assert_eq!(group_count_after, group_count_before - 1);

        let deleted_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(deleted_entry.is_none());
        let deleted_subgroup = Group::find_node_location(&destination_db.root, deleted_subgroup_uuid);
        assert!(deleted_subgroup.is_none());
        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_some());

        assert!(destination_db.deleted_objects.contains_key(&deleted_entry_uuid));
        assert!(destination_db.deleted_objects.contains_key(&deleted_subgroup_uuid));
        assert!(!destination_db.deleted_objects.contains_key(&deleted_group_uuid));
    }

    #[test]
    fn test_deleted_group_in_source_modified_in_destination() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_group_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        thread::sleep(time::Duration::from_secs(1));
        let mut deleted_group = Group::new("deleted_group");
        deleted_group.uuid = deleted_group_uuid;
        group_add_child(&destination_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_some());

        assert!(!destination_db.deleted_objects.contains_key(&deleted_group_uuid));
    }

    #[test]
    fn test_deleted_group_has_new_entries() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let mut deleted_group = Group::new("deleted_group");
        let deleted_group_uuid = deleted_group.uuid;

        let mut new_entry = Entry::default();
        let new_entry_uuid = new_entry.uuid;
        new_entry.set_field_and_commit("Title", "new_entry");
        deleted_group.add_child(rc_refcell_node(new_entry), 0);
        group_add_child(&destination_db.root, rc_refcell_node(deleted_group), 0).unwrap();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.insert(deleted_group_uuid, Some(Times::now()));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_some());
        let new_entry = Group::find_node_location(&destination_db.root, new_entry_uuid);
        assert!(new_entry.is_some());

        assert!(!destination_db.deleted_objects.contains_key(&deleted_group_uuid));
        assert!(!destination_db.deleted_objects.contains_key(&new_entry_uuid));
    }

    #[test]
    fn test_add_new_non_root_entry() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let source_sub_group = with_node::<Group, _, _>(&source_db.root, |group| group.groups()).unwrap()[0].clone();

        let mut new_entry = Entry::default();
        let new_entry_uuid = new_entry.uuid;
        new_entry.set_field_and_commit("Title", "new_entry");
        // source_sub_group.add_child(new_entry);
        group_add_child(&source_sub_group, rc_refcell_node(new_entry), 0).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before + 1);
        assert_eq!(group_count_after, group_count_before);

        let created_entry_location = Group::find_node_location(&destination_db.root, new_entry_uuid).unwrap();
        assert_eq!(created_entry_location.len(), 2);
    }

    #[test]
    fn test_add_new_entry_new_group() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let group_count_before = get_all_groups(&destination_db.root).len();
        let entry_count_before = get_all_entries(&destination_db.root).len();

        let mut source_group = Group::new("new_group");
        let mut source_sub_group = Group::new("new_subgroup");

        let mut new_entry = Entry::default();
        let new_entry_uuid = new_entry.uuid;
        new_entry.set_field_and_commit("Title", "new_entry");
        source_sub_group.add_child(rc_refcell_node(new_entry), 0);
        source_group.add_child(rc_refcell_node(source_sub_group), 0);
        group_add_child(&source_db.root, rc_refcell_node(source_group), 0).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 3);

        let group_count_after = get_all_groups(&destination_db.root).len();
        let entry_count_after = get_all_entries(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before + 1);
        assert_eq!(group_count_after, group_count_before + 2);

        let created_entry_location = Group::find_node_location(&destination_db.root, new_entry_uuid).unwrap();
        assert_eq!(created_entry_location.len(), 3);
    }

    #[test]
    fn test_entry_relocation_existing_group() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let group_count_before = get_all_groups(&destination_db.root).len();
        let entry_count_before = get_all_entries(&destination_db.root).len();

        thread::sleep(time::Duration::from_secs(1));
        let new_location_changed_timestamp = Times::now();

        source_db
            .relocate_node(
                Uuid::parse_str(ENTRY2_ID).unwrap(),
                &[Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &[Uuid::parse_str(GROUP2_ID).unwrap()],
                new_location_changed_timestamp,
            )
            .unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let group_count_after = get_all_groups(&destination_db.root).len();
        let entry_count_after = get_all_entries(&destination_db.root).len();
        assert_eq!(group_count_after, group_count_before);
        assert_eq!(entry_count_after, entry_count_before);

        let moved_entry_location = Group::find_node_location(&destination_db.root, Uuid::parse_str(ENTRY2_ID).unwrap()).unwrap();
        assert_eq!(moved_entry_location.len(), 2);
        assert_eq!(&moved_entry_location[0].to_string(), ROOT_GROUP_ID);
        assert_eq!(&moved_entry_location[1].to_string(), GROUP2_ID);

        let moved_entry = get_entry(&destination_db, &["group2", "entry2"]);
        let ts = moved_entry.borrow().get_times().get_location_changed().unwrap();
        assert_eq!(ts, new_location_changed_timestamp);
    }

    #[test]
    fn test_entry_relocation_and_update() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let group_count_before = get_all_groups(&destination_db.root).len();
        let entry_count_before = get_all_entries(&destination_db.root).len();

        let entry2 = Group::find_entry(
            &source_db.root,
            &[
                Uuid::parse_str(GROUP1_ID).unwrap(),
                Uuid::parse_str(SUBGROUP1_ID).unwrap(),
                Uuid::parse_str(ENTRY2_ID).unwrap(),
            ],
        )
        .unwrap();

        with_node_mut::<Entry, _, _>(&entry2, |entry2| {
            entry2.set_field_and_commit("Title", "entry2_modified_in_source");
        });

        thread::sleep(time::Duration::from_secs(1));
        let new_location_changed_timestamp = Times::now();

        source_db
            .relocate_node(
                Uuid::parse_str(ENTRY2_ID).unwrap(),
                &[Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &[Uuid::parse_str(GROUP2_ID).unwrap()],
                new_location_changed_timestamp,
            )
            .unwrap();

        let entry2 = Group::find_entry(
            &destination_db.root,
            &[
                Uuid::parse_str(GROUP1_ID).unwrap(),
                Uuid::parse_str(SUBGROUP1_ID).unwrap(),
                Uuid::parse_str(ENTRY2_ID).unwrap(),
            ],
        )
        .unwrap();
        with_node_mut::<Entry, _, _>(&entry2, |entry2| {
            entry2.set_field_and_commit("Title", "entry2_modified_in_destination");
        });
        let entry_modified_timestamp = entry2.borrow().get_times().get_last_modification().unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 2);

        let group_count_after = get_all_groups(&destination_db.root).len();
        let entry_count_after = get_all_entries(&destination_db.root).len();
        assert_eq!(group_count_after, group_count_before);
        assert_eq!(entry_count_after, entry_count_before);

        let moved_entry_location = Group::find_node_location(&destination_db.root, Uuid::parse_str(ENTRY2_ID).unwrap()).unwrap();
        assert_eq!(moved_entry_location.len(), 2);
        assert_eq!(&moved_entry_location[0].to_string(), ROOT_GROUP_ID);
        assert_eq!(&moved_entry_location[1].to_string(), GROUP2_ID);

        let moved_entry = get_entry(&destination_db, &["group2", "entry2_modified_in_destination"]);
        let ts1 = moved_entry.borrow().get_times().get_last_modification().unwrap();
        assert_eq!(ts1, entry_modified_timestamp,);
        let ts2 = moved_entry.borrow().get_times().get_location_changed().unwrap();
        assert_eq!(ts2, new_location_changed_timestamp);
    }

    #[test]
    fn test_entry_relocation_in_destination_and_update() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let group_count_before = get_all_groups(&destination_db.root).len();
        let entry_count_before = get_all_entries(&destination_db.root).len();

        let entry2 = Group::find_entry(
            &source_db.root,
            &[
                Uuid::parse_str(GROUP1_ID).unwrap(),
                Uuid::parse_str(SUBGROUP1_ID).unwrap(),
                Uuid::parse_str(ENTRY2_ID).unwrap(),
            ],
        )
        .unwrap();
        with_node_mut::<Entry, _, _>(&entry2, |entry2| {
            entry2.set_field_and_commit("Title", "entry2_modified_in_source");
        });
        let entry_modified_timestamp = entry2.borrow().get_times().get_last_modification().unwrap();

        thread::sleep(time::Duration::from_secs(1));
        let new_location_changed_timestamp = Times::now();

        destination_db
            .relocate_node(
                Uuid::parse_str(ENTRY2_ID).unwrap(),
                &[Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &[Uuid::parse_str(GROUP2_ID).unwrap()],
                new_location_changed_timestamp,
            )
            .unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let group_count_after = get_all_groups(&destination_db.root).len();
        let entry_count_after = get_all_entries(&destination_db.root).len();
        assert_eq!(group_count_after, group_count_before);
        assert_eq!(entry_count_after, entry_count_before);

        let moved_entry_location = Group::find_node_location(&destination_db.root, Uuid::parse_str(ENTRY2_ID).unwrap()).unwrap();
        assert_eq!(moved_entry_location.len(), 2);
        assert_eq!(&moved_entry_location[0].to_string(), ROOT_GROUP_ID);
        assert_eq!(&moved_entry_location[1].to_string(), GROUP2_ID);

        let moved_entry = get_entry(&destination_db, &["group2", "entry2_modified_in_source"]);
        let ts1 = moved_entry.borrow().get_times().get_last_modification().unwrap();
        assert_eq!(ts1, entry_modified_timestamp,);
        let ts2 = moved_entry.borrow().get_times().get_location_changed().unwrap();
        assert_eq!(ts2, new_location_changed_timestamp);
    }

    #[test]
    fn test_entry_relocation_new_group() {
        let mut destination_db = create_test_database();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let source_db = destination_db.clone();
        let mut new_group = Group::new("new_group");
        let new_group_uuid = new_group.uuid;

        let mut new_entry = Entry::default();
        let entry_uuid = new_entry.uuid;
        new_entry.set_field_and_commit("Title", "entry1");

        thread::sleep(time::Duration::from_secs(1));
        new_entry.times.set_location_changed(Some(Times::now()));
        // FIXME we should not have to update the history here. We should
        // have a better compare function in the merge function instead.
        new_entry.update_history();
        new_group.add_child(rc_refcell_node(new_entry), 0);
        group_add_child(&source_db.root, rc_refcell_node(new_group), 0).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 2);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before + 1);
        assert_eq!(group_count_after, group_count_before + 1);

        let created_entry_location = Group::find_node_location(&destination_db.root, entry_uuid).unwrap();
        assert_eq!(created_entry_location.len(), 2);
        assert_eq!(&created_entry_location[0].to_string(), ROOT_GROUP_ID);
        assert_eq!(created_entry_location[1], new_group_uuid);
    }

    #[test]
    fn test_group_relocation() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let source_group_1 = get_group(&source_db, &["group1"]);
        let source_sub_group_1 = with_node_mut::<Group, _, _>(&source_group_1, |g| g.remove_node(Uuid::parse_str(SUBGROUP1_ID).unwrap()))
            .unwrap()
            .unwrap();
        assert!(node_is_group(&source_sub_group_1));
        thread::sleep(time::Duration::from_secs(1));
        let new_location_changed_timestamp = Times::now();
        source_sub_group_1
            .borrow_mut()
            .get_times_mut()
            .set_location_changed(Some(new_location_changed_timestamp));

        let source_group_2 = get_group(&source_db, &["group2"]);
        group_add_child(&source_group_2, source_sub_group_1, 0).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let created_entry_location = Group::find_node_location(&destination_db.root, Uuid::parse_str(ENTRY2_ID).unwrap()).unwrap();
        assert_eq!(created_entry_location.len(), 3);
        assert_eq!(created_entry_location[0], destination_db.root.borrow().get_uuid());
        assert_eq!(&created_entry_location[1].to_string(), GROUP2_ID);
        assert_eq!(&created_entry_location[2].to_string(), SUBGROUP1_ID);

        let relocated_group = get_group(&destination_db, &["group2", "subgroup1"]);
        let ts = relocated_group.borrow().get_times().get_location_changed().unwrap();
        assert_eq!(ts, new_location_changed_timestamp);
    }

    #[test]
    fn test_update_in_destination_no_conflict() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        with_node_mut::<Entry, _, _>(&entry, |entry| entry.set_field_and_commit("Title", "entry1_updated")).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        let merged_history = with_node::<Entry, _, _>(&entry, |entry| entry.history.clone()).unwrap().unwrap();
        assert!(merged_history.is_ordered());
        assert_eq!(merged_history.entries.len(), 2);
        let merged_entry = &merged_history.entries[1];
        assert_eq!(merged_entry.get_title(), Some("entry1"));

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_in_source_no_conflict() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let entry = with_node::<Group, _, _>(&source_db.root, |group| group.entries()).unwrap()[0].clone();
        with_node_mut::<Entry, _, _>(&entry, |entry| entry.set_field_and_commit("Title", "entry1_updated")).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        let merged_history = with_node::<Entry, _, _>(&entry, |entry| entry.history.clone()).unwrap().unwrap();
        assert!(merged_history.is_ordered());
        assert_eq!(merged_history.entries.len(), 2);
        let merged_entry = &merged_history.entries[1];
        assert_eq!(merged_entry.get_title(), Some("entry1"));

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_with_conflicts() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        with_node_mut::<Entry, _, _>(&entry, |e| e.set_field_and_commit("Title", "entry1_updated_from_destination")).unwrap();

        let entry = with_node::<Group, _, _>(&source_db.root, |group| group.entries()).unwrap()[0].clone();
        with_node_mut::<Entry, _, _>(&entry, |entry| entry.set_field_and_commit("Title", "entry1_updated_from_source")).unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let entry = with_node::<Group, _, _>(&destination_db.root, |group| group.entries()).unwrap()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated_from_source"));

        let merged_history = with_node::<Entry, _, _>(&entry, |entry| entry.history.clone()).unwrap().unwrap();
        assert!(merged_history.is_ordered());
        assert_eq!(merged_history.entries.len(), 3);
        let merged_entry = &merged_history.entries[1];
        assert_eq!(merged_entry.get_title(), Some("entry1_updated_from_destination"));

        // Merging again should not result in any additional change.
        let merge_result = destination_db.merge(&destination_db.clone()).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
    }

    #[test]
    fn test_group_update_in_source() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let group = get_group(&source_db, &["group1", "subgroup1"]);
        group.borrow_mut().set_title(Some("subgroup1_updated_name"));
        // Making sure to wait 1 sec before update the timestamp, to make
        // sure that we get a different modification timestamp.
        thread::sleep(time::Duration::from_secs(1));
        let new_modification_timestamp = Times::now();
        group
            .borrow_mut()
            .get_times_mut()
            .set_last_modification(Some(new_modification_timestamp));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let modified_group = get_group(&destination_db, &["group1", "subgroup1_updated_name"]);
        assert_eq!(modified_group.borrow().get_title().unwrap(), "subgroup1_updated_name");
        let ts = modified_group.borrow().get_times().get_last_modification();
        assert_eq!(ts, Some(new_modification_timestamp));
    }

    #[test]
    fn test_group_update_in_destination() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let group = get_group(&destination_db, &["group1", "subgroup1"]);
        group.borrow_mut().set_title(Some("subgroup1_updated_name"));
        // Making sure to wait 1 sec before update the timestamp, to make
        // sure that we get a different modification timestamp.
        thread::sleep(time::Duration::from_secs(1));
        let new_modification_timestamp = Times::now();
        group
            .borrow_mut()
            .get_times_mut()
            .set_last_modification(Some(new_modification_timestamp));

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let modified_group = get_group(&destination_db, &["group1", "subgroup1_updated_name"]);
        assert_eq!(modified_group.borrow().get_title(), Some("subgroup1_updated_name"));
        assert_eq!(
            modified_group.borrow().get_times().get_last_modification(),
            Some(new_modification_timestamp),
        );
    }

    #[test]
    fn test_group_update_and_relocation() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let group = get_group(&source_db, &["group1", "subgroup1"]);
        group.borrow_mut().set_title(Some("subgroup1_updated_name"));
        // Making sure to wait 1 sec before update the timestamp, to make
        // sure that we get a different modification timestamp.
        thread::sleep(time::Duration::from_secs(1));
        let new_modification_timestamp = Times::now();
        group
            .borrow_mut()
            .get_times_mut()
            .set_last_modification(Some(new_modification_timestamp));

        source_db
            .relocate_node(
                Uuid::parse_str(SUBGROUP1_ID).unwrap(),
                &[Uuid::parse_str(GROUP1_ID).unwrap()],
                &[Uuid::parse_str(GROUP2_ID).unwrap()],
                new_modification_timestamp,
            )
            .unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 2);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let modified_group = get_group(&destination_db, &["group2", "subgroup1_updated_name"]);
        assert_eq!(modified_group.borrow().get_title(), Some("subgroup1_updated_name"));
        assert_eq!(
            modified_group.borrow().get_times().get_last_modification(),
            Some(new_modification_timestamp),
        );
    }

    #[test]
    fn test_group_update_in_destination_and_relocation_in_source() {
        let mut destination_db = create_test_database();
        let source_db = destination_db.clone();

        let entry_count_before = get_all_entries(&destination_db.root).len();
        let group_count_before = get_all_groups(&destination_db.root).len();

        let group = get_group(&source_db, &["group1", "subgroup1"]);
        group.borrow_mut().set_title(Some("subgroup1_updated_name"));
        // Making sure to wait 1 sec before update the timestamp, to make
        // sure that we get a different modification timestamp.
        thread::sleep(time::Duration::from_secs(1));
        let new_modification_timestamp = Times::now();
        group
            .borrow_mut()
            .get_times_mut()
            .set_last_modification(Some(new_modification_timestamp));

        thread::sleep(time::Duration::from_secs(1));
        let new_location_changed_timestamp = Times::now();
        destination_db
            .relocate_node(
                Uuid::parse_str(SUBGROUP1_ID).unwrap(),
                &[Uuid::parse_str(GROUP1_ID).unwrap()],
                &[Uuid::parse_str(GROUP2_ID).unwrap()],
                new_location_changed_timestamp,
            )
            .unwrap();

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before);

        let modified_group = get_group(&destination_db, &["group2", "subgroup1_updated_name"]);
        assert_eq!(modified_group.borrow().get_title(), Some("subgroup1_updated_name"));
        assert_eq!(
            modified_group.borrow().get_times().get_last_modification(),
            Some(new_modification_timestamp)
        );
        assert_eq!(
            modified_group.borrow().get_times().get_location_changed(),
            Some(new_location_changed_timestamp)
        );
    }
}
