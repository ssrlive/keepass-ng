use crate::{
    db::{entry::Entry, node::*, CustomData, Times},
    rc_refcell, Result,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum MergeEventType {
    EntryCreated,
    EntryLocationUpdated,

    EntryUpdated,
    GroupCreated,
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

impl MergeLog {
    pub fn merge_with(&self, other: &MergeLog) -> MergeLog {
        let mut response = MergeLog::default();
        response.warnings.append(self.warnings.clone().as_mut());
        response.warnings.append(other.warnings.clone().as_mut());
        response.events.append(self.events.clone().as_mut());
        response.events.append(other.events.clone().as_mut());
        response
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub(crate) struct GroupRef {
    pub uuid: Uuid,
    pub name: String,
}

impl GroupRef {
    pub fn new(uuid: Uuid, name: &str) -> Self {
        let name = name.to_string();
        Self { uuid, name }
    }
}

pub(crate) type NodeLocation = Vec<GroupRef>;

/// A database group with child groups and entries
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Group {
    /// The unique identifier of the group
    pub uuid: Uuid,

    /// The name of the group
    pub name: String,

    /// Notes for the group
    pub notes: Option<String>,

    /// ID of the group's icon
    pub icon_id: Option<usize>,

    /// UUID for a custom group icon
    pub custom_icon_uuid: Option<Uuid>,

    /// The list of child nodes (Groups or Entries)
    pub children: Vec<NodePtr>,

    /// The list of time fields for this group
    pub times: Times,

    // Custom Data
    pub custom_data: CustomData,

    /// Whether the group is expanded in the user interface
    pub is_expanded: bool,

    /// Default autotype sequence
    pub default_autotype_sequence: Option<String>,

    /// Whether autotype is enabled
    // TODO: in example XML files, this is "null" - what should the type be?
    pub enable_autotype: Option<String>,

    /// Whether searching is enabled
    // TODO: in example XML files, this is "null" - what should the type be?
    pub enable_searching: Option<String>,

    /// UUID for the last top visible entry
    // TODO figure out what that is supposed to mean. According to the KeePass sourcecode, it has
    // something to do with restoring selected items when re-opening a database.
    pub last_top_visible_entry: Option<Uuid>,
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.compare_children(other)
    }
}

impl Eq for Group {}

impl Node for Group {
    fn duplicate(&self) -> NodePtr {
        let mut new_group = self.clone();
        new_group.children = Vec::with_capacity(self.children.len());
        for child in self.children.iter() {
            new_group.children.push(child.borrow().duplicate());
        }
        rc_refcell!(new_group)
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn get_title(&self) -> Option<&str> {
        Some(&self.name)
    }

    fn get_notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    fn get_icon_id(&self) -> Option<usize> {
        self.icon_id
    }

    fn get_custom_icon_uuid(&self) -> Option<&Uuid> {
        self.custom_icon_uuid.as_ref()
    }

    fn get_children(&self) -> Option<Vec<NodePtr>> {
        Some(self.children.clone())
    }

    fn get_times(&self) -> &Times {
        &self.times
    }
}

impl Group {
    pub fn new(name: &str) -> Group {
        Group {
            name: name.to_string(),
            times: Times::new(),
            uuid: Uuid::new_v4(),
            ..Group::default()
        }
    }

    fn compare_children(&self, other: &Self) -> bool {
        if self.children.len() != other.children.len() {
            return false;
        }
        self.children
            .iter()
            .zip(other.children.iter())
            .all(|(a, b)| {
                if let (Some(a), Some(b)) = (
                    a.borrow().as_any().downcast_ref::<Group>(),
                    b.borrow().as_any().downcast_ref::<Group>(),
                ) {
                    a == b
                } else if let (Some(a), Some(b)) = (
                    a.borrow().as_any().downcast_ref::<Entry>(),
                    b.borrow().as_any().downcast_ref::<Entry>(),
                ) {
                    a == b
                } else {
                    false
                }
            })
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn add_child(&mut self, child: NodePtr) {
        self.children.push(child);
    }

    /// Recursively get a Group or Entry reference by specifying a path relative to the current Group
    /// ```
    /// use keepass::{
    ///     db::{Entry, Group},
    ///     Database, DatabaseKey,
    /// };
    /// use std::fs::File;
    ///
    /// let mut file = File::open("tests/resources/test_db_with_password.kdbx").unwrap();
    /// let db = Database::open(&mut file, DatabaseKey::new().with_password("demopass")).unwrap();
    ///
    /// if let Some(e) = Group::get(&db.root, &["General", "Sample Entry #2"]) {
    ///     if let Some(e) = e.borrow().as_any().downcast_ref::<Entry>() {
    ///         println!("User: {}", e.get_username().unwrap());
    ///     }
    /// }
    /// ```
    pub fn get(root: &NodePtr, path: &[&str]) -> Option<NodePtr> {
        if path.is_empty() {
            Some(root.clone())
        } else if path.len() == 1 {
            let head = path[0];
            root.borrow().get_children().and_then(|c| {
                c.into_iter()
                    .find(|n| n.borrow().get_title().map(|t| t == head).unwrap_or(false))
            })
        } else {
            let head = path[0];
            let tail = &path[1..path.len()];
            let head_group = root.borrow().get_children().and_then(|c| {
                c.into_iter().find(|n| {
                    n.borrow().as_any().downcast_ref::<Group>().is_some()
                        && n.borrow().get_title().map(|t| t == head).unwrap_or(false)
                })
            })?;

            Self::get(&head_group, tail)
        }
    }

    /// Get a timestamp field by name
    ///
    /// Returning the chrono::NaiveDateTime which does not include timezone
    /// or UTC offset because KeePass clients typically store timestamps
    /// relative to the local time on the machine writing the data without
    /// including accurate UTC offset or timezone information.
    pub fn get_time(&self, key: &str) -> Option<&chrono::NaiveDateTime> {
        self.times.get(key)
    }

    /// Convenience method for getting the time that the group expires
    pub fn get_expiry_time(&self) -> Option<&chrono::NaiveDateTime> {
        self.times.get_expiry()
    }

    pub fn entries(&self) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in &self.children {
            if node_is_entry(node) {
                response.push(node.clone());
            }
        }
        response
    }

    pub fn groups(&self) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in &self.children {
            if node_is_group(node) {
                response.push(node.clone());
            }
        }
        response
    }

    fn replace_entry(&mut self, entry: &NodePtr) -> Option<()> {
        let mut target_entry = None;
        let uuid = entry.borrow().get_uuid();
        for node in self.get_children()?.iter() {
            let tmp = NodeIterator::new(node)
                .filter(|n| node_is_entry(n))
                .find(|n| n.borrow().get_uuid() == uuid);
            if tmp.is_some() {
                target_entry = tmp;
                break;
            }
        }

        let entry = entry.borrow();
        let entry = entry.as_any().downcast_ref::<Entry>()?;
        target_entry
            .as_ref()?
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()?
            .replace_with(entry);

        Some(())
    }

    pub(crate) fn has_group(root: &NodePtr, uuid: Uuid) -> bool {
        root.borrow()
            .get_children()
            .map(|c| {
                c.into_iter()
                    .any(|n| n.borrow().get_uuid() == uuid && node_is_group(&n))
            })
            .unwrap_or(false)
    }

    pub(crate) fn get_group_mut(
        root: &NodePtr,
        location: &NodeLocation,
        create_groups: bool,
    ) -> Result<NodePtr> {
        if location.is_empty() {
            return Err("Empty location.".into());
        }

        let mut remaining_location = location.clone();
        remaining_location.remove(0);

        if remaining_location.is_empty() {
            return Ok(root.clone());
        }

        let next_location = &remaining_location[0];
        let mut next_location_uuid = next_location.uuid;

        if !Self::has_group(root, next_location_uuid) && create_groups {
            let mut current_group: Option<Group> = None;
            for i in (0..(remaining_location.len())).rev() {
                let mut new_group = Group::new(&remaining_location[i].name);
                if let Some(current_group) = current_group {
                    new_group.add_child(rc_refcell!(current_group));
                }
                current_group = Some(new_group);
            }

            if let Some(current_group) = current_group {
                next_location_uuid = current_group.uuid;
                node_add_child(root, rc_refcell!(current_group)).ok_or("Add child failed")?;
            } else {
                return Err("Could not create group.".into());
            }
        }

        let mut target = None;
        for node in root.borrow().get_children().ok_or("No children.")? {
            if node_is_group(&node) && node.borrow().get_uuid() == next_location_uuid {
                target = Some(node);
                break;
            }
        }

        if let Some(target) = target {
            return Self::get_group_mut(&target, &remaining_location, create_groups);
        }
        Err("The group was not found.".into())
    }

    pub(crate) fn insert_entry(
        root: &NodePtr,
        entry: NodePtr,
        location: &NodeLocation,
    ) -> Result<()> {
        let group = Self::get_group_mut(root, location, true)?;
        node_add_child(&group, entry).ok_or("Could not add entry.")?;
        Ok(())
    }

    pub(crate) fn remove_entry(
        root: &NodePtr,
        uuid: Uuid,
        location: &NodeLocation,
    ) -> Result<NodePtr> {
        let group = Self::get_group_mut(root, location, false)?;

        let mut removed_entry: Option<NodePtr> = None;
        let mut new_nodes: Vec<NodePtr> = vec![];
        println!(
            "Searching for entry {} in {}",
            uuid,
            group.borrow().get_title().unwrap_or("No title")
        );
        for node in group.borrow().get_children().unwrap_or(vec![]) {
            if node_is_entry(&node) {
                let node_uuid = node.borrow().get_uuid();
                println!("Saw entry {}", node_uuid);
                if node_uuid != uuid {
                    new_nodes.push(node.clone());
                    continue;
                }
                removed_entry = Some(node.clone());
            } else if node_is_group(&node) {
                new_nodes.push(node.clone());
            }
        }

        if let Some(entry) = removed_entry {
            group
                .borrow_mut()
                .as_any_mut()
                .downcast_mut::<Group>()
                .ok_or("Could not downcast group.")?
                .children = new_nodes;
            Ok(entry)
        } else {
            let title = group.borrow().get_title().unwrap_or("No title").to_string();
            Err(format!("Could not find entry {} in group \"{}\".", uuid, title).into())
        }
    }

    pub(crate) fn find_entry_location(&self, id: Uuid) -> Option<NodeLocation> {
        let mut current_location = vec![GroupRef::new(self.uuid, &self.name)];
        for node in &self.children {
            if node_is_entry(node) {
                if node.borrow().get_uuid() == id {
                    return Some(current_location);
                }
            } else if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                if let Some(mut location) = g.find_entry_location(id) {
                    current_location.append(&mut location);
                    return Some(current_location);
                }
            }
        }
        None
    }

    pub fn find_entry_by_uuid(&self, id: Uuid) -> Option<NodePtr> {
        self.get_children().and_then(|children| {
            children.iter().find_map(|node| {
                if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                    return g.find_entry_by_uuid(id);
                }
                if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                    if e.uuid == id {
                        return Some(node.clone());
                    }
                }
                None
            })
        })
    }

    pub(crate) fn add_entry(&mut self, entry: NodePtr, location: &NodeLocation) {
        if location.is_empty() {
            panic!("TODO handle this with a Response.");
        }

        let mut remaining_location = location.clone();
        remaining_location.remove(0);

        if remaining_location.is_empty() {
            self.add_child(entry);
            return;
        }

        let next_location = &remaining_location[0];

        println!(
            "Searching for group {} {:?}",
            next_location.name, next_location.uuid
        );
        for node in &mut self.children {
            if let Some(g) = node.borrow_mut().as_any_mut().downcast_mut::<Group>() {
                if g.uuid != next_location.uuid {
                    continue;
                }
                g.add_entry(entry, &remaining_location);
                return;
            }
        }

        // The group was not found, so we create it.
        let mut new_group = Group {
            name: next_location.name.clone(),
            uuid: next_location.uuid,
            ..Group::default()
        };
        new_group.add_entry(entry, &remaining_location);
        self.add_child(rc_refcell!(new_group));
    }

    /// Merge this group with another group
    pub fn merge(root: &NodePtr, other_group: &NodePtr) -> Result<MergeLog> {
        let mut log = MergeLog::default();

        let other_entries = other_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .ok_or("Could not downcast other group to group.")?
            .get_all_entries(&vec![]);

        // Handle entry relocation.
        for (entry, entry_location) in other_entries.iter() {
            let entry_uuid = entry.borrow().get_uuid();
            let the_entry = root
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .ok_or("Could not downcast root to group.")?
                .find_entry_by_uuid(entry_uuid);

            let existing_entry = match the_entry {
                Some(e) => e,
                None => continue,
            };

            let the_entry_location = root
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .ok_or("Could not downcast root to group.")?
                .find_entry_location(entry_uuid);

            let existing_entry_location = match the_entry_location {
                Some(l) => l,
                None => continue,
            };

            let source_location_changed_time =
                match entry.borrow().get_times().get_location_changed() {
                    Some(t) => *t,
                    None => {
                        log.warnings.push(format!(
                            "Entry {} did not have a location updated timestamp",
                            entry_uuid
                        ));
                        Times::epoch()
                    }
                };
            let destination_location_changed =
                match existing_entry.borrow().get_times().get_location_changed() {
                    Some(t) => *t,
                    None => {
                        log.warnings.push(format!(
                            "Entry {} did not have a location updated timestamp",
                            entry_uuid
                        ));
                        Times::now()
                    }
                };
            if source_location_changed_time > destination_location_changed {
                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryLocationUpdated,
                    node_uuid: entry_uuid,
                });
                let _ = Group::remove_entry(root, entry_uuid, &existing_entry_location)?;
                Group::insert_entry(root, entry.clone(), entry_location)?;
            }
        }

        // Handle entry updates
        for (entry, entry_location) in other_entries.iter() {
            let entry_uuid = entry.borrow().get_uuid();
            let the_entry = root
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .ok_or("Could not downcast root to group.")?
                .find_entry_by_uuid(entry_uuid);
            if let Some(existing_entry) = the_entry {
                if is_nodes_equal(&existing_entry, entry) {
                    continue;
                }

                let source_last_modification =
                    match entry.borrow().get_times().get_last_modification() {
                        Some(t) => *t,
                        None => {
                            log.warnings.push(format!(
                                "Entry {} did not have a last modification timestamp",
                                entry_uuid
                            ));
                            Times::epoch()
                        }
                    };
                let destination_last_modification =
                    match existing_entry.borrow().get_times().get_last_modification() {
                        Some(t) => *t,
                        None => {
                            log.warnings.push(format!(
                                "Entry {} did not have a last modification timestamp",
                                entry_uuid
                            ));
                            Times::now()
                        }
                    };

                if destination_last_modification == source_last_modification {
                    if !is_nodes_equal(&existing_entry, entry) {
                        // This should never happen.
                        // This means that an entry was updated without updating the last modification
                        // timestamp.
                        return Err(
                            "Entries have the same modification time but are not the same!".into(),
                        );
                    }
                    continue;
                }

                let (merged_entry, entry_merge_log) =
                    if destination_last_modification > source_last_modification {
                        existing_entry
                            .borrow_mut()
                            .as_any_mut()
                            .downcast_mut::<Entry>()
                            .ok_or("Could not downcast existing entry to Entry.")?
                            .merge(entry)?
                    } else {
                        entry
                            .clone()
                            .borrow_mut()
                            .as_any_mut()
                            .downcast_mut::<Entry>()
                            .ok_or("Could not downcast entry to Entry.")?
                            .merge(&existing_entry)?
                    };
                if is_nodes_equal(&existing_entry, &merged_entry) {
                    continue;
                }

                root.borrow_mut()
                    .as_any_mut()
                    .downcast_mut::<Group>()
                    .unwrap()
                    .replace_entry(&merged_entry);

                log.events.push(MergeEvent {
                    event_type: MergeEventType::EntryUpdated,
                    node_uuid: merged_entry.borrow().get_uuid(),
                });
                log = log.merge_with(&entry_merge_log);
            } else {
                root.borrow_mut()
                    .as_any_mut()
                    .downcast_mut::<Group>()
                    .ok_or("Could not downcast root to group.")?
                    .add_entry(entry.clone(), entry_location);
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
    pub(crate) fn get_all_entries(
        &self,
        current_location: &NodeLocation,
    ) -> Vec<(NodePtr, NodeLocation)> {
        let mut response: Vec<(NodePtr, NodeLocation)> = vec![];
        let mut new_location = current_location.clone();
        new_location.push(GroupRef::new(self.uuid, &self.name));

        for node in self.children.iter() {
            if node_is_entry(node) {
                response.push((node.clone(), new_location.clone()));
            } else if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                let mut new_entries = g.get_all_entries(&new_location);
                response.append(&mut new_entries);
            }
        }
        response
    }
}

