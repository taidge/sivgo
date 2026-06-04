//! Command: modify an object's Inkscape label.

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Changes the `inkscape:label` attribute on a node.
#[derive(Debug)]
pub struct ModifyObjectLabelCommand {
    node_id: NodeId,
    old_label: Option<String>,
    new_label: Option<String>,
}

impl ModifyObjectLabelCommand {
    pub fn new(node_id: NodeId, old_label: Option<String>, new_label: Option<String>) -> Self {
        Self {
            node_id,
            old_label,
            new_label,
        }
    }

    fn apply(doc: &mut SvgDocument, node_id: NodeId, label: &Option<String>) {
        if let Some(node) = doc.get_node_mut(node_id) {
            match label {
                Some(value) => {
                    node.attributes
                        .insert("inkscape:label".to_string(), value.clone());
                }
                None => {
                    node.attributes.remove("inkscape:label");
                }
            }
        }
    }
}

impl Command for ModifyObjectLabelCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.new_label);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.old_label);
    }

    fn description(&self) -> &str {
        "Rename object"
    }
}
