//! Selection set for tracking which nodes are selected in the editor.

use sivgo_dom::{Box3dAttrs, NodeId, SvgDocument, SvgElement};
use sivgo_math::{Rect, Transform2D};

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
    /// This uses element geometry transformed into document space.
    pub fn bounding_box(&self, doc: &SvgDocument) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for &id in &self.selected {
            if let Some(bb) = node_bounding_box(doc, id) {
                result = Some(match result {
                    Some(r) => r.union(bb),
                    None => bb,
                });
            }
        }
        result
    }
}

/// Compute a node's local element bounds transformed into document space.
pub fn node_bounding_box(doc: &SvgDocument, id: NodeId) -> Option<Rect> {
    let node = doc.get_node(id)?;
    let bb = element_bounding_box(&node.element)?;
    Some(node_transform(doc, id).transform_rect(bb))
}

/// Compute the cumulative transform from the document root to a node.
pub fn node_transform(doc: &SvgDocument, id: NodeId) -> Transform2D {
    let mut chain = Vec::new();
    let mut current = Some(id);
    while let Some(node_id) = current {
        chain.push(node_id);
        current = doc.parent(node_id);
    }

    // Walking root to leaf, each transform maps into the space established by
    // everything above it, so it composes *inside* what has accumulated:
    // `local` applies first, the ancestors' transform second. Folding the
    // other way yields leaf-first order, which misplaces any element sitting
    // under a rotated or scaled group.
    chain
        .into_iter()
        .rev()
        .filter_map(|node_id| {
            doc.get_node(node_id)
                .and_then(|node| node.presentation.transform)
        })
        .fold(Transform2D::IDENTITY, |acc, local| local.then(&acc))
}

/// Simple bounding box computation for individual elements.
///
/// The returned bounds are in the element's local coordinate space.
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
        SvgElement::Polyline(a) => point_list_bounding_box(&a.points),
        SvgElement::Polygon(a) => point_list_bounding_box(&a.points),
        SvgElement::Path(a) => {
            let bb = a.d.bounding_box();
            if bb.is_empty() {
                None
            } else {
                Some(bb)
            }
        }
        SvgElement::Image(a) => rect_from_xywh(
            a.x.as_ref().map_or(0.0, |l| l.to_px()),
            a.y.as_ref().map_or(0.0, |l| l.to_px()),
            a.width.as_ref().map_or(0.0, |l| l.to_px()),
            a.height.as_ref().map_or(0.0, |l| l.to_px()),
        ),
        SvgElement::Use(a) => rect_from_xywh(
            a.x.as_ref().map_or(0.0, |l| l.to_px()),
            a.y.as_ref().map_or(0.0, |l| l.to_px()),
            a.width.as_ref().map_or(0.0, |l| l.to_px()),
            a.height.as_ref().map_or(0.0, |l| l.to_px()),
        ),
        SvgElement::Text(a) => {
            let font_size = a.font_size.as_ref().map_or(16.0, |l| l.to_px()).max(1.0);
            let x =
                a.x.first().map_or(0.0, |l| l.to_px()) + a.dx.first().map_or(0.0, |l| l.to_px());
            let y =
                a.y.first().map_or(0.0, |l| l.to_px()) + a.dy.first().map_or(0.0, |l| l.to_px());
            let width =
                (a.text_content.chars().count() as f64 * font_size * 0.6).max(font_size * 0.5);
            Some(Rect::new(x, y - font_size, x + width, y))
        }
        SvgElement::TSpan(a) => {
            let font_size = a.font_size.as_ref().map_or(16.0, |l| l.to_px()).max(1.0);
            let x =
                a.x.first().map_or(0.0, |l| l.to_px()) + a.dx.first().map_or(0.0, |l| l.to_px());
            let y =
                a.y.first().map_or(0.0, |l| l.to_px()) + a.dy.first().map_or(0.0, |l| l.to_px());
            let width =
                (a.text_content.chars().count() as f64 * font_size * 0.6).max(font_size * 0.5);
            Some(Rect::new(x, y - font_size, x + width, y))
        }
        SvgElement::Box3d(a) => box3d_bounding_box(a),
        _ => None,
    }
}

fn box3d_bounding_box(attrs: &Box3dAttrs) -> Option<Rect> {
    let corners = [
        attrs.corner0.as_deref(),
        attrs.corner1.as_deref(),
        attrs.corner2.as_deref(),
        attrs.corner3.as_deref(),
        attrs.corner4.as_deref(),
        attrs.corner5.as_deref(),
        attrs.corner6.as_deref(),
        attrs.corner7.as_deref(),
    ];

    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for corner in corners.into_iter().flatten() {
        let Some((x, y)) = parse_box3d_corner(corner) else {
            continue;
        };
        bounds = Some(match bounds {
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
            None => (x, y, x, y),
        });
    }

    bounds.map(|(min_x, min_y, max_x, max_y)| Rect::new(min_x, min_y, max_x, max_y))
}

fn parse_box3d_corner(value: &str) -> Option<(f64, f64)> {
    let (x, y) = value.trim().split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn rect_from_xywh(x: f64, y: f64, width: f64, height: f64) -> Option<Rect> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Rect::new(x, y, x + width, y + height))
}

fn point_list_bounding_box(points: &[(f64, f64)]) -> Option<Rect> {
    let mut iter = points.iter();
    let (mut min_x, mut min_y) = *iter.next()?;
    let mut max_x = min_x;
    let mut max_y = min_y;

    for &(x, y) in iter {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    Some(Rect::new(min_x, min_y, max_x, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{ImageAttrs, PolylineAttrs, RectAttrs, TextAttrs};

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

    #[test]
    fn element_bounding_box_covers_image_polyline_and_text() {
        let image = SvgElement::Image(ImageAttrs {
            x: Some(len(15.0)),
            y: Some(len(20.0)),
            width: Some(len(30.0)),
            height: Some(len(40.0)),
            ..Default::default()
        });
        assert_eq!(
            element_bounding_box(&image),
            Some(Rect::new(15.0, 20.0, 45.0, 60.0))
        );

        let polyline = SvgElement::Polyline(PolylineAttrs {
            points: vec![(20.0, 5.0), (-2.0, 12.0), (8.0, -4.0)],
        });
        assert_eq!(
            element_bounding_box(&polyline),
            Some(Rect::new(-2.0, -4.0, 20.0, 12.0))
        );

        let text = SvgElement::Text(TextAttrs {
            x: vec![len(5.0)],
            y: vec![len(30.0)],
            font_size: Some(len(14.0)),
            text_content: "Hi".to_string(),
            ..Default::default()
        });
        let bb = element_bounding_box(&text).unwrap();
        assert_eq!(bb.x0, 5.0);
        assert_eq!(bb.y0, 16.0);
        assert_eq!(bb.y1, 30.0);
        assert!(bb.x1 > bb.x0);
    }

    #[test]
    fn selection_bounding_box_accounts_for_node_transform() {
        let (mut doc, ids) = make_doc_with_rects(1);
        let id = ids[0];
        doc.get_node_mut(id).unwrap().presentation.transform =
            Some(Transform2D::translate(25.0, 35.0));

        let mut sel = SelectionSet::new();
        sel.select(id);

        assert_eq!(
            sel.bounding_box(&doc),
            Some(Rect::new(25.0, 35.0, 35.0, 45.0))
        );
    }
}
