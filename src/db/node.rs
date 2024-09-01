use crate::{
    db::{iconid::IconId, Entry, Group, Times},
    Result,
};
use std::collections::VecDeque;
use uuid::Uuid;

pub type NodePtr = std::rc::Rc<std::cell::RefCell<dyn Node>>;

#[derive(Debug, Clone)]
pub struct SerializableNodePtr {
    node_ptr: NodePtr,
}

impl PartialEq for SerializableNodePtr {
    fn eq(&self, other: &Self) -> bool {
        node_is_equals_to(&self.node_ptr, &other.node_ptr)
    }
}

impl Eq for SerializableNodePtr {}

#[cfg(feature = "serialization")]
impl serde::ser::Serialize for SerializableNodePtr {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        self.node_ptr.borrow().serialize(serializer)
    }
}

impl From<NodePtr> for SerializableNodePtr {
    fn from(node: NodePtr) -> Self {
        SerializableNodePtr { node_ptr: node }
    }
}

impl From<&NodePtr> for SerializableNodePtr {
    fn from(node: &NodePtr) -> Self {
        SerializableNodePtr { node_ptr: node.clone() }
    }
}

impl From<SerializableNodePtr> for NodePtr {
    fn from(serializable: SerializableNodePtr) -> Self {
        serializable.node_ptr
    }
}

impl From<&SerializableNodePtr> for NodePtr {
    fn from(serializable: &SerializableNodePtr) -> Self {
        serializable.node_ptr.clone()
    }
}

impl AsRef<NodePtr> for SerializableNodePtr {
    fn as_ref(&self) -> &NodePtr {
        &self.node_ptr
    }
}

impl AsMut<NodePtr> for SerializableNodePtr {
    fn as_mut(&mut self) -> &mut NodePtr {
        &mut self.node_ptr
    }
}

impl std::ops::Deref for SerializableNodePtr {
    type Target = NodePtr;

    fn deref(&self) -> &Self::Target {
        &self.node_ptr
    }
}

impl std::ops::DerefMut for SerializableNodePtr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_ptr
    }
}

pub fn rc_refcell_node<T: Node>(e: T) -> NodePtr {
    std::rc::Rc::new(std::cell::RefCell::new(e)) as NodePtr
}

pub fn node_is_group(group: &NodePtr) -> bool {
    with_node::<Group, _, _>(group, |_| true).unwrap_or(false)
}

/// Get a reference to a node if it is of the specified type
/// and call the closure with the reference.
/// Usage:
/// ```no_run
/// use keepass_ng::db::{with_node, Entry, Group, NodePtr, rc_refcell_node};
///
/// let node: NodePtr = rc_refcell_node(Group::new("group"));
/// with_node::<Group, _, _>(&node, |group| {
///     // do something with group
/// });
///
/// with_node::<Entry, _, _>(&node, |entry| {
///     // do something with entry
/// });
/// ```
pub fn with_node<T, F, R>(node: &NodePtr, f: F) -> Option<R>
where
    T: 'static,
    F: FnOnce(&T) -> R,
{
    node.borrow().as_any().downcast_ref::<T>().map(f)
}

/// Get a mutable reference to a node if it is of the specified type
/// and call the closure with the mutable reference.
/// Usage:
/// ```no_run
/// use keepass_ng::db::{with_node_mut, Entry, Group, NodePtr, rc_refcell_node};
///
/// let node: NodePtr = rc_refcell_node(Group::new("group"));
/// with_node_mut::<Group, _, _>(&node, |group| {
///     // do something with group
/// });
///
/// with_node_mut::<Entry, _, _>(&node, |entry| {
///     // do something with entry
/// });
/// ```
pub fn with_node_mut<T, F, R>(node: &NodePtr, f: F) -> Option<R>
where
    T: 'static,
    F: FnOnce(&mut T) -> R,
{
    node.borrow_mut().as_any_mut().downcast_mut::<T>().map(f)
}

pub fn node_is_entry(entry: &NodePtr) -> bool {
    with_node::<Entry, _, _>(entry, |_| true).unwrap_or(false)
}

pub fn group_get_children(group: &NodePtr) -> Option<Vec<NodePtr>> {
    with_node::<Group, _, _>(group, |g| g.get_children())
}

pub fn group_add_child(parent: &NodePtr, child: NodePtr, index: usize) -> Result<()> {
    with_node_mut::<Group, _, _>(parent, |parent| {
        parent.add_child(child, index);
        Ok(())
    })
    .unwrap_or(Err(crate::Error::from("parent is not a group")))?;
    Ok(())
}

pub fn group_reset_children(parent: &NodePtr, children: Vec<NodePtr>) -> Result<()> {
    let uuid = parent.borrow().get_uuid();
    for c in &children {
        c.borrow_mut().set_parent(Some(uuid));
    }
    with_node_mut::<Group, _, _>(parent, |parent| {
        parent.children = children.into_iter().map(|c| c.into()).collect();
        Ok(())
    })
    .unwrap_or(Err(crate::Error::from("parent is not a group")))?;
    Ok(())
}

