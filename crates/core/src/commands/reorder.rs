//! Command: change z-order (raise / lower).

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Direction of a reorder operation.
#[derive(Debug, Clone, Copy)]
pub enum ReorderDirection {
    /// Move up one position (toward the end of the children list = in front).
    Raise,
    /// Move down one position (toward the start of the children list = behind).
    Lower,
    /// Move to the very top.
    RaiseToTop,
    /// Move to the very bottom.
    LowerToBottom,
}

/// Reorders a node within its parent's children list.
#[derive(Debug)]
pub struct ReorderCommand {
    node_id: NodeId,
    direction: ReorderDirection,
    /// Old index before the reorder (for undo).
    old_index: usize,
}

impl ReorderCommand {
    pub fn new(node_id: NodeId, direction: ReorderDirection) -> Self {
        Self {
            node_id,
            direction,
            old_index: 0,
        }
    }
}

impl Command for ReorderCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let parent_id = match doc.parent(self.node_id) {
            Some(p) => p,
            None => return,
        };
        if let Some(idx) = doc.child_index(self.node_id) {
            self.old_index = idx;
        }

        if let Some((parent, _)) = doc.detach_node(self.node_id) {
            let len = doc.children(parent).len();
            let new_index = match self.direction {
                ReorderDirection::Raise => (self.old_index + 1).min(len),
                ReorderDirection::Lower => self.old_index.saturating_sub(1),
                ReorderDirection::RaiseToTop => len,
                ReorderDirection::LowerToBottom => 0,
            };
            doc.insert_child_at(parent_id, self.node_id, new_index);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        let parent_id = match doc.parent(self.node_id) {
            Some(p) => p,
            None => return,
        };
        if let Some(_) = doc.detach_node(self.node_id) {
            doc.insert_child_at(parent_id, self.node_id, self.old_index);
        }
    }

    fn description(&self) -> &str {
        match self.direction {
            ReorderDirection::Raise => "Raise",
            ReorderDirection::Lower => "Lower",
            ReorderDirection::RaiseToTop => "Raise to top",
            ReorderDirection::LowerToBottom => "Lower to bottom",
        }
    }
}
