use crate::db::{entry::Entry, group::Group};
use std::collections::VecDeque;
use uuid::Uuid;

/// An owned node in the database tree structure which can either be an Entry or Group
#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub enum Node {
    Group(Group),
    Entry(Entry),
}

impl Node {
    pub fn as_ref(&self) -> NodeRef<'_> {
        self.into()
    }

    pub fn as_mut(&mut self) -> NodeRefMut<'_> {
        self.into()
    }

    pub fn uuid(&self) -> &Uuid {
        match self {
            Node::Group(g) => g.get_uuid(),
            Node::Entry(e) => e.get_uuid(),
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self, Node::Group(_))
    }

    pub fn is_entry(&self) -> bool {
        matches!(self, Node::Entry(_))
    }

    pub fn title(&self) -> Option<&str> {
        match self {
            Node::Group(g) => Some(g.get_name()),
            Node::Entry(e) => e.get_title(),
        }
    }

    pub fn get_children(&self) -> Option<&[Node]> {
        match self {
            Node::Group(g) => Some(g.get_children()),
            Node::Entry(_) => None,
        }
    }
}

impl From<Entry> for Node {
    fn from(e: Entry) -> Self {
        Node::Entry(e)
    }
}

impl From<Group> for Node {
    fn from(g: Group) -> Self {
        Node::Group(g)
    }
}

/// A shared reference to a node in the database tree structure which can either point to an Entry or a Group
#[derive(Debug, Eq, PartialEq)]
pub enum NodeRef<'a> {
    Group(&'a Group),
    Entry(&'a Entry),
}

impl NodeRef<'_> {
    pub fn uuid(&self) -> &Uuid {
        match self {
            NodeRef::Group(g) => &g.uuid,
            NodeRef::Entry(e) => e.get_uuid(),
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self, NodeRef::Group(_))
    }

    pub fn is_entry(&self) -> bool {
        matches!(self, NodeRef::Entry(_))
    }

    pub fn title(&self) -> Option<&str> {
        match self {
            NodeRef::Group(g) => Some(g.name.as_str()),
            NodeRef::Entry(e) => e.get_title(),
        }
    }

    pub fn get_children(&self) -> Option<Vec<NodeRef<'_>>> {
        match self {
            NodeRef::Group(g) => Some(
                g.get_children()
                    .iter()
                    .map(|n| n.into())
                    .collect::<Vec<_>>(),
            ),
            NodeRef::Entry(_) => None,
        }
    }
}

impl<'a> std::convert::From<&'a Node> for NodeRef<'a> {
    fn from(n: &'a Node) -> Self {
        match n {
            Node::Group(g) => NodeRef::Group(g),
            Node::Entry(e) => NodeRef::Entry(e),
        }
    }
}

/// An exclusive mutable reference to a node in the database tree structure which can either point to an Entry or a Group
#[derive(Debug, Eq, PartialEq)]
pub enum NodeRefMut<'a> {
    Group(&'a mut Group),
    Entry(&'a mut Entry),
}

impl NodeRefMut<'_> {
    pub fn uuid(&self) -> &Uuid {
        match self {
            NodeRefMut::Group(g) => &g.uuid,
            NodeRefMut::Entry(e) => e.get_uuid(),
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self, NodeRefMut::Group(_))
    }

    pub fn is_entry(&self) -> bool {
        matches!(self, NodeRefMut::Entry(_))
    }

    pub fn title(&self) -> Option<&str> {
        match self {
            NodeRefMut::Group(g) => Some(g.name.as_str()),
            NodeRefMut::Entry(e) => e.get_title(),
        }
    }

    pub fn get_children(&mut self) -> Option<Vec<NodeRefMut<'_>>> {
        match self {
            NodeRefMut::Group(g) => Some(
                g.get_children_mut()
                    .iter_mut()
                    .map(|n| n.into())
                    .collect::<Vec<_>>(),
            ),
            NodeRefMut::Entry(_) => None,
        }
    }
}

impl<'a> std::convert::From<&'a mut Node> for NodeRefMut<'a> {
    fn from(n: &'a mut Node) -> Self {
        match n {
            Node::Group(g) => NodeRefMut::Group(g),
            Node::Entry(e) => NodeRefMut::Entry(e),
        }
    }
}

/// An iterator over Group and Entry references
pub struct NodeIter<'a> {
    queue: VecDeque<NodeRef<'a>>,
}

impl<'a> NodeIter<'a> {
    pub fn new(queue: VecDeque<NodeRef<'a>>) -> Self {
        Self { queue }
    }
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<NodeRef<'a>> {
        let head = self.queue.pop_front()?;

        if let NodeRef::Group(g) = head {
            self.queue.extend(g.children.iter().map(|n| n.into()))
        }

        Some(head)
    }
}