pub fn group_remove_node_by_uuid(root: &NodePtr, uuid: Uuid) -> crate::Result<NodePtr> {
    let root_uuid = root.borrow().get_uuid();
    if root_uuid == uuid {
        return Err("Cannot remove root node".into());
    }

    let node = search_node_by_uuid(root, uuid).ok_or("Node not found")?;
    let parent_uuid = node.borrow().get_parent().ok_or("Node has no parent")?;
    let err = format!("Parent \"{parent_uuid}\" not found");
    let parent = search_node_by_uuid_with_specific_type::<Group>(root, parent_uuid).ok_or(err)?;
    with_node_mut::<Group, _, _>(&parent, |parent| {
        let err = format!("Node \"{uuid}\" not found in parent");
        let index = parent.children.iter().position(|c| c.borrow().get_uuid() == uuid).ok_or(err)?;
        parent.children.remove(index);
        Ok::<_, crate::Error>(())
    })
    .unwrap_or(Err(crate::Error::from("Not a group")))?;

    Ok(node)
}

pub fn node_is_equals_to(node: &NodePtr, other: &NodePtr) -> bool {
    let node = node.borrow();
    let other = other.borrow();
    let g_node = node.as_any().downcast_ref::<Group>();
    let g_other = other.as_any().downcast_ref::<Group>();
    if let (Some(g_node), Some(g_other)) = (g_node, g_other) {
        return g_node == g_other;
    }
    let e_node = node.as_any().downcast_ref::<Entry>();
    let e_other = other.as_any().downcast_ref::<Entry>();
    if let (Some(e_node), Some(e_other)) = (e_node, e_other) {
        return e_node == e_other;
    } else if let (None, None) = (e_node, e_other) {
        return true;
    }
    false
}

pub fn search_node_by_uuid(root: &NodePtr, uuid: Uuid) -> Option<NodePtr> {
    NodeIterator::new(root).find(|n| n.borrow().get_uuid() == uuid)
}

pub fn search_node_by_uuid_with_specific_type<'a, T>(root: &'a NodePtr, uuid: Uuid) -> Option<NodePtr>
where
    T: 'a + 'static,
{
    NodeIterator::new(root)
        .filter(|n| with_node::<T, _, _>(n, |_| true).is_some())
        .find(|n| n.borrow().get_uuid() == uuid)
}

#[cfg(feature = "serialization")]
pub trait Node: as_any::AsAny + std::fmt::Debug + erased_serde::Serialize {
    fn duplicate(&self) -> NodePtr;
    fn get_uuid(&self) -> Uuid;
    fn set_uuid(&mut self, uuid: Uuid);
    fn get_title(&self) -> Option<&str>;
    fn set_title(&mut self, title: Option<&str>);
    fn get_notes(&self) -> Option<&str>;
    fn set_notes(&mut self, notes: Option<&str>);
    fn get_icon_id(&self) -> Option<IconId>;
    fn set_icon_id(&mut self, icon_id: Option<IconId>);
    fn get_custom_icon_uuid(&self) -> Option<Uuid>;

    /// Get a timestamp field by name
    ///
    /// Returning the `NaiveDateTime` which does not include timezone
    /// or UTC offset because `KeePass` clients typically store timestamps
    /// relative to the local time on the machine writing the data without
    /// including accurate UTC offset or timezone information.
    fn get_times(&self) -> &Times;
    fn get_times_mut(&mut self) -> &mut Times;

    fn get_parent(&self) -> Option<Uuid>;
    fn set_parent(&mut self, parent: Option<Uuid>);
}

#[cfg(feature = "serialization")]
erased_serde::serialize_trait_object!(Node);

#[cfg(not(feature = "serialization"))]
pub trait Node: as_any::AsAny + std::fmt::Debug {
    fn duplicate(&self) -> NodePtr;
    fn get_uuid(&self) -> Uuid;
    fn set_uuid(&mut self, uuid: Uuid);
    fn get_title(&self) -> Option<&str>;
    fn set_title(&mut self, title: Option<&str>);
    fn get_notes(&self) -> Option<&str>;
    fn set_notes(&mut self, notes: Option<&str>);
    fn get_icon_id(&self) -> Option<IconId>;
    fn set_icon_id(&mut self, icon_id: Option<IconId>);
    fn get_custom_icon_uuid(&self) -> Option<Uuid>;
    fn get_times(&self) -> &Times;
    fn get_times_mut(&mut self) -> &mut Times;
    fn get_parent(&self) -> Option<Uuid>;
    fn set_parent(&mut self, parent: Option<Uuid>);
}

pub struct NodeIterator {
    queue: VecDeque<NodePtr>,
}

impl NodeIterator {
    pub fn new(root: &NodePtr) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(root.clone());
        Self { queue }
    }
}

impl Iterator for NodeIterator {
    type Item = NodePtr;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.queue.pop_front()?;
        if let Some(children) = group_get_children(&next) {
            self.queue.extend(children);
        }
        Some(next)
    }
}
