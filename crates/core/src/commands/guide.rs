//! Commands for Inkscape guide metadata.

use sivgo_dom::{NamedViewAttrs, NodeId, SodipodiGuideAttrs, SvgDocument, SvgElement, SvgNode};

use crate::command::Command;

/// Adds a `<sodipodi:guide>` under `<sodipodi:namedview>`.
#[derive(Debug)]
pub struct AddGuideCommand {
    root_id: NodeId,
    namedview_id: NodeId,
    guide_id: NodeId,
    created_namedview: bool,
    namedview_index: usize,
    guide_index: usize,
    guide_attrs: SodipodiGuideAttrs,
}

impl AddGuideCommand {
    /// Create a guide-add command for the current document.
    pub fn new(
        doc: &SvgDocument,
        position: (f64, f64),
        normal: (f64, f64),
        label: Option<String>,
    ) -> Option<Self> {
        let root_id = doc.root()?;
        let namedview_id = Self::find_namedview(doc);
        let created_namedview = namedview_id.is_none();
        let namedview_id = namedview_id.unwrap_or_else(NodeId::new);
        let namedview_index = doc.children(root_id).len();
        let guide_index = if created_namedview {
            0
        } else {
            doc.children(namedview_id).len()
        };

        Some(Self {
            root_id,
            namedview_id,
            guide_id: NodeId::new(),
            created_namedview,
            namedview_index,
            guide_index,
            guide_attrs: SodipodiGuideAttrs {
                position: Some(format!("{},{}", position.0, position.1)),
                orientation: Some(format!("{},{}", normal.0, normal.1)),
                inkscape_label: label.filter(|value| !value.trim().is_empty()),
                inkscape_locked: None,
                inkscape_color: None,
            },
        })
    }

    /// The guide id that will be used when the command executes.
    pub fn guide_id(&self) -> NodeId {
        self.guide_id
    }

    fn find_namedview(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        doc.children(root).iter().copied().find(|child| {
            doc.get_node(*child)
                .map(|node| matches!(node.element, SvgElement::SodipodiNamedView(_)))
                .unwrap_or(false)
        })
    }
}

impl Command for AddGuideCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if self.created_namedview && doc.get_node(self.namedview_id).is_none() {
            doc.insert_node(SvgNode::new(
                self.namedview_id,
                SvgElement::SodipodiNamedView(NamedViewAttrs::default()),
            ));
            doc.insert_child_at(self.root_id, self.namedview_id, self.namedview_index);
        }

        if doc.get_node(self.guide_id).is_none() {
            doc.insert_node(SvgNode::new(
                self.guide_id,
                SvgElement::SodipodiGuide(self.guide_attrs.clone()),
            ));
        }
        doc.insert_child_at(self.namedview_id, self.guide_id, self.guide_index);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if doc.get_node(self.guide_id).is_some() {
            doc.remove_node(self.guide_id);
        }
        if self.created_namedview && doc.get_node(self.namedview_id).is_some() {
            doc.remove_node(self.namedview_id);
        }
    }

    fn description(&self) -> &str {
        "Add guide"
    }
}
