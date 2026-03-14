//! Command: toggle the locked state of a node.

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Toggles or sets the `locked` flag on a node.
#[derive(Debug)]
pub struct SetLockCommand {
    node_id: NodeId,
    old_locked: bool,
    new_locked: bool,
}

impl SetLockCommand {
    /// Create a command that sets the locked state of `node_id` to `new_locked`.
    pub fn new(node_id: NodeId, new_locked: bool) -> Self {
        Self {
            node_id,
            old_locked: false,
            new_locked,
        }
    }

    /// Create a command that toggles the locked state of `node_id`.
    pub fn toggle(node_id: NodeId, current_locked: bool) -> Self {
        Self {
            node_id,
            old_locked: current_locked,
            new_locked: !current_locked,
        }
    }
}

impl Command for SetLockCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            self.old_locked = node.locked;
            node.locked = self.new_locked;
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.locked = self.old_locked;
        }
    }

    fn description(&self) -> &str {
        if self.new_locked {
            "Lock object"
        } else {
            "Unlock object"
        }
    }
}
