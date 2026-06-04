//! Command: modify a node's serialized SVG `id` attribute.

use sivgo_dom::{NodeId, SvgDocument};

use crate::command::Command;

/// Changes a node's SVG `id` (`xml_id`) while preserving the editor [`NodeId`].
#[derive(Debug)]
pub struct ModifyXmlIdCommand {
    node_id: NodeId,
    old_xml_id: Option<String>,
    new_xml_id: Option<String>,
}

impl ModifyXmlIdCommand {
    pub fn new(node_id: NodeId, old_xml_id: Option<String>, new_xml_id: Option<String>) -> Self {
        Self {
            node_id,
            old_xml_id,
            new_xml_id,
        }
    }

    fn apply(doc: &mut SvgDocument, node_id: NodeId, xml_id: &Option<String>) {
        if let Some(node) = doc.get_node_mut(node_id) {
            node.xml_id = xml_id.clone();
            doc.notify_attribute_changed(node_id, "id");
        }
    }
}

impl Command for ModifyXmlIdCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.new_xml_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.old_xml_id);
    }

    fn description(&self) -> &str {
        "Modify SVG id"
    }
}
