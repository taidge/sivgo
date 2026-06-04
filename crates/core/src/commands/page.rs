//! Commands for Inkscape multi-page document metadata.

use sivgo_dom::{InkscapePageAttrs, NamedViewAttrs, NodeId, SvgDocument, SvgElement, SvgNode};

use crate::command::Command;

/// Adds an `<inkscape:page>` under `<sodipodi:namedview>`.
#[derive(Debug)]
pub struct AddPageCommand {
    root_id: NodeId,
    namedview_id: NodeId,
    page_id: NodeId,
    created_namedview: bool,
    namedview_index: usize,
    page_index: usize,
    page_attrs: InkscapePageAttrs,
}

impl AddPageCommand {
    /// Create a page-add command for the current document.
    pub fn new(
        doc: &SvgDocument,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        label: Option<String>,
    ) -> Option<Self> {
        let root_id = doc.root()?;
        let namedview_id = Self::find_namedview(doc);
        let created_namedview = namedview_id.is_none();
        let namedview_id = namedview_id.unwrap_or_else(NodeId::new);
        let namedview_index = doc.children(root_id).len();
        let page_index = if created_namedview {
            0
        } else {
            doc.children(namedview_id).len()
        };

        Some(Self {
            root_id,
            namedview_id,
            page_id: NodeId::new(),
            created_namedview,
            namedview_index,
            page_index,
            page_attrs: InkscapePageAttrs {
                x: Some(x.to_string()),
                y: Some(y.to_string()),
                width: Some(width.max(1.0).to_string()),
                height: Some(height.max(1.0).to_string()),
                inkscape_label: label.filter(|value| !value.trim().is_empty()),
            },
        })
    }

    /// The page id that will be used when the command executes.
    pub fn page_id(&self) -> NodeId {
        self.page_id
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

impl Command for AddPageCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if self.created_namedview && doc.get_node(self.namedview_id).is_none() {
            doc.insert_node(SvgNode::new(
                self.namedview_id,
                SvgElement::SodipodiNamedView(NamedViewAttrs::default()),
            ));
            doc.insert_child_at(self.root_id, self.namedview_id, self.namedview_index);
        }

        if doc.get_node(self.page_id).is_none() {
            doc.insert_node(SvgNode::new(
                self.page_id,
                SvgElement::InkscapePage(self.page_attrs.clone()),
            ));
        }
        doc.insert_child_at(self.namedview_id, self.page_id, self.page_index);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if doc.get_node(self.page_id).is_some() {
            doc.remove_node(self.page_id);
        }
        if self.created_namedview && doc.get_node(self.namedview_id).is_some() {
            doc.remove_node(self.namedview_id);
        }
    }

    fn description(&self) -> &str {
        "Add page"
    }
}

/// Replaces an `<inkscape:page>` attribute set in place.
#[derive(Debug)]
pub struct ModifyPageCommand {
    page_id: NodeId,
    old_attrs: InkscapePageAttrs,
    new_attrs: InkscapePageAttrs,
}

impl ModifyPageCommand {
    pub fn new(
        page_id: NodeId,
        old_attrs: InkscapePageAttrs,
        new_attrs: InkscapePageAttrs,
    ) -> Self {
        Self {
            page_id,
            old_attrs,
            new_attrs,
        }
    }

    fn apply(doc: &mut SvgDocument, page_id: NodeId, attrs: &InkscapePageAttrs) {
        if let Some(node) = doc.get_node_mut(page_id) {
            if let SvgElement::InkscapePage(page_attrs) = &mut node.element {
                *page_attrs = attrs.clone();
            }
        }
    }
}

impl Command for ModifyPageCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.page_id, &self.new_attrs);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::apply(doc, self.page_id, &self.old_attrs);
    }

    fn description(&self) -> &str {
        "Modify page"
    }
}
