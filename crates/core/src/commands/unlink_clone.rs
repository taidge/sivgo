//! Command: replace an SVG `<use>` clone with an independent subtree.

use sivgo_dom::{observer::DomEvent, NodeId, SvgDocument, SvgNode};

use crate::command::Command;

/// Replaces one node's current subtree with a prebuilt subtree, preserving the
/// root node id and its position in the parent. Undo restores the exact old
/// subtree.
#[derive(Debug)]
pub struct UnlinkCloneCommand {
    node_id: NodeId,
    old_subtree: Vec<SvgNode>,
    new_subtree: Vec<SvgNode>,
}

impl UnlinkCloneCommand {
    pub fn new(node_id: NodeId, old_subtree: Vec<SvgNode>, new_subtree: Vec<SvgNode>) -> Self {
        Self {
            node_id,
            old_subtree,
            new_subtree,
        }
    }

    fn apply_subtree(doc: &mut SvgDocument, node_id: NodeId, subtree: &[SvgNode]) {
        let Some(root_snapshot) = subtree.first() else {
            return;
        };

        let current_children = doc.children(node_id).to_vec();
        for child_id in current_children {
            doc.remove_node(child_id);
        }

        if let Some(root) = doc.get_node_mut(node_id) {
            let parent = root.parent;
            *root = root_snapshot.clone();
            root.id = node_id;
            root.parent = parent;
        } else {
            doc.insert_node(root_snapshot.clone());
        }

        for node in subtree.iter().skip(1) {
            doc.insert_node(node.clone());
        }

        doc.push_event(DomEvent::NodeModified(node_id));
        if let Some(parent) = doc.parent(node_id) {
            doc.push_event(DomEvent::ChildrenChanged(parent));
        }
        doc.push_event(DomEvent::ChildrenChanged(node_id));
    }
}

impl Command for UnlinkCloneCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::apply_subtree(doc, self.node_id, &self.new_subtree);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::apply_subtree(doc, self.node_id, &self.old_subtree);
    }

    fn description(&self) -> &str {
        "Unlink clone"
    }
}
