//! Command: toggle the hidden state of a node.

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Toggles or sets the `hidden` flag on a node.
#[derive(Debug)]
pub struct SetVisibilityCommand {
    node_id: NodeId,
    old_hidden: bool,
    new_hidden: bool,
}

impl SetVisibilityCommand {
    /// Create a command that sets the hidden state of `node_id` to `new_hidden`.
    pub fn new(node_id: NodeId, new_hidden: bool) -> Self {
        Self {
            node_id,
            old_hidden: false,
            new_hidden,
        }
    }

    /// Create a command that toggles the hidden state of `node_id`.
    pub fn toggle(node_id: NodeId, current_hidden: bool) -> Self {
        Self {
            node_id,
            old_hidden: current_hidden,
            new_hidden: !current_hidden,
        }
    }
}

impl Command for SetVisibilityCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            self.old_hidden = node.hidden;
            node.hidden = self.new_hidden;
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.hidden = self.old_hidden;
        }
    }

    fn description(&self) -> &str {
        if self.new_hidden {
            "Hide object"
        } else {
            "Show object"
        }
    }
}
