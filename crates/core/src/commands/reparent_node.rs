//! Command: reparent or reorder an existing node.

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Moves an existing node to a new parent and insertion index.
#[derive(Debug)]
pub struct ReparentNodeCommand {
    node_id: NodeId,
    new_parent: NodeId,
    new_index: Option<usize>,
    old_parent: Option<NodeId>,
    old_index: usize,
}

impl ReparentNodeCommand {
    pub fn new(node_id: NodeId, new_parent: NodeId, new_index: Option<usize>) -> Self {
        Self {
            node_id,
            new_parent,
            new_index,
            old_parent: None,
            old_index: 0,
        }
    }
}

impl Command for ReparentNodeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if doc.get_node(self.node_id).is_none() || doc.get_node(self.new_parent).is_none() {
            return;
        }

        if let Some((old_parent, old_index)) = doc.detach_node(self.node_id) {
            self.old_parent = Some(old_parent);
            self.old_index = old_index;
        }

        let mut index = self
            .new_index
            .unwrap_or_else(|| doc.children(self.new_parent).len());
        if self.old_parent == Some(self.new_parent) && index > self.old_index {
            index = index.saturating_sub(1);
        }
        doc.insert_child_at(self.new_parent, self.node_id, index);
        self.new_index = Some(index);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        let Some(old_parent) = self.old_parent else {
            return;
        };
        if doc.get_node(self.node_id).is_none() || doc.get_node(old_parent).is_none() {
            return;
        }

        doc.detach_node(self.node_id);
        doc.insert_child_at(old_parent, self.node_id, self.old_index);
    }

    fn description(&self) -> &str {
        "Reparent object"
    }
}
