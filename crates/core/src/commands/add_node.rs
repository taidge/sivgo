//! Command: add a new node to the document.

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument, SvgElement, SvgNode};

use crate::command::Command;

/// Adds a new element as a child of the specified parent.
#[derive(Debug)]
pub struct AddNodeCommand {
    parent_id: NodeId,
    element: SvgElement,
    presentation: PresentationAttrs,
    insert_index: Option<usize>,
    /// Filled after first execute -- the id of the created node.
    created_id: Option<NodeId>,
}

impl AddNodeCommand {
    /// Create a command that will add `element` as a child of `parent_id`.
    ///
    /// If `insert_index` is `None`, the node is appended at the end.
    pub fn new(
        parent_id: NodeId,
        element: SvgElement,
        presentation: PresentationAttrs,
        insert_index: Option<usize>,
    ) -> Self {
        Self {
            parent_id,
            element,
            presentation,
            insert_index,
            created_id: None,
        }
    }

    /// Return the node id assigned after the first execute.
    pub fn created_id(&self) -> Option<NodeId> {
        self.created_id
    }
}

impl Command for AddNodeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if let Some(id) = self.created_id {
            // Re-execute (redo): re-insert the previously-removed node.
            let mut node = SvgNode::new(id, self.element.clone());
            node.presentation = self.presentation.clone();
            doc.insert_node(node);
            if let Some(idx) = self.insert_index {
                doc.insert_child_at(self.parent_id, id, idx);
            } else {
                doc.append_child(self.parent_id, id);
            }
        } else {
            // First execute: create a fresh node.
            let id = doc.add_node(self.element.clone());
            if let Some(node) = doc.get_node_mut(id) {
                node.presentation = self.presentation.clone();
            }
            if let Some(idx) = self.insert_index {
                doc.insert_child_at(self.parent_id, id, idx);
            } else {
                doc.append_child(self.parent_id, id);
            }
            self.created_id = Some(id);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(id) = self.created_id {
            // Detach from parent but remember the index for redo.
            if let Some((_parent, idx)) = doc.detach_node(id) {
                self.insert_index = Some(idx);
            }
            // We cannot fully "remove_node" because that recursively deletes,
            // but our node has no children, so it is equivalent.  We remove it
            // from the arena directly by using remove_node.
            doc.remove_node(id);
        }
    }

    fn description(&self) -> &str {
        "Add element"
    }
}
