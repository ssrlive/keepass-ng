use crate::{
    db::{Entry, Group, Times},
    Result,
};
use std::collections::VecDeque;
use uuid::Uuid;

pub type NodePtr = std::rc::Rc<std::cell::RefCell<dyn Node>>;

#[macro_export]
macro_rules! rc_refcell_node {
    ($e:expr) => {
        std::rc::Rc::new(std::cell::RefCell::new($e)) as NodePtr
    };
}

pub fn node_is_group(group: &NodePtr) -> bool {
    group.borrow().as_any().downcast_ref::<Group>().is_some()
}

pub fn node_is_entry(entry: &NodePtr) -> bool {
    entry.borrow().as_any().downcast_ref::<Entry>().is_some()
}

pub fn group_get_children(group: &NodePtr) -> Option<Vec<NodePtr>> {
    group.borrow().as_any().downcast_ref::<Group>().map(|g| g.get_children())
}

pub fn group_add_child(parent: &NodePtr, child: NodePtr) -> Result<()> {
    parent
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<Group>()
        .ok_or("parent is not a group")?
        .add_child(child);
    Ok(())
}

pub fn group_reset_children(parent: &NodePtr, children: Vec<NodePtr>) -> Result<()> {
    parent
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<Group>()
        .ok_or("parent is not a group")?
        .children = children;
    Ok(())
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

pub fn search_node_by_uuid(root: &NodePtr, id: Uuid) -> Option<NodePtr> {
    NodeIterator::new(root).find(|n| n.borrow().get_uuid() == id)
}

pub fn search_node_by_uuid_with_specific_type<'a, T>(root: &'a NodePtr, id: Uuid) -> Option<NodePtr>
where
    T: 'a + 'static,
{
    NodeIterator::new(root)
        .filter(|n| n.borrow().as_any().downcast_ref::<T>().is_some())
        .find(|n| n.borrow().get_uuid() == id)
}

#[cfg(feature = "serialization")]
pub trait Node: as_any::AsAny + std::fmt::Debug + erased_serde::Serialize {
    fn duplicate(&self) -> NodePtr;
    fn get_uuid(&self) -> Uuid;
    fn get_title(&self) -> Option<&str>;
    fn set_title(&mut self, title: Option<&str>);
    fn get_notes(&self) -> Option<&str>;
    fn set_notes(&mut self, notes: Option<&str>);
    fn get_icon_id(&self) -> Option<usize>;
    fn get_custom_icon_uuid(&self) -> Option<Uuid>;
    fn get_times(&self) -> &Times;

    /// Get a timestamp field by name
    ///
    /// Returning the chrono::NaiveDateTime which does not include timezone
    /// or UTC offset because KeePass clients typically store timestamps
    /// relative to the local time on the machine writing the data without
    /// including accurate UTC offset or timezone information.
    fn get_time(&self, key: &str) -> Option<&chrono::NaiveDateTime>;

    /// Convenience method for getting the time that the entry expires.
    /// This value is usually only meaningful/useful when expires == true
    fn get_expiry_time(&self) -> Option<&chrono::NaiveDateTime>;
}

#[cfg(feature = "serialization")]
erased_serde::serialize_trait_object!(Node);

#[cfg(not(feature = "serialization"))]
pub trait Node: as_any::AsAny + std::fmt::Debug {
    fn duplicate(&self) -> NodePtr;
    fn get_uuid(&self) -> Uuid;
    fn get_title(&self) -> Option<&str>;
    fn set_title(&mut self, title: Option<&str>);
    fn get_notes(&self) -> Option<&str>;
    fn set_notes(&mut self, notes: Option<&str>);
    fn get_icon_id(&self) -> Option<usize>;
    fn get_custom_icon_uuid(&self) -> Option<Uuid>;
    fn get_times(&self) -> &Times;
    fn get_time(&self, key: &str) -> Option<&chrono::NaiveDateTime>;
    fn get_expiry_time(&self) -> Option<&chrono::NaiveDateTime>;
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
            self.queue.extend(children.into_iter());
        }
        Some(next)
    }
}
