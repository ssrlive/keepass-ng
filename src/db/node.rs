use crate::db::{Entry, Group, Times};
use std::collections::VecDeque;
use uuid::Uuid;

pub type NodePtr = std::rc::Rc<std::cell::RefCell<dyn Node>>;

#[macro_export]
macro_rules! rc_refcell {
    ($e:expr) => {
        std::rc::Rc::new(std::cell::RefCell::new($e))
    };
}

pub fn node_is_group(node: &NodePtr) -> bool {
    node.borrow().as_any().downcast_ref::<Group>().is_some()
}

pub fn node_is_entry(node: &NodePtr) -> bool {
    node.borrow().as_any().downcast_ref::<Entry>().is_some()
}

pub fn node_add_child(parent: &NodePtr, child: NodePtr) -> Option<()> {
    parent
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<Group>()
        .map(|g| {
            g.add_child(child);
        })
}

pub fn is_nodes_equal(a: &NodePtr, b: &NodePtr) -> bool {
    let a = a.borrow();
    let b = b.borrow();
    let g_a = a.as_any().downcast_ref::<Group>();
    let g_b = b.as_any().downcast_ref::<Group>();
    if let (Some(g_a), Some(g_b)) = (g_a, g_b) {
        return g_a == g_b;
    }
    let e_a = a.as_any().downcast_ref::<Entry>();
    let e_b = b.as_any().downcast_ref::<Entry>();
    if let (Some(e_a), Some(e_b)) = (e_a, e_b) {
        return e_a == e_b;
    }
    false
}

pub trait Node: as_any::AsAny + std::fmt::Debug + erased_serde::Serialize {
    fn duplicate(&self) -> NodePtr;
    fn get_uuid(&self) -> Uuid;
    fn get_title(&self) -> Option<&str>;
    fn get_notes(&self) -> Option<&str>;
    fn get_icon_id(&self) -> Option<usize>;
    fn get_custom_icon_uuid(&self) -> Option<&Uuid>;
    fn get_children(&self) -> Option<Vec<NodePtr>>;
    fn get_times(&self) -> &Times;
}

erased_serde::serialize_trait_object!(Node);

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
        if let Some(children) = next.borrow().get_children() {
            self.queue.extend(children.into_iter());
        }
        Some(next)
    }
}
