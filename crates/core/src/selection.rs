//! Selection set for tracking which nodes are selected in the editor.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::Rect;

/// An ordered set of selected node ids.
#[derive(Debug, Clone, Default)]
pub struct SelectionSet {
    selected: Vec<NodeId>,
}

impl SelectionSet {
    /// Create an empty selection.
    pub fn new() -> Self {
        Self {
            selected: Vec::new(),
        }
    }

    /// Replace the entire selection with a single node.
    pub fn select(&mut self, id: NodeId) {
        self.selected.clear();
        self.selected.push(id);
    }

    /// Add a node to the selection (if not already present).
    pub fn add(&mut self, id: NodeId) {
        if !self.selected.contains(&id) {
            self.selected.push(id);
        }
    }

    /// Remove a node from the selection.
    pub fn remove(&mut self, id: NodeId) {
        self.selected.retain(|&n| n != id);
    }

    /// Toggle membership: add if absent, remove if present.
    pub fn toggle(&mut self, id: NodeId) {
        if self.is_selected(id) {
            self.remove(id);
        } else {
            self.add(id);
        }
    }

    /// Clear the entire selection.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Check whether a node is selected.
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected.contains(&id)
    }

    /// The selected node ids in order.
    pub fn selected(&self) -> &[NodeId] {
        &self.selected
    }

    /// Whether the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Number of selected nodes.
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// The primary (first) selected node.
    pub fn primary(&self) -> Option<NodeId> {
        self.selected.first().copied()
    }

    /// Select the next object in document order (Tab-cycle).
    ///
    /// If nothing is selected the first selectable child of the root element is
    /// picked.  If the currently selected node is the last sibling, wraps to
    /// the first.
    pub fn select_next(&mut self, doc: &SvgDocument) {
        let selectables = Self::collect_selectables(doc);
        if selectables.is_empty() {
            return;
        }
        let current = self.primary();
        let next = match current {
            Some(cur) => {
                if let Some(pos) = selectables.iter().position(|&id| id == cur) {
                    selectables[(pos + 1) % selectables.len()]
                } else {
                    selectables[0]
                }
            }
            None => selectables[0],
        };
        self.select(next);
    }

    /// Select the previous object in document order (Shift+Tab-cycle).
    pub fn select_prev(&mut self, doc: &SvgDocument) {
        let selectables = Self::collect_selectables(doc);
        if selectables.is_empty() {
            return;
        }
        let current = self.primary();
        let prev = match current {
            Some(cur) => {
                if let Some(pos) = selectables.iter().position(|&id| id == cur) {
                    if pos == 0 {
                        selectables[selectables.len() - 1]
                    } else {
                        selectables[pos - 1]
                    }
                } else {
                    selectables[selectables.len() - 1]
                }
            }
            None => selectables[selectables.len() - 1],
        };
        self.select(prev);
    }

    /// Collect all selectable node ids in document (depth-first) order.
    fn collect_selectables(doc: &SvgDocument) -> Vec<NodeId> {
        let mut result = Vec::new();
        if let Some(root) = doc.root() {
            Self::collect_selectables_recurse(doc, root, &mut result);
        }
        result
    }

    fn collect_selectables_recurse(doc: &SvgDocument, node_id: NodeId, out: &mut Vec<NodeId>) {
        if let Some(node) = doc.get_node(node_id) {
            match &node.element {
                SvgElement::Rect(_)
                | SvgElement::Circle(_)
                | SvgElement::Ellipse(_)
                | SvgElement::Line(_)
                | SvgElement::Polyline(_)
                | SvgElement::Polygon(_)
                | SvgElement::Path(_)
                | SvgElement::Text(_)
                | SvgElement::Image(_) => {
                    out.push(node_id);
                }
                SvgElement::Group(_) => {
                    out.push(node_id);
                    for &child in &node.children {
                        Self::collect_selectables_recurse(doc, child, out);
                    }
                }
                SvgElement::Svg(_) => {
                    // Don't push root svg, just recurse into children
                    for &child in &node.children {
                        Self::collect_selectables_recurse(doc, child, out);
                    }
                }
                _ => {
                    // Defs, gradients, filters, etc. are not selectable
                    // but recurse into groups inside them
                }
            }
        }
    }

    /// Compute the union bounding box of all selected elements.
    ///
    /// This is a simplified version that uses element geometry directly.
    pub fn bounding_box(&self, doc: &SvgDocument) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for &id in &self.selected {
            if let Some(node) = doc.get_node(id) {
                let bb = element_bounding_box(&node.element);
                if let Some(bb) = bb {
                    result = Some(match result {
                        Some(r) => r.union(bb),
                        None => bb,
                    });
                }
            }
        }
        result
    }
}

