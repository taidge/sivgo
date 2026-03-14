//! Command: modify the path data (`d` attribute) of a path element.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::BezPath;

use crate::command::Command;

/// Replaces the `d` attribute of a `<path>` element.
#[derive(Debug)]
pub struct ModifyPathCommand {
    node_id: NodeId,
    old_path: BezPath,
    new_path: BezPath,
}

impl ModifyPathCommand {
    /// Create a command that changes the path data of `node_id`
    /// from `old_path` to `new_path`.
    pub fn new(node_id: NodeId, old_path: BezPath, new_path: BezPath) -> Self {
        Self {
            node_id,
            old_path,
            new_path,
        }
    }

    fn set_path(doc: &mut SvgDocument, node_id: NodeId, path: &BezPath) {
        if let Some(node) = doc.get_node_mut(node_id) {
            if let SvgElement::Path(ref mut attrs) = node.element {
                attrs.d = path.clone();
            }
        }
    }
}

impl Command for ModifyPathCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::set_path(doc, self.node_id, &self.new_path);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::set_path(doc, self.node_id, &self.old_path);
    }

    fn description(&self) -> &str {
        "Modify path"
    }
}
