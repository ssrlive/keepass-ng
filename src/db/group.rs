use crate::{
    db::{entry::Entry, node::*, CustomData, IconId, Times},
    rc_refcell_node, Result,
};
use uuid::Uuid;

pub enum SearchField {
    Uuid,
    Title,
}

impl SearchField {
    pub(crate) fn matches(&self, node: &NodePtr, field_value: &str) -> bool {
        match self {
            SearchField::Uuid => node.borrow().get_uuid().to_string() == field_value,
            SearchField::Title => match node.borrow().get_title() {
                Some(title) => title == field_value,
                None => false,
            },
        }
    }
}

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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Group {
    /// The unique identifier of the group
    pub(crate) uuid: Uuid,

    /// The name of the group
    pub(crate) name: String,

    /// Notes for the group
    pub(crate) notes: Option<String>,

    /// ID of the group's icon
    pub(crate) icon_id: Option<IconId>,

    /// UUID for a custom group icon
    pub(crate) custom_icon_uuid: Option<Uuid>,

    /// The list of child nodes (Groups or Entries)
    pub(crate) children: Vec<SerializableNodePtr>,

    /// The list of time fields for this group
    pub(crate) times: Times,

    // Custom Data
    pub(crate) custom_data: CustomData,

    /// Whether the group is expanded in the user interface
    pub(crate) is_expanded: bool,

    /// Default autotype sequence
    pub(crate) default_autotype_sequence: Option<String>,

    /// Whether autotype is enabled
    // TODO: in example XML files, this is "null" - what should the type be?
    pub(crate) enable_autotype: Option<String>,

    /// Whether searching is enabled
    // TODO: in example XML files, this is "null" - what should the type be?
    pub(crate) enable_searching: Option<String>,

    /// UUID for the last top visible entry
    // TODO figure out what that is supposed to mean. According to the KeePass sourcecode, it has
    // something to do with restoring selected items when re-opening a database.
    pub(crate) last_top_visible_entry: Option<Uuid>,

    pub(crate) parent: Option<Uuid>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            name: "Default Group".to_string(),
            notes: None,
            icon_id: Some(IconId::FOLDER),
            custom_icon_uuid: None,
            children: Vec::new(),
            times: Times::new(),
            custom_data: CustomData::default(),
            is_expanded: false,
            default_autotype_sequence: None,
            enable_autotype: None,
            enable_searching: None,
            last_top_visible_entry: None,
            parent: None,
        }
    }
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.compare_children(other)
            && self.times == other.times
            && self.name == other.name
            && self.notes == other.notes
            && self.icon_id == other.icon_id
            && self.custom_icon_uuid == other.custom_icon_uuid
            && self.is_expanded == other.is_expanded
            && self.default_autotype_sequence == other.default_autotype_sequence
            && self.enable_autotype == other.enable_autotype
            && self.enable_searching == other.enable_searching
            && self.last_top_visible_entry == other.last_top_visible_entry
            && self.custom_data == other.custom_data
        // && self.parent == other.parent
    }
}

impl Eq for Group {}

impl Node for Group {
    fn duplicate(&self) -> NodePtr {
        let mut new_group = self.clone();
        new_group.parent = None;
        new_group.children = self
            .children
            .iter()
            .map(|child| {
                let child = child.borrow().duplicate();
                child.borrow_mut().set_parent(Some(new_group.uuid));
                child.into()
            })
            .collect();
        rc_refcell_node!(new_group)
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    fn get_title(&self) -> Option<&str> {
        Some(&self.name)
    }

    fn set_title(&mut self, title: Option<&str>) {
        self.name = title.unwrap_or_default().to_string();
    }

    fn get_notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    fn set_notes(&mut self, notes: Option<&str>) {
        self.notes = notes.map(std::string::ToString::to_string);
    }

    fn get_icon_id(&self) -> Option<IconId> {
        self.icon_id
    }

    fn set_icon_id(&mut self, icon_id: Option<IconId>) {
        self.icon_id = icon_id;
    }

    fn get_custom_icon_uuid(&self) -> Option<Uuid> {
        self.custom_icon_uuid
    }

    fn get_times(&self) -> &Times {
        &self.times
    }

    fn get_times_mut(&mut self) -> &mut Times {
        &mut self.times
    }

    fn get_parent(&self) -> Option<Uuid> {
        self.parent
    }

    fn set_parent(&mut self, parent: Option<Uuid>) {
        self.parent = parent;
    }
}

impl Group {
    pub fn new(name: &str) -> Group {
        Group {
            name: name.to_string(),
            ..Group::default()
        }
    }

