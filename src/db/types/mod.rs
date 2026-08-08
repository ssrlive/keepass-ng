pub(crate) mod attachment;
pub(crate) mod autotype;
pub(crate) mod color;
pub(crate) mod custom_data;
pub(crate) mod entry;
pub(crate) mod group;
pub(crate) mod history;
pub(crate) mod meta;
pub(crate) mod node;
pub(crate) mod times;
pub(crate) mod value;

pub use attachment::Attachment;
pub use autotype::{AutoType, AutoTypeAssociation};
pub use color::{Color, ParseColorError};
pub use custom_data::{CustomDataItem, CustomDataValue};
pub use entry::Entry;
pub use group::Group;
pub use history::History;
pub use meta::{MemoryProtection, Meta};
pub use node::{
    Node, NodeIterator, NodePtr, SerializableNodePtr, group_add_child, group_get_children, group_remove_node_by_uuid, node_is_entry,
    node_is_equals_to, node_is_group, rc_refcell_node, search_node_by_uuid, search_node_by_uuid_with_specific_type, with_node,
    with_node_mut,
};
pub use times::Times;
pub use value::Value;

use crate::{config::DatabaseConfig, db::IconId};
use std::collections::HashMap;

use chrono::NaiveDateTime;
use uuid::Uuid;

/// A decrypted `KeePass` database
#[derive(Debug)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Database {
    /// Configuration settings of the database such as encryption and compression algorithms
    pub config: DatabaseConfig,

    /// Root node of the KeePass database
    pub root: SerializableNodePtr,

    /// References to previously-deleted objects and their deletion times.
    pub deleted_objects: HashMap<Uuid, Option<NaiveDateTime>>,

    /// Metadata of the KeePass database
    pub meta: Meta,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            root: self.root.borrow().duplicate().into(),
            deleted_objects: self.deleted_objects.clone(),
            meta: self.meta.clone(),
        }
    }
}

impl PartialEq for Database {
    fn eq(&self, other: &Self) -> bool {
        self.config == other.config
            && self.deleted_objects == other.deleted_objects
            && self.meta == other.meta
            && node_is_equals_to(&self.root, &other.root)
    }
}

impl Eq for Database {}

impl Database {
    /// Create a new, empty database
    pub fn new(config: DatabaseConfig) -> Database {
        Self {
            config,
            root: rc_refcell_node(Group::new("Root")).into(),
            deleted_objects: Default::default(),
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
            self.deleted_objects.insert(uuid, Some(Times::now()));
            return Ok(node);
        }
        let node_in_recycle_bin = self.node_is_in_recycle_bin(uuid);
        let recycle_bin = self.get_recycle_bin().ok_or("").or_else(|_| self.create_recycle_bin())?;
        let recycle_bin_uuid = recycle_bin.borrow().get_uuid();
        // This can remove the recycle bin itself, or node in the recycle bin, or node not in the recycle bin
        let node = group_remove_node_by_uuid(&self.root, uuid)?;
        self.deleted_objects.insert(uuid, Some(Times::now()));
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
}
