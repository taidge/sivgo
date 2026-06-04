//! Command: replace an element and its presentation attributes in place.

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument, SvgElement};

use crate::command::Command;

/// Replaces a node's typed SVG element and presentation attributes while
/// preserving its node id, parent, children, and metadata.
#[derive(Debug)]
pub struct ModifyElementCommand {
    node_id: NodeId,
    old_element: SvgElement,
    new_element: SvgElement,
    old_presentation: PresentationAttrs,
    new_presentation: PresentationAttrs,
}

impl ModifyElementCommand {
    /// Create a command that changes a node's element and presentation.
    pub fn new(
        node_id: NodeId,
        old_element: SvgElement,
        new_element: SvgElement,
        old_presentation: PresentationAttrs,
        new_presentation: PresentationAttrs,
    ) -> Self {
        Self {
            node_id,
            old_element,
            new_element,
            old_presentation,
            new_presentation,
        }
    }

    fn apply(
        doc: &mut SvgDocument,
        node_id: NodeId,
        element: &SvgElement,
        pres: &PresentationAttrs,
    ) {
        if let Some(node) = doc.get_node_mut(node_id) {
            node.element = element.clone();
            node.presentation = pres.clone();
        }
    }
}

impl Command for ModifyElementCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.new_element, &self.new_presentation);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.node_id, &self.old_element, &self.old_presentation);
    }

    fn description(&self) -> &str {
        "Modify element"
    }
}
