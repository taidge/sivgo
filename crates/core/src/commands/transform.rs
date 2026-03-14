//! Command: change an element's transform.

use sivgo_dom::{NodeId, SvgDocument};
use sivgo_math::Transform2D;

use crate::command::Command;

/// Changes the transform on a single node.
#[derive(Debug)]
pub struct TransformCommand {
    node_id: NodeId,
    old_transform: Option<Transform2D>,
    new_transform: Option<Transform2D>,
}

impl TransformCommand {
    pub fn new(
        node_id: NodeId,
        old_transform: Option<Transform2D>,
        new_transform: Option<Transform2D>,
    ) -> Self {
        Self {
            node_id,
            old_transform,
            new_transform,
        }
    }
}

impl Command for TransformCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.presentation.transform = self.new_transform;
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.presentation.transform = self.old_transform;
        }
    }

    fn description(&self) -> &str {
        "Transform"
    }
}
