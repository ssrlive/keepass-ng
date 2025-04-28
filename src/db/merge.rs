use uuid::Uuid;

pub(crate) type NodeLocation = Vec<Uuid>;

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
    FindGroupError(NodeLocation),

    #[error("Could not find entry at {0:?}")]
    FindEntryError(NodeLocation),

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

        destination_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_entry_uuid,
            deletion_time: Times::now(),
        });

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

        destination_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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

        let modified_entry = Group::find_entry(&destination_db.root, &vec![modified_entry_uuid]).unwrap();
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

        destination_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_entry_uuid,
            deletion_time: Times::now(),
        });

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before - 1);
        assert_eq!(group_count_after, group_count_before);

        let new_entry = Group::find_node_location(&destination_db.root, deleted_entry_uuid);
        assert!(new_entry.is_none());

        assert!(destination_db.deleted_objects.contains(deleted_entry_uuid));
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
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

        let merge_result = destination_db.merge(&source_db).unwrap();
        assert_eq!(merge_result.warnings.len(), 0);
        assert_eq!(merge_result.events.len(), 1);

        let entry_count_after = get_all_entries(&destination_db.root).len();
        let group_count_after = get_all_groups(&destination_db.root).len();
        assert_eq!(entry_count_after, entry_count_before);
        assert_eq!(group_count_after, group_count_before - 1);

        let deleted_group = Group::find_node_location(&destination_db.root, deleted_group_uuid);
        assert!(deleted_group.is_none());

        assert!(destination_db.deleted_objects.contains(deleted_group_uuid));
    }

    #[test]
    fn test_deleted_entry_in_source_modified_in_destination() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_entry_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_entry_uuid,
            deletion_time: Times::now(),
        });

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

        assert!(!destination_db.deleted_objects.contains(deleted_entry_uuid));
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
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_entry_uuid,
            deletion_time: Times::now(),
        });
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_subgroup_uuid,
            deletion_time: Times::now(),
        });
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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

        assert!(destination_db.deleted_objects.contains(deleted_entry_uuid));
        assert!(destination_db.deleted_objects.contains(deleted_subgroup_uuid));
        assert!(destination_db.deleted_objects.contains(deleted_group_uuid));
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
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_entry_uuid,
            deletion_time: Times::now(),
        });
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_subgroup_uuid,
            deletion_time: Times::now(),
        });
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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

        assert!(destination_db.deleted_objects.contains(deleted_entry_uuid));
        assert!(destination_db.deleted_objects.contains(deleted_subgroup_uuid));
        assert!(!destination_db.deleted_objects.contains(deleted_group_uuid));
    }

    #[test]
    fn test_deleted_group_in_source_modified_in_destination() {
        let mut destination_db = create_test_database();
        let mut source_db = destination_db.clone();

        let deleted_group_uuid = Uuid::new_v4();

        thread::sleep(time::Duration::from_secs(1));
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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

        assert!(!destination_db.deleted_objects.contains(deleted_group_uuid));
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
        source_db.deleted_objects.objects.push(crate::db::DeletedObject {
            uuid: deleted_group_uuid,
            deletion_time: Times::now(),
        });

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

        assert!(!destination_db.deleted_objects.contains(deleted_group_uuid));
        assert!(!destination_db.deleted_objects.contains(new_entry_uuid));
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
                &vec![Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &vec![Uuid::parse_str(GROUP2_ID).unwrap()],
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
            &vec![
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
                &vec![Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &vec![Uuid::parse_str(GROUP2_ID).unwrap()],
                new_location_changed_timestamp,
            )
            .unwrap();

        let entry2 = Group::find_entry(
            &destination_db.root,
            &vec![
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
            &vec![
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
                &vec![Uuid::parse_str(GROUP1_ID).unwrap(), Uuid::parse_str(SUBGROUP1_ID).unwrap()],
                &vec![Uuid::parse_str(GROUP2_ID).unwrap()],
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
                &vec![Uuid::parse_str(GROUP1_ID).unwrap()],
                &vec![Uuid::parse_str(GROUP2_ID).unwrap()],
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
                &vec![Uuid::parse_str(GROUP1_ID).unwrap()],
                &vec![Uuid::parse_str(GROUP2_ID).unwrap()],
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
