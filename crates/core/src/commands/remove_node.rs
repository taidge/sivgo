//! Command: remove a node from the document.

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument, SvgElement, SvgNode};

use crate::command::Command;

/// Removes a node from the document (storing it for undo).
#[derive(Debug)]
pub struct RemoveNodeCommand {
    node_id: NodeId,
    /// Saved state for undo.
    parent_id: Option<NodeId>,
    index: usize,
    element: Option<SvgElement>,
    presentation: Option<PresentationAttrs>,
    xml_id: Option<String>,
    children: Vec<NodeId>,
    extra_attrs: Option<std::collections::HashMap<String, String>>,
}

impl RemoveNodeCommand {
    /// Create a command that will remove the node with `node_id`.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            parent_id: None,
            index: 0,
            element: None,
            presentation: None,
            xml_id: None,
            children: Vec::new(),
            extra_attrs: None,
        }
    }
}

impl Command for RemoveNodeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        // Save the node data before removing.
        if let Some(node) = doc.get_node(self.node_id) {
            self.element = Some(node.element.clone());
            self.presentation = Some(node.presentation.clone());
            self.xml_id = node.xml_id.clone();
            self.children = node.children.clone();
            self.extra_attrs = Some(node.attributes.clone());
        }
        // Detach from parent.
        if let Some((parent, idx)) = doc.detach_node(self.node_id) {
            self.parent_id = Some(parent);
            self.index = idx;
        }
        // Remove from the arena.
        doc.remove_node(self.node_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Re-create the node.
        if let Some(element) = self.element.take() {
            let mut node = SvgNode::new(self.node_id, element.clone());
            if let Some(pres) = &self.presentation {
                node.presentation = pres.clone();
            }
            node.xml_id = self.xml_id.clone();
            node.children = self.children.clone();
            if let Some(attrs) = &self.extra_attrs {
                node.attributes = attrs.clone();
            }
            doc.insert_node(node);

            // Re-attach to parent.
            if let Some(parent_id) = self.parent_id {
                doc.insert_child_at(parent_id, self.node_id, self.index);
            }

            // Re-attach children's parent pointers.
            for &child_id in &self.children {
                if let Some(child) = doc.get_node_mut(child_id) {
                    child.parent = Some(self.node_id);
                }
            }

            // Store element back for potential redo.
            self.element = Some(element);
        }
    }

    fn description(&self) -> &str {
        "Delete element"
    }
}