/// Simple bounding box computation for individual elements.
///
/// This is a simplified version for Phase 3 -- a full implementation would
/// incorporate transforms and descend into groups.
pub fn element_bounding_box(element: &SvgElement) -> Option<Rect> {
    match element {
        SvgElement::Rect(a) => Some(Rect::new(
            a.x.value,
            a.y.value,
            a.x.value + a.width.value,
            a.y.value + a.height.value,
        )),
        SvgElement::Circle(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let r = a.r.value;
            Some(Rect::new(cx - r, cy - r, cx + r, cy + r))
        }
        SvgElement::Ellipse(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let rx = a.rx.value;
            let ry = a.ry.value;
            Some(Rect::new(cx - rx, cy - ry, cx + rx, cy + ry))
        }
        SvgElement::Line(a) => {
            let x0 = a.x1.value.min(a.x2.value);
            let y0 = a.y1.value.min(a.y2.value);
            let x1 = a.x1.value.max(a.x2.value);
            let y1 = a.y1.value.max(a.y2.value);
            Some(Rect::new(x0, y0, x1, y1))
        }
        SvgElement::Path(a) => {
            let bb = a.d.bounding_box();
            if bb.is_empty() {
                None
            } else {
                Some(bb)
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{RectAttrs, SvgNode};

    fn len(v: f64) -> SvgLength {
        SvgLength {
            value: v,
            unit: LengthUnit::Px,
        }
    }

    fn make_doc_with_rects(n: usize) -> (SvgDocument, Vec<NodeId>) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(sivgo_dom::SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        let root = doc.root().unwrap();
        let mut ids = Vec::new();
        for i in 0..n {
            let id = doc.add_node(SvgElement::Rect(RectAttrs {
                x: len(i as f64 * 20.0),
                y: len(0.0),
                width: len(10.0),
                height: len(10.0),
                ..Default::default()
            }));
            doc.append_child(root, id);
            ids.push(id);
        }
        (doc, ids)
    }

    #[test]
    fn tab_cycle_forward() {
        let (doc, ids) = make_doc_with_rects(3);
        let mut sel = SelectionSet::new();
        sel.select_next(&doc);
        assert_eq!(sel.primary(), Some(ids[0]));
        sel.select_next(&doc);
        assert_eq!(sel.primary(), Some(ids[1]));
        sel.select_next(&doc);
        assert_eq!(sel.primary(), Some(ids[2]));
        // Wrap around
        sel.select_next(&doc);
        assert_eq!(sel.primary(), Some(ids[0]));
    }

    #[test]
    fn tab_cycle_backward() {
        let (doc, ids) = make_doc_with_rects(3);
        let mut sel = SelectionSet::new();
        sel.select_prev(&doc);
        assert_eq!(sel.primary(), Some(ids[2]));
        sel.select_prev(&doc);
        assert_eq!(sel.primary(), Some(ids[1]));
        sel.select_prev(&doc);
        assert_eq!(sel.primary(), Some(ids[0]));
        // Wrap around
        sel.select_prev(&doc);
        assert_eq!(sel.primary(), Some(ids[2]));
    }

    #[test]
    fn tab_cycle_empty_doc() {
        let doc = SvgDocument::new();
        let mut sel = SelectionSet::new();
        sel.select_next(&doc);
        assert!(sel.is_empty());
    }
}
