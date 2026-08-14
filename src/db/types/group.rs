use crate::db::{CustomDataItem, Icon, IconId, Times, entry::Entry, node::*, rc_refcell_node};
use std::collections::HashMap;
use uuid::Uuid;

pub(crate) enum SearchField {
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

/// A database group with child groups and entries
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Group {
    /// The unique identifier of the group
    pub(crate) uuid: Uuid,

    /// The name of the group
    pub(crate) name: Option<String>,

    /// Notes for the group
    pub(crate) notes: Option<String>,

    /// Tags assigned to the group
    pub(crate) tags: Vec<String>,

    /// ID of the group's icon
    pub(crate) icon: Icon,

    /// The list of child nodes (Groups or Entries)
    pub(crate) children: Vec<SerializableNodePtr>,

    /// The list of time fields for this group
    pub(crate) times: Times,

    // Custom Data
    pub(crate) custom_data: HashMap<String, CustomDataItem>,

    /// Whether the group is expanded in the user interface
    pub(crate) is_expanded: bool,

    /// Default autotype sequence
    pub(crate) default_autotype_sequence: Option<String>,

    /// Whether autotype is enabled
    pub(crate) enable_autotype: Option<bool>,

    /// Whether searching is enabled
    pub(crate) enable_searching: Option<bool>,

    /// UUID for the last top visible entry
    // TODO figure out what that is supposed to mean. According to the KeePass sourcecode, it has
    // something to do with restoring selected items when re-opening a database.
    pub(crate) last_top_visible_entry: Option<Uuid>,

    pub(crate) parent: Option<Uuid>,

    /// UUID of the group's previous parent
    pub(crate) previous_parent_group: Option<Uuid>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            name: Some("Default Group".to_string()),
            notes: None,
            tags: Vec::new(),
            icon: Icon::BuiltIn(IconId::FOLDER),
            children: Vec::new(),
            times: Times::new(),
            custom_data: Default::default(),
            is_expanded: false,
            default_autotype_sequence: None,
            enable_autotype: None,
            enable_searching: None,
            last_top_visible_entry: None,
            parent: None,
            previous_parent_group: None,
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
            && self.icon == other.icon
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
        rc_refcell_node(new_group)
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    fn get_title(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn set_title(&mut self, title: Option<&str>) {
        self.name = title.map(std::string::ToString::to_string);
    }

    fn get_notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    fn set_notes(&mut self, notes: Option<&str>) {
        self.notes = notes.map(std::string::ToString::to_string);
    }

    fn get_icon(&self) -> Icon {
        self.icon
    }

    fn set_icon(&mut self, icon: Icon) {
        self.icon = icon;
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
            name: Some(name.to_string()),
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
            if let (Some(a), Some(b)) = (a.borrow().downcast_ref::<Group>(), b.borrow().downcast_ref::<Group>()) {
                a == b
            } else if let (Some(a), Some(b)) = (a.borrow().downcast_ref::<Entry>(), b.borrow().downcast_ref::<Entry>()) {
                a == b
            } else {
                false
            }
        })
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = Some(name.to_string());
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn get_tags_mut(&mut self) -> &mut Vec<String> {
        &mut self.tags
    }

    pub fn custom_data(&self) -> &HashMap<String, CustomDataItem> {
        &self.custom_data
    }

    pub fn custom_data_mut(&mut self) -> &mut HashMap<String, CustomDataItem> {
        &mut self.custom_data
    }

    pub fn previous_parent_group(&self) -> Option<Uuid> {
        self.previous_parent_group
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
    /// use keepass_ng::{
    ///     db::{with_node, Database, Entry, Group},
    ///     DatabaseKey,
    /// };
    /// use std::fs::File;
    ///
    /// let mut file = File::open("tests/resources/test_db_with_password.kdbx").unwrap();
    /// let db = Database::open(&mut file, DatabaseKey::new().with_password("demopass")).unwrap();
    ///
    /// let e = Group::get(&db.root, &["General", "Sample Entry #2"]).unwrap();
    /// with_node::<Entry, _, _>(&e, |e| {
    ///     println!("User: {}", e.get_username().unwrap());
    /// });
    /// ```
    pub fn get(group: &NodePtr, path: &[&str]) -> Option<NodePtr> {
        Self::get_internal(group, path, SearchField::Title)
    }

    pub fn get_by_uuid<T: AsRef<str>>(group: &NodePtr, path: &[T]) -> Option<NodePtr> {
        Self::get_internal(group, path, SearchField::Uuid)
    }

    fn get_internal<T: AsRef<str>>(group: &NodePtr, path: &[T], search_field: SearchField) -> Option<NodePtr> {
        if path.is_empty() {
            Some(group.clone())
        } else if path.len() == 1 {
            group_get_children(group)
                .unwrap_or_default()
                .iter()
                .find_map(|node| match search_field.matches(node, path[0].as_ref()) {
                    true => Some(node.clone()),
                    false => None,
                })
        } else {
            let head = path[0].as_ref();
            let tail = &path[1..path.len()];
            let head_group = group_get_children(group).unwrap_or_default().iter().find_map(|node| {
                if node_is_group(node) && search_field.matches(node, head) {
                    Some(node.clone())
                } else {
                    None
                }
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

    pub fn reset_children(&mut self, children: Vec<NodePtr>) {
        let uuid = self.get_uuid();
        children.iter().for_each(|c| c.borrow_mut().set_parent(Some(uuid)));
        self.children = children.into_iter().map(|c| c.into()).collect();
    }
}

#[allow(unused_imports)]
#[cfg(test)]
mod group_tests {
    use super::{Entry, Group, Node, Times};
    #[cfg(feature = "merge")]
    use crate::db::merge::entry_set_field_and_commit;
    use crate::db::{rc_refcell_node, *};
    use std::{thread, time};

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_idempotence() {
        let destination_group = rc_refcell_node(Group::new("group1"));
        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        let count = group_get_children(&destination_group).unwrap().len();
        group_add_child(&destination_group, entry, count).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let sg2: NodePtr = source_group.clone();
        let merge_result = Group::merge(&destination_group, &sg2).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        with_node::<Group, _, _>(&destination_group, |destination_group| {
            assert_eq!(destination_group.children.len(), 1);
            // The 2 groups should be exactly the same after merging, since
            // nothing was performed during the merge.
            with_node::<Group, _, _>(&source_group, |source_group| {
                assert_eq!(destination_group, source_group);
            });

            let entry = destination_group.entries()[0].clone();
            entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();
        });
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

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_add_new_entry() {
        let destination_group = rc_refcell_node(Group::new("group1"));
        let source_group = rc_refcell_node(Group::new("group1"));

        let entry = rc_refcell_node(Entry::default());
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
        assert_eq!(group_get_children(&destination_group).unwrap().len(), 1);
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_add_new_non_root_entry() {
        let destination_group = rc_refcell_node(Group::new("group1"));
        let destination_sub_group = rc_refcell_node(Group::new("subgroup1"));

        group_add_child(&destination_group, destination_sub_group, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();
        let source_sub_group = with_node::<Group, _, _>(&source_group, |g| g.groups()[0].clone()).unwrap();

        let entry: NodePtr = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        let count = group_get_children(&source_sub_group).unwrap().len();
        group_add_child(&source_sub_group, entry, count).unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);
        let destination_entries = with_node::<Group, _, _>(&destination_group, |g| g.get_all_entries(&[])).unwrap();
        assert_eq!(destination_entries.len(), 1);
        let (_created_entry, created_entry_location) = destination_entries.first().unwrap();
        println!("{created_entry_location:?}");
        assert_eq!(created_entry_location.len(), 2);
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_add_new_entry_new_group() {
        let destination_group = rc_refcell_node(Group::new("group1"));
        let _destination_sub_group = rc_refcell_node(Group::new("subgroup1"));
        let source_group = rc_refcell_node(Group::new("group1"));
        let source_sub_group = rc_refcell_node(Group::new("subgroup1"));

        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&source_sub_group, entry, 0).unwrap();
        group_add_child(&source_group, source_sub_group, 0).unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        with_node::<Group, _, _>(&destination_group, |destination_group| {
            let destination_entries = destination_group.get_all_entries(&[]);
            assert_eq!(destination_entries.len(), 1);
            let (_, created_entry_location) = destination_entries.first().unwrap();
            assert_eq!(created_entry_location.len(), 2);
        });
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_entry_relocation_existing_group() {
        let entry = rc_refcell_node(Entry::default());
        let entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        let destination_group = rc_refcell_node(Group::new("group1"));
        let destination_sub_group1 = rc_refcell_node(Group::new("subgroup1"));
        let destination_sub_group2 = rc_refcell_node(Group::new("subgroup2"));
        let destination_sub_group2_uuid = destination_sub_group2.borrow().get_uuid();
        group_add_child(&destination_sub_group1, entry, 0).unwrap();
        group_add_child(&destination_group, destination_sub_group1.borrow().duplicate(), 0).unwrap();
        group_add_child(&destination_group, destination_sub_group2.borrow().duplicate(), 1).unwrap();

        let source_group = destination_group.borrow().duplicate();
        assert_eq!(
            with_node::<Group, _, _>(&source_group, |g| g.get_all_entries(&[])).unwrap().len(),
            1
        );

        let destination_group_uuid = destination_group.borrow().get_uuid();
        let destination_sub_group1_uuid = destination_sub_group1.borrow().get_uuid();

        let location = vec![destination_group_uuid, destination_sub_group1_uuid];
        let removed_entry = Group::remove_entry(&source_group, entry_uuid, &location).unwrap();

        removed_entry.borrow_mut().get_times_mut().set_location_changed(Some(Times::now()));
        assert!(
            with_node::<Group, _, _>(&source_group, |g| g.get_all_entries(&[]))
                .unwrap()
                .is_empty()
        );
        // FIXME we should not have to update the history here. We should
        // have a better compare function in the merge function instead.
        with_node_mut::<Entry, _, _>(&removed_entry, |entry| {
            entry.update_history();
        });

        let location = vec![destination_group_uuid, destination_sub_group2_uuid];

        Group::insert_entry(&source_group, removed_entry, &location).unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let destination_entries = with_node::<Group, _, _>(&destination_group, |g| g.get_all_entries(&[])).unwrap();
        assert_eq!(destination_entries.len(), 1);
        let (_moved_entry, moved_entry_location) = destination_entries.first().unwrap();
        assert_eq!(moved_entry_location.len(), 2);
        assert_eq!(moved_entry_location[0], destination_group_uuid);
        assert_eq!(moved_entry_location[1], destination_sub_group2_uuid);
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_merge_entry_relocation_new_group() {
        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        let destination_group = rc_refcell_node(Group::new("group1"));
        let uuid1 = destination_group.borrow().get_uuid();
        let destination_sub_group = rc_refcell_node(Group::new("subgroup1"));
        group_add_child(&destination_sub_group, entry.borrow().duplicate(), 0).unwrap();
        group_add_child(&destination_group, destination_sub_group, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();
        let source_sub_group = rc_refcell_node(Group::new("subgroup2"));
        let uuid2 = source_sub_group.borrow().get_uuid();
        thread::sleep(time::Duration::from_secs(1));
        with_node_mut::<Entry, _, _>(&entry, |entry| {
            entry.times.set_location_changed(Some(Times::now()));
            // FIXME we should not have to update the history here. We should
            // have a better compare function in the merge function instead.
            entry.update_history();
        });
        group_add_child(&source_sub_group, entry, 0).unwrap();
        with_node_mut::<Group, _, _>(&source_group, |g| {
            g.reset_children(vec![]);
            g.add_child(source_sub_group, 0);
        })
        .unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let destination_entries = with_node::<Group, _, _>(&destination_group, |g| g.get_all_entries(&[])).unwrap();
        assert_eq!(destination_entries.len(), 1);
        let (_, created_entry_location) = destination_entries.first().unwrap();
        assert_eq!(created_entry_location.len(), 2);
        assert_eq!(created_entry_location[0], uuid1);
        assert_eq!(created_entry_location[1], uuid2);
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_update_in_destination_no_conflict() {
        let destination_group = rc_refcell_node(Group::new("group1"));

        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();

        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = with_node::<Group, _, _>(&destination_group, |g| g.entries()[0].clone()).unwrap();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 0);

        let entry = with_node::<Group, _, _>(&destination_group, |g| g.entries()[0].clone()).unwrap();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_update_in_source_no_conflict() {
        let destination_group = rc_refcell_node(Group::new("group1"));

        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = with_node::<Group, _, _>(&source_group, |g| g.entries()[0].clone()).unwrap();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = with_node::<Group, _, _>(&destination_group, |g| g.entries()[0].clone()).unwrap();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated"));
    }

    #[cfg(feature = "merge")]
    #[test]
    fn test_update_with_conflicts() {
        let destination_group = rc_refcell_node(Group::new("group1"));

        let entry = rc_refcell_node(Entry::default());
        let _entry_uuid = entry.borrow().get_uuid();
        entry_set_field_and_commit(&entry, "Title", "entry1").unwrap();
        group_add_child(&destination_group, entry, 0).unwrap();

        let source_group = destination_group.borrow().duplicate();

        let entry = with_node::<Group, _, _>(&destination_group, |g| g.entries()[0].clone()).unwrap();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated_from_destination").unwrap();

        let entry = with_node::<Group, _, _>(&source_group, |g| g.entries()[0].clone()).unwrap();
        entry_set_field_and_commit(&entry, "Title", "entry1_updated_from_source").unwrap();

        let merge_result = Group::merge(&destination_group, &source_group).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry = with_node::<Group, _, _>(&destination_group, |g| g.entries()[0].clone()).unwrap();
        assert_eq!(entry.borrow().get_title(), Some("entry1_updated_from_source"));

        let merged_history = with_node::<Entry, _, _>(&entry, |e| e.history.clone().unwrap()).unwrap();
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

        let general_group = rc_refcell_node(Group::new("General"));
        let sample_entry = rc_refcell_node(Entry::default());
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

        let general_group = rc_refcell_node(Group::new("General"));
        let general_group_uuid = general_group.borrow().get_uuid().to_string();
        let sample_entry = rc_refcell_node(Entry::default());
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