    pub fn get_children(&self) -> Vec<NodePtr> {
        self.children.iter().map(|c| c.into()).collect()
    }

    fn compare_children(&self, other: &Self) -> bool {
        if self.children.len() != other.children.len() {
            return false;
        }
        self.children.iter().zip(other.children.iter()).all(|(a, b)| {
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

    pub fn add_child(&mut self, child: NodePtr, index: usize) {
        child.borrow_mut().set_parent(Some(self.get_uuid()));
        if index < self.children.len() {
            self.children.insert(index, child.into());
        } else {
            self.children.push(child.into());
        }
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
        Self::get_internal(root, path, SearchField::Title)
    }

    #[allow(dead_code)]
    pub(crate) fn get_by_uuid<T: AsRef<str>>(root: &NodePtr, path: &[T]) -> Option<NodePtr> {
        Self::get_internal(root, path, SearchField::Uuid)
    }

    fn get_internal<T: AsRef<str>>(root: &NodePtr, path: &[T], search_field: SearchField) -> Option<NodePtr> {
        if path.is_empty() {
            Some(root.clone())
        } else if path.len() == 1 {
            group_get_children(root).and_then(|c| {
                c.iter().find_map(|node| match search_field.matches(node, path[0].as_ref()) {
                    true => Some(node.clone()),
                    false => None,
                })
            })
        } else {
            let head = path[0].as_ref();
            let tail = &path[1..path.len()];
            let head_group = group_get_children(root).and_then(|c| {
                c.iter().find_map(|node| match node_is_group(node) {
                    true => match search_field.matches(node, head) {
                        true => Some(node.clone()),
                        false => None,
                    },
                    false => None,
                })
            })?;

            Self::get_internal(&head_group, tail, search_field)
        }
    }

    pub fn entries(&self) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in &self.children {
            if node_is_entry(node) {
                response.push(node.into());
            }
        }
        response
    }

    pub fn groups(&self) -> Vec<NodePtr> {
        let mut response: Vec<NodePtr> = vec![];
        for node in &self.children {
            if node_is_group(node) {
                response.push(node.into());
            }
        }
        response
    }

    fn replace_entry(root: &NodePtr, entry: &NodePtr) -> Option<()> {
        let uuid = entry.borrow().get_uuid();
        let target_entry = search_node_by_uuid_with_specific_type::<Entry>(root, uuid);
        Entry::entry_replaced_with(target_entry.as_ref()?, entry)
    }

    pub(crate) fn has_group(root: &NodePtr, uuid: Uuid) -> bool {
        group_get_children(root).map_or(false, |c| c.into_iter().any(|n| n.borrow().get_uuid() == uuid && node_is_group(&n)))
    }

    pub(crate) fn get_group_mut(root: &NodePtr, location: &NodeLocation, create_groups: bool) -> Result<NodePtr> {
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
            let mut current_group: Option<NodePtr> = None;
            for i in (0..(remaining_location.len())).rev() {
                let new_group = rc_refcell_node!(Group::new(&remaining_location[i].name));
                if let Some(current_group) = current_group {
                    let count = group_get_children(&current_group).map_or(0, |c| c.len());
                    group_add_child(&new_group, current_group, count)?;
                }
                current_group = Some(new_group);
            }

            if let Some(current_group) = current_group {
                next_location_uuid = current_group.borrow().get_uuid();
                let count = group_get_children(root).map_or(0, |c| c.len());
                group_add_child(root, current_group, count)?;
            } else {
                return Err("Could not create group.".into());
            }
        }

        let mut target = None;
        for node in group_get_children(root).ok_or("No children.")? {
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

    pub(crate) fn insert_entry(root: &NodePtr, entry: NodePtr, location: &NodeLocation) -> Result<()> {
        let group = Self::get_group_mut(root, location, true)?;
        let count = group_get_children(&group).map_or(0, |c| c.len());
        group_add_child(&group, entry, count)?;
        Ok(())
    }

    pub(crate) fn remove_entry(root: &NodePtr, uuid: Uuid, location: &NodeLocation) -> Result<NodePtr> {
        let group = Self::get_group_mut(root, location, false)?;

        let mut removed_entry: Option<NodePtr> = None;
        let mut new_nodes: Vec<NodePtr> = vec![];
        println!(
            "Searching for entry {} in {}",
            uuid,
            group.borrow().get_title().unwrap_or("No title")
        );
        for node in group_get_children(&group).unwrap_or_default() {
            if node_is_entry(&node) {
                let node_uuid = node.borrow().get_uuid();
                println!("Saw entry {node_uuid}");
                if node_uuid != uuid {
                    new_nodes.push(node.borrow().duplicate());
                    continue;
                }
                removed_entry = Some(node.borrow().duplicate());
            } else if node_is_group(&node) {
                new_nodes.push(node.borrow().duplicate());
            }
        }

        if let Some(entry) = removed_entry {
            group_reset_children(&group, new_nodes)?;
            Ok(entry)
        } else {
            let title = group.borrow().get_title().unwrap_or("No title").to_string();
            Err(format!("Could not find entry {uuid} in group \"{title}\".").into())
        }
    }

    pub(crate) fn find_entry_location(&self, uuid: Uuid) -> Option<NodeLocation> {
        let mut current_location = vec![GroupRef::new(self.uuid, &self.name)];
        for node in &self.children {
            if node_is_entry(node) {
                if node.borrow().get_uuid() == uuid {
                    return Some(current_location);
                }
            } else if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                if let Some(mut location) = g.find_entry_location(uuid) {
                    current_location.append(&mut location);
                    return Some(current_location);
                }
            }
        }
        None
    }

    pub(crate) fn add_entry(parent: &NodePtr, entry: NodePtr, location: &NodeLocation) -> crate::Result<()> {
        if location.is_empty() {
            panic!("TODO handle this with a Response.");
        }

        let mut remaining_location = location.clone();
        remaining_location.remove(0);

        if remaining_location.is_empty() {
            let count = group_get_children(parent).map_or(0, |c| c.len());
            group_add_child(parent, entry, count)?;
            return Ok(());
        }

        let next_location = &remaining_location[0];

        println!("Searching for group {} {:?}", next_location.name, next_location.uuid);
        for node in group_get_children(parent).unwrap_or_default() {
            if node_is_group(&node) {
                if node.borrow().get_uuid() != next_location.uuid {
                    continue;
                }
                Self::add_entry(&node, entry, &remaining_location)?;
                return Ok(());
            }
        }

        // The group was not found, so we create it.
        let new_group = rc_refcell_node!(Group::new(&next_location.name));
        new_group.borrow_mut().set_uuid(next_location.uuid);
        Self::add_entry(&new_group, entry, &remaining_location)?;
        let count = group_get_children(parent).map_or(0, |c| c.len());
        group_add_child(parent, new_group, count)?;
        Ok(())
    }

    /// Merge this group with another group
    #[allow(clippy::too_many_lines)]
    pub fn merge(root: &NodePtr, other_group: &NodePtr) -> Result<MergeLog> {
        let mut log = MergeLog::default();

        let other_entries = other_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .ok_or("Could not downcast other group to group.")?
            .get_all_entries(&vec![]);

        // Handle entry relocation.
        for (entry, entry_location) in &other_entries {
            let entry_uuid = entry.borrow().get_uuid();
            let the_entry = search_node_by_uuid_with_specific_type::<Entry>(root, entry_uuid);

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
                let _ = Group::remove_entry(root, entry_uuid, &existing_entry_location)?;
                Group::insert_entry(root, entry.borrow().duplicate(), entry_location)?;
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
                // merged_entry.borrow_mut().set_parent(existing_entry.borrow().get_parent());
                if node_is_equals_to(&existing_entry, &merged_entry) {
                    continue;
                }

                Group::replace_entry(root, &merged_entry).ok_or("Could not replace entry")?;

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
    pub(crate) fn get_all_entries(&self, current_location: &NodeLocation) -> Vec<(NodePtr, NodeLocation)> {
        let mut response: Vec<(NodePtr, NodeLocation)> = vec![];
        let mut new_location = current_location.clone();
        new_location.push(GroupRef::new(self.uuid, &self.name));

        for node in &self.children {
            if node_is_entry(node) {
                response.push((node.into(), new_location.clone()));
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
    use super::{Entry, Group, GroupRef, Node, Times};
    use crate::{
        db::{entry::entry_set_field_and_commit, *},
        rc_refcell_node,
    };
    use std::{thread, time};

    #[test]
    fn test_merge_idempotence() {
        let destination_group = rc_refcell_node!(Group::new("group1"));
        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        let count = group_get_children(&destination_group).unwrap().len();
        group_add_child(&destination_group, entry, count).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let sg2: NodePtr = source_group.clone();
        let merge_result = Group::merge(&destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        {
            let mut destination_group = destination_group.borrow_mut();
            let destination_group = destination_group.as_any_mut().downcast_mut::<Group>().unwrap();
            assert_eq!(destination_group.children.len(), 1);
            // The 2 groups should be exactly the same after merging, since
            // nothing was performed during the merge.
            let source_group = source_group.borrow();
            let source_group = source_group.as_any().downcast_ref::<Group>().unwrap();
            assert_eq!(destination_group, source_group);

            let entry = destination_group.entries()[0].clone();
            entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();
        }
        let merge_result = Group::merge(&destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let destination_group_just_after_merge = destination_group.borrow().duplicate();
        let merge_result = Group::merge(&destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        // Merging twice in a row, even if the first merge updated the destination group,
        // should not create more changes.
        assert!(node_is_equals_to(&destination_group_just_after_merge, &destination_group));
    }

    #[test]
    fn test_merge_add_new_entry() {
        let destination_group = rc_refcell_node!(Group::new("group1"));
        let source_group = rc_refcell_node!(Group::new("group1"));

        let entry = rc_refcell_node!(Entry::default());
        let entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&source_group, entry, 0).unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);
        {
            assert_eq!(group_get_children(&destination_group).unwrap().len(), 1);
            let new_entry = search_node_by_uuid_with_specific_type::<Entry>(&destination_group, entry_uuid);
            assert!(new_entry.is_some());
            assert_eq!(new_entry.unwrap().borrow().get_title().unwrap(), "entry1");
        }

        // Merging the same group again should not create a duplicate entry.
        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
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
        let destination_group = rc_refcell_node!(Group::new("group1"));
        let destination_sub_group = rc_refcell_node!(Group::new("subgroup1"));

        group_add_child(&destination_group, destination_sub_group, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();
        let source_sub_group = source_group.borrow().as_any().downcast_ref::<Group>().unwrap().groups()[0].clone();

        let entry: NodePtr = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        let count = group_get_children(&source_sub_group).unwrap().len();
        group_add_child(&source_sub_group, entry, count).unwrap();

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
        let (_created_entry, created_entry_location) = destination_entries.first().unwrap();
        println!("{:?}", created_entry_location);
        assert_eq!(created_entry_location.len(), 2);
    }

    #[test]
    fn test_merge_add_new_entry_new_group() {
        let destination_group = rc_refcell_node!(Group::new("group1"));
        let _destination_sub_group = rc_refcell_node!(Group::new("subgroup1"));
        let source_group = rc_refcell_node!(Group::new("group1"));
        let source_sub_group = rc_refcell_node!(Group::new("subgroup1"));

        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&source_sub_group, entry, 0).unwrap();
        group_add_child(&source_group, source_sub_group, 0).unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        {
            let destination_group = destination_group.borrow();
            let destination_group = destination_group.as_any().downcast_ref::<Group>().unwrap();
            let destination_entries = destination_group.get_all_entries(&vec![]);
            assert_eq!(destination_entries.len(), 1);
            let (_, created_entry_location) = destination_entries.first().unwrap();
            assert_eq!(created_entry_location.len(), 2);
        }
    }

    #[test]
    fn test_merge_entry_relocation_existing_group() {
        let entry = rc_refcell_node!(Entry::default());
        let entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        let destination_group = rc_refcell_node!(Group::new("group1"));
        let destination_sub_group1 = rc_refcell_node!(Group::new("subgroup1"));
        let destination_sub_group2 = rc_refcell_node!(Group::new("subgroup2"));
        let destination_sub_group2_uuid = destination_sub_group2.borrow().get_uuid();
        group_add_child(&destination_sub_group1, entry, 0).unwrap();
        group_add_child(&destination_group, destination_sub_group1.borrow().duplicate(), 0).unwrap();
        group_add_child(&destination_group, destination_sub_group2.borrow().duplicate(), 1).unwrap();

        let source_group = destination_group.borrow().duplicate();
        assert!(
            source_group
                .borrow()
                .as_any()
                .downcast_ref::<Group>()
                .unwrap()
                .get_all_entries(&vec![])
                .len()
                == 1
        );

        let destination_group_uuid = destination_group.borrow().get_uuid();
        let destination_sub_group1_uuid = destination_sub_group1.borrow().get_uuid();

        let location = vec![
            GroupRef::new(destination_group_uuid, ""),
            GroupRef::new(destination_sub_group1_uuid, ""),
        ];
        let removed_entry = Group::remove_entry(&source_group, entry_uuid, &location).unwrap();

        removed_entry.borrow_mut().get_times_mut().set_location_changed(Some(Times::now()));
        assert!(source_group
            .borrow()
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
            .get_all_entries(&vec![])
            .is_empty());
        // FIXME we should not have to update the history here. We should
        // have a better compare function in the merge function instead.
        removed_entry
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()
            .unwrap()
            .update_history();

        let location = vec![
            GroupRef::new(destination_group_uuid, ""),
            GroupRef::new(destination_sub_group2_uuid, ""),
        ];

        Group::insert_entry(&source_group, removed_entry, &location).unwrap();

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
        let (_moved_entry, moved_entry_location) = destination_entries.first().unwrap();
        assert_eq!(moved_entry_location.len(), 2);
        assert_eq!(moved_entry_location[0].name, "group1".to_string());
        assert_eq!(moved_entry_location[1].name, "subgroup2".to_string());
    }

    #[test]
    fn test_merge_entry_relocation_new_group() {
        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        let destination_group = rc_refcell_node!(Group::new("group1"));
        let destination_sub_group = rc_refcell_node!(Group::new("subgroup1"));
        group_add_child(&destination_sub_group, entry.borrow().duplicate(), 0).unwrap();
        group_add_child(&destination_group, destination_sub_group, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();
        let source_sub_group = rc_refcell_node!(Group::new("subgroup2"));
        thread::sleep(time::Duration::from_secs(1));
        entry
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<Entry>()
            .unwrap()
            .times
            .set_location_changed(Some(Times::now()));
        // FIXME we should not have to update the history here. We should
        // have a better compare function in the merge function instead.
        entry.borrow_mut().as_any_mut().downcast_mut::<Entry>().unwrap().update_history();
        group_add_child(&source_sub_group, entry, 0).unwrap();
        group_reset_children(&source_group, vec![]).unwrap();
        group_add_child(&source_group, source_sub_group, 0).unwrap();

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
        let (_, created_entry_location) = destination_entries.first().unwrap();
        assert_eq!(created_entry_location.len(), 2);
        assert_eq!(created_entry_location[0].name, "group1".to_string());
        assert_eq!(created_entry_location[1].name, "subgroup2".to_string());
    }

    #[test]
    fn test_update_in_destination_no_conflict() {
        let destination_group = rc_refcell_node!(Group::new("group1"));

        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = destination_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry = destination_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_in_source_no_conflict() {
        let destination_group = rc_refcell_node!(Group::new("group1"));

        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = source_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = destination_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[test]
    fn test_update_with_conflicts() {
        let destination_group = rc_refcell_node!(Group::new("group1"));

        let entry = rc_refcell_node!(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = destination_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated_from_destination").unwrap();

        let entry = source_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated_from_source").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = destination_group.borrow().as_any().downcast_ref::<Group>().unwrap().entries()[0].clone();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated_from_source"));

        let merged_history = entry.borrow().as_any().downcast_ref::<Entry>().unwrap().history.clone().unwrap();
        assert!(merged_history.is_ordered());
        assert_eq!(merged_history.entries.len(), 3);
        let merged_entry = &merged_history.entries[1];
        assert_eq!(merged_entry.get_title(), Some("entry1_updated_from_destination"));

        // Merging again should not result in any additional change.
        let destination_group_dup = destination_group.borrow().duplicate();
        let merge_result = Group::merge(&destination_group, &destination_group_dup).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);
    }

    #[test]
    fn get() {
        let db = Database::new(Default::default());

        let general_group = rc_refcell_node!(Group::new("General"));
        let sample_entry = rc_refcell_node!(Entry::default());
        sample_entry.borrow_mut().set_title(Some("Sample Entry #2"));
        group_add_child(&general_group, sample_entry, 0).unwrap();
        group_add_child(&db.root, general_group, 0).unwrap();

        assert!(Group::get(&db.root, &["General", "Sample Entry #2"]).is_some());
        assert!(Group::get(&db.root, &["General"]).is_some());
        assert!(Group::get(&db.root, &["Invalid Group"]).is_none());
        assert!(Group::get(&db.root, &[]).is_some());
    }

    #[test]
    fn get_by_uuid() {
        let db = Database::new(Default::default());

        let general_group = rc_refcell_node!(Group::new("General"));
        let general_group_uuid = general_group.borrow().get_uuid().to_string();
        let sample_entry = rc_refcell_node!(Entry::default());
        let sample_entry_uuid = sample_entry.borrow().get_uuid().to_string();
        sample_entry.borrow_mut().set_title(Some("Sample Entry #2"));
        group_add_child(&general_group, sample_entry, 0).unwrap();
        group_add_child(&db.root, general_group, 0).unwrap();

        let invalid_uuid = uuid::Uuid::new_v4().to_string();

        // Testing with references to the UUIDs
        let group_path: [&str; 1] = [general_group_uuid.as_ref()];
        let entry_path: [&str; 2] = [general_group_uuid.as_ref(), sample_entry_uuid.as_ref()];
        let invalid_path: [&str; 1] = [invalid_uuid.as_ref()];
        let empty_path: [&str; 0] = [];

        assert!(Group::get_by_uuid(&db.root, &group_path).is_some());
        assert!(Group::get_by_uuid(&db.root, &entry_path).is_some());
        assert!(Group::get_by_uuid(&db.root, &invalid_path).is_none());
        assert!(Group::get_by_uuid(&db.root, &empty_path).is_some());

        // Testing with owned versions of the UUIDs.
        let group_path = vec![general_group_uuid.clone()];
        let entry_path = vec![general_group_uuid.clone(), sample_entry_uuid.clone()];
        let invalid_path = vec![invalid_uuid.clone()];
        let empty_path: Vec<String> = vec![];

        assert!(Group::get_by_uuid(&db.root, &group_path).is_some());
        assert!(Group::get_by_uuid(&db.root, &entry_path).is_some());
        assert!(Group::get_by_uuid(&db.root, &invalid_path).is_none());
        assert!(Group::get_by_uuid(&db.root, &empty_path).is_some());
    }
}
