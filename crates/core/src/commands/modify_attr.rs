//! Command: modify presentation attributes of a node.

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument};

use crate::command::Command;

/// Replaces the presentation attributes of a node.
#[derive(Debug)]
pub struct ModifyAttributeCommand {
    node_id: NodeId,
    old_attrs: PresentationAttrs,
    new_attrs: PresentationAttrs,
}

impl ModifyAttributeCommand {
    /// Create a command that changes the presentation attributes of `node_id`
    /// from `old_attrs` to `new_attrs`.
    pub fn new(
        node_id: NodeId,
        old_attrs: PresentationAttrs,
        new_attrs: PresentationAttrs,
    ) -> Self {
        Self {
            node_id,
            old_attrs,
            new_attrs,
        }
    }
}

impl Command for ModifyAttributeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.presentation = self.new_attrs.clone();
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.presentation = self.old_attrs.clone();
        }
    }

    fn description(&self) -> &str {
        "Modify attributes"
    }
}