#[cfg(test)]
mod group_tests {
    use crate::{db::NodePtr, rc_refcell};

    use super::{Entry, Group, GroupRef, Node, Times};
    use std::{thread, time};

    #[test]
    fn test_merge_idempotence() {
        let mut destination_group = Group::new("group1");
        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");
        destination_group.add_child(rc_refcell!(entry));

        let mut destination_group: NodePtr = rc_refcell!(destination_group);

        let source_group = destination_group.borrow().duplicate();

        let sg2: NodePtr = source_group.clone();
        let merge_result = Group::merge(&mut destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        {
            let mut destination_group = destination_group.borrow_mut();
            let destination_group = destination_group
                .as_any_mut()
                .downcast_mut::<Group>()
                .unwrap();
            assert_eq!(destination_group.children.len(), 1);
            // The 2 groups should be exactly the same after merging, since
            // nothing was performed during the merge.
            let source_group = source_group.borrow();
            let source_group = source_group.as_any().downcast_ref::<Group>().unwrap();
            assert_eq!(destination_group, source_group);

            let entry = &mut destination_group.entries()[0];
            let mut entry = entry.borrow_mut();
            if let Some(entry) = entry.as_any_mut().downcast_mut::<Entry>() {
                entry.set_field_and_commit("Title", "entry1_updated");
            }
        }
        let merge_result = Group::merge(&mut destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let destination_group_just_after_merge = destination_group.borrow().duplicate();
        let merge_result = Group::merge(&mut destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        // Merging twice in a row, even if the first merge updated the destination group,
        // should not create more changes.
        {
            let destination_group_just_after_merge = destination_group_just_after_merge.borrow();
            let destination_group_just_after_merge = destination_group_just_after_merge
                .as_any()
                .downcast_ref::<Group>()
                .unwrap();
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            assert_eq!(destination_group_just_after_merge, destination_group);
        }
    }

    #[test]
    fn test_merge_add_new_entry() {
        let destination_group = Group::new("group1");
        let mut source_group = Group::new("group1");

        let mut entry = Entry::new();
        let entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");
        source_group.add_child(rc_refcell!(entry));

        let mut destination_group: NodePtr = rc_refcell!(destination_group);
        let source_group: NodePtr = rc_refcell!(source_group);
        let merge_result = Group::merge(&mut destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);
        {
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            assert_eq!(destination_group.children.len(), 1);
            let new_entry = destination_group.find_entry_by_uuid(entry_uuid);
            assert!(new_entry.is_some());
            assert_eq!(
                new_entry
                    .unwrap()
                    .borrow()
                    .as_any()
                    .downcast_ref::<Entry>()
                    .unwrap()
                    .get_title()
                    .unwrap(),
                "entry1".to_string()
            );
        }

        // Merging the same group again should not create a duplicate entry.
        let merge_result = Group::merge(&mut destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
        {
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            assert_eq!(destination_group.children.len(), 1);
        }
    }

    #[test]
    fn test_merge_add_new_non_root_entry() {
        let mut destination_group = Group::new("group1");
        let destination_sub_group = Group::new("subgroup1");
        destination_group.add_child(rc_refcell!(destination_sub_group));

        let source_group = destination_group.duplicate();
        let source_sub_group = source_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .groups()[0]
            .clone();

        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid;
        entry.set_field_and_commit("Title", "entry1");
        source_sub_group
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Group>()
            .unwrap()
            .add_child(rc_refcell!(entry));

        let destination_group: NodePtr = rc_refcell!(destination_group);

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);
        let destination_entries = destination_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .get_all_entries(&vec![]);
        assert_eq!(destination_entries.len(), 1);
        let (_created_entry, created_entry_location) = destination_entries.get(0).unwrap();
        println!("{:?}", created_entry_location);
        assert_eq!(created_entry_location.len(), 2);
    }

    #[test]
    fn test_merge_add_new_entry_new_group() {
        let destination_group = Group::new("group1");
        let mut _destination_sub_group = Group::new("subgroup1");
        let mut source_group = Group::new("group1");
        let mut source_sub_group = Group::new("subgroup1");

        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");
        source_sub_group.children.push(rc_refcell!(entry));
        source_group.children.push(rc_refcell!(source_sub_group));

        let mut destination_group: NodePtr = rc_refcell!(destination_group);
        let source_group: NodePtr = rc_refcell!(source_group);
        let merge_result = Group::merge(&mut destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        {
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            let destination_entries = destination_group.get_all_entries(&vec![]);
            assert_eq!(destination_entries.len(), 1);
            let (_, created_entry_location) = destination_entries.get(0).unwrap();
            assert_eq!(created_entry_location.len(), 2);
        }
    }

    #[test]
    fn test_merge_entry_relocation_existing_group() {
        let mut entry = Entry::new();
        let entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");
        let mut destination_group = Group::new("group1");
        let mut destination_sub_group1 = Group::new("subgroup1");
        let destination_sub_group2 = Group::new("subgroup2");
        destination_sub_group1.add_child(rc_refcell!(entry.clone()));
        destination_group.add_child(rc_refcell!(destination_sub_group1.clone()));
        destination_group.add_child(rc_refcell!(destination_sub_group2.clone()));

        let destination_group_uuid = destination_group.uuid;
        let destination_sub_group1_uuid = destination_sub_group1.uuid;

        let mut destination_group: NodePtr = rc_refcell!(destination_group);
        let mut source_group = destination_group.borrow().duplicate();

        assert_eq!(
            source_group
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .unwrap()
                .get_all_entries(&vec![])
                .len(),
            1
        );

        let location = vec![
            GroupRef::new(destination_group_uuid, ""),
            GroupRef::new(destination_sub_group1_uuid, ""),
        ];
        let removed_entry = Group::remove_entry(&source_group, entry_uuid, &location).unwrap();
        {
            removed_entry
                .borrow_mut()
                .as_any_mut()
                .downcast_mut::<Entry>()
                .unwrap()
                .times
                .set_location_changed(Times::now());
        }
        assert_eq!(
            source_group
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .unwrap()
                .get_all_entries(&vec![])
                .len(),
            0
        );

        // FIXME we should not have to update the history here. We should
        // have a better compare function in the merge function instead.
        {
            removed_entry
                .borrow_mut()
                .as_any_mut()
                .downcast_mut::<Entry>()
                .unwrap()
                .update_history();
        }

        let location = vec![
            GroupRef::new(destination_group_uuid, ""),
            GroupRef::new(destination_sub_group2.uuid.clone(), ""),
        ];
        Group::insert_entry(&mut source_group, removed_entry, &location).unwrap();

        // let source_group: NodePtr = rc_refcell!(source_group.clone());
        let merge_result = Group::merge(&mut destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        {
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            let destination_entries = destination_group.get_all_entries(&vec![]);
            assert_eq!(destination_entries.len(), 1);
            let (_, moved_entry_location) = destination_entries.get(0).unwrap();
            assert_eq!(moved_entry_location.len(), 2);
            assert_eq!(moved_entry_location[0].name, "group1".to_string());
            assert_eq!(moved_entry_location[1].name, "subgroup2".to_string());
        }
    }

    #[test]
    fn test_merge_entry_relocation_new_group() {
        let (destination_group, source_group) = {
            let mut entry = Entry::new();
            let _entry_uuid = entry.uuid.clone();
            entry.set_field_and_commit("Title", "entry1");
            let mut destination_group = Group::new("group1");
            let mut destination_sub_group = Group::new("subgroup1");
            destination_sub_group.add_child(rc_refcell!(entry.clone()));
            destination_group.add_child(rc_refcell!(destination_sub_group));

            let source_group = destination_group.duplicate();
            let mut source_group = source_group.borrow_mut();
            let source_group = source_group.as_any_mut().downcast_mut::<Group>().unwrap();
            let mut source_sub_group = Group::new("subgroup2");
            thread::sleep(time::Duration::from_secs(1));
            entry.times.set_location_changed(Times::now());
            // FIXME we should not have to update the history here. We should
            // have a better compare function in the merge function instead.
            entry.update_history();
            source_sub_group.add_child(rc_refcell!(entry.clone()));
            source_group.children = vec![];
            source_group.add_child(rc_refcell!(source_sub_group));
            let destination_group: NodePtr = rc_refcell!(destination_group);
            let source_group: NodePtr = rc_refcell!(source_group.clone());
            (destination_group, source_group)
        };

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let destination_group = destination_group.borrow();
        let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();

        let destination_entries = destination_group.get_all_entries(&vec![]);
        assert_eq!(destination_entries.len(), 1);
        let (_, created_entry_location) = destination_entries.get(0).unwrap();
        assert_eq!(created_entry_location.len(), 2);
        assert_eq!(created_entry_location[0].name, "group1".to_string());
        assert_eq!(created_entry_location[1].name, "subgroup2".to_string());
    }

    #[test]
    fn test_update_in_destination_no_conflict() {
        let mut destination_group = Group::new("group1");

        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");

        destination_group.add_child(rc_refcell!(entry));

        let source_group = destination_group.duplicate();

        let entry = &mut destination_group.entries()[0];
        if let Some(entry) = entry.borrow_mut().as_any_mut().downcast_mut::<Entry>() {
            entry.set_field_and_commit("Title", "entry1_updated");
        }

        let mut destination_group: NodePtr = rc_refcell!(destination_group);

        let merge_result = Group::merge(&mut destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let destination_group = destination_group.borrow();
        let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();

        let entry = &destination_group.entries()[0];
        let entry = entry.borrow();
        let entry = entry.as_any().downcast_ref::<Entry>().unwrap();
        assert_eq!(entry.get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_in_source_no_conflict() {
        let mut destination_group = Group::new("group1");

        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid.clone();
        entry.set_field_and_commit("Title", "entry1");
        destination_group.add_child(rc_refcell!(entry));

        let source_group = destination_group.duplicate();

        let entry = source_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .entries()[0]
            .clone();
        entry
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()
            .unwrap()
            .set_field_and_commit("Title", "entry1_updated");

        let destination_group: NodePtr = rc_refcell!(destination_group);

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = destination_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .entries()[0]
            .clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_with_conflicts() {
        let mut destination_group = Group::new("group1");

        let mut entry = Entry::new();
        let _entry_uuid = entry.uuid;
        entry.set_field_and_commit("Title", "entry1");
        destination_group.add_child(rc_refcell!(entry));

        let source_group = destination_group.duplicate();

        let entry = destination_group.entries()[0].clone();
        entry
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()
            .unwrap()
            .set_field_and_commit("Title", "entry1_updated_from_destination");

        let entry = source_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .entries()[0]
            .clone();
        entry
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()
            .unwrap()
            .set_field_and_commit("Title", "entry1_updated_from_source");

        let destination_group: NodePtr = rc_refcell!(destination_group);

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = destination_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .entries()[0]
            .clone();
        assert_eq!(
            entry.borrow().get_title(),
            Some("entry1_updated_from_source")
        );

        let merged_history = entry
            .borrow()
            .as_any()
            .downcast_ref::<Entry>()
            .unwrap()
            .history
            .clone()
            .unwrap();
        assert!(merged_history.is_ordered());
        assert_eq!(merged_history.entries.len(), 3);
        let merged_entry = &merged_history.entries[1];
        assert_eq!(
            merged_entry.get_title(),
            Some("entry1_updated_from_destination")
        );

        // Merging again should not result in any additional change.
        let destination_group_dup = destination_group.borrow().duplicate();
        let merge_result = Group::merge(&destination_group, &destination_group_dup).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
    }
}
