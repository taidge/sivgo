//! Command wrapper: route newly inserted root-level graphical nodes to a layer.

use std::collections::HashSet;

use sivgo_dom::{NodeId, SvgDocument, SvgElement};

use crate::command::Command;

/// Wraps a tool command and moves any newly created root-level graphical nodes
/// into the active layer after the wrapped command executes.
#[derive(Debug)]
pub struct RouteToActiveLayerCommand {
    inner: Box<dyn Command>,
    layer_id: NodeId,
    moved_nodes: Vec<(NodeId, usize)>,
}

impl RouteToActiveLayerCommand {
    pub fn new(inner: Box<dyn Command>, layer_id: NodeId) -> Self {
        Self {
            inner,
            layer_id,
            moved_nodes: Vec::new(),
        }
    }

    fn is_graphical(element: &SvgElement) -> bool {
        matches!(
            element,
            SvgElement::Rect(_)
                | SvgElement::Circle(_)
                | SvgElement::Ellipse(_)
                | SvgElement::Line(_)
                | SvgElement::Polyline(_)
                | SvgElement::Polygon(_)
                | SvgElement::Path(_)
                | SvgElement::Group(_)
                | SvgElement::Text(_)
                | SvgElement::FlowRoot(_)
                | SvgElement::Image(_)
                | SvgElement::Use(_)
                | SvgElement::Symbol(_)
                | SvgElement::Box3d(_)
        )
    }

    fn route_new_root_children(&mut self, doc: &mut SvgDocument, before: HashSet<NodeId>) {
        let Some(root) = doc.root() else {
            return;
        };
        if doc.get_node(self.layer_id).is_none() {
            return;
        }

        let after: Vec<NodeId> = doc.children(root).to_vec();
        self.moved_nodes.clear();
        for (index, child_id) in after.iter().copied().enumerate() {
            if before.contains(&child_id) || child_id == self.layer_id {
                continue;
            }
            let Some(node) = doc.get_node(child_id) else {
                continue;
            };
            if !Self::is_graphical(&node.element) {
                continue;
            }
            self.moved_nodes.push((child_id, index));
        }

        for (child_id, _) in &self.moved_nodes {
            doc.detach_node(*child_id);
            doc.append_child(self.layer_id, *child_id);
        }
    }
}

impl Command for RouteToActiveLayerCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let before: HashSet<NodeId> = doc
            .root()
            .map(|root| doc.children(root).iter().copied().collect())
            .unwrap_or_default();

        self.inner.execute(doc);
        self.route_new_root_children(doc, before);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(root) = doc.root() {
            for (node_id, index) in self.moved_nodes.iter().rev() {
                if doc.get_node(*node_id).is_some() {
                    doc.detach_node(*node_id);
                    doc.insert_child_at(root, *node_id, *index);
                }
            }
        }

        self.inner.undo(doc);
    }

    fn description(&self) -> &str {
        self.inner.description()
    }
}
