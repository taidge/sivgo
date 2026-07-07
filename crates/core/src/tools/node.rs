//! Node tool: edit individual path nodes (anchor points) and control handles.
//!
//! Allows clicking and dragging path nodes, dragging cubic Bezier control
//! handles, deleting nodes, and toggling segment types between line and curve.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::{BezPath, PathSegment, Point};

use crate::commands::ModifyPathCommand;
use crate::input::{Key, KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// Helper types
// ---------------------------------------------------------------------------

/// What kind of handle the user grabbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleKind {
    /// An anchor point (the main on-curve point of a segment).
    Anchor,
    /// The first control point of a CurveTo segment.
    ControlPoint1,
    /// The second control point of a CurveTo segment.
    ControlPoint2,
    /// The control point of a QuadTo segment.
    QuadControl,
}

/// A reference to a point within a path's element list.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct HitHandle {
    /// Index into the `BezPath::elements()` vector.
    element_index: usize,
    kind: HandleKind,
    /// The position of the handle at the time of the hit.
    position: Point,
}

// ---------------------------------------------------------------------------
// NodeTool state
// ---------------------------------------------------------------------------

/// Internal state of the node tool.
#[derive(Debug)]
enum NodeState {
    /// Waiting for input.
    Idle,
    /// Dragging a node or handle.
    Dragging {
        /// The path element we are editing.
        path_node_id: NodeId,
        /// The path data before the drag started (for undo).
        original_path: BezPath,
        /// Which handle we are dragging.
        handle: HitHandle,
        /// The document-space position where the drag started.
        start_doc: Point,
        /// The most recent document-space position during the drag.
        last_doc: Point,
    },
}

/// Node tool for editing path anchor points and control handles.
#[derive(Debug)]
pub struct NodeTool {
    state: NodeState,
    /// Hit-test tolerance in screen pixels.
    tolerance: f64,
    /// The element index of the most recently selected handle (for delete / toggle).
    selected_element: Option<usize>,
    /// The node id of the path that owns `selected_element`.
    selected_path: Option<NodeId>,
}

impl NodeTool {
    pub fn new() -> Self {
        Self {
            state: NodeState::Idle,
            tolerance: 6.0,
            selected_element: None,
            selected_path: None,
        }
    }
}

impl Default for NodeTool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Path query helpers
// ---------------------------------------------------------------------------

/// Find the handle nearest to `doc_pt` in the given path, within `tolerance`
/// (in document-space units). Returns `None` if nothing is close enough.
fn find_nearest_handle(path: &BezPath, doc_pt: Point, tolerance: f64) -> Option<HitHandle> {
    let elements = path.elements();
    let tol_sq = tolerance * tolerance;
    let mut best: Option<(f64, HitHandle)> = None;

    for (i, seg) in elements.iter().enumerate() {
        let candidates: Vec<(HandleKind, Point)> = match *seg {
            PathSegment::MoveTo(p) => vec![(HandleKind::Anchor, p)],
            PathSegment::LineTo(p) => vec![(HandleKind::Anchor, p)],
            PathSegment::QuadTo(cp, end) => {
                vec![(HandleKind::QuadControl, cp), (HandleKind::Anchor, end)]
            }
            PathSegment::CurveTo(cp1, cp2, end) => vec![
                (HandleKind::ControlPoint1, cp1),
                (HandleKind::ControlPoint2, cp2),
                (HandleKind::Anchor, end),
            ],
            PathSegment::ClosePath => vec![],
        };

        for (kind, pos) in candidates {
            let dist_sq = pos.distance_squared(doc_pt);
            if dist_sq <= tol_sq && (best.is_none() || dist_sq < best.as_ref().unwrap().0) {
                best = Some((
                    dist_sq,
                    HitHandle {
                        element_index: i,
                        kind,
                        position: pos,
                    },
                ));
            }
        }
    }

    best.map(|(_, h)| h)
}

/// Build a new `BezPath` with one handle moved by `delta`.
fn path_with_moved_handle(path: &BezPath, handle: &HitHandle, delta: Point) -> BezPath {
    let elements = path.elements();
    let mut result = BezPath::new();
    let dx = delta.x;
    let dy = delta.y;

    for (i, seg) in elements.iter().enumerate() {
        if i == handle.element_index {
            match *seg {
                PathSegment::MoveTo(p) => {
                    debug_assert!(handle.kind == HandleKind::Anchor);
                    result.move_to(Point::new(p.x + dx, p.y + dy));
                }
                PathSegment::LineTo(p) => {
                    debug_assert!(handle.kind == HandleKind::Anchor);
                    result.line_to(Point::new(p.x + dx, p.y + dy));
                }
                PathSegment::QuadTo(cp, end) => match handle.kind {
                    HandleKind::QuadControl => {
                        result.quad_to(Point::new(cp.x + dx, cp.y + dy), end);
                    }
                    HandleKind::Anchor => {
                        result.quad_to(cp, Point::new(end.x + dx, end.y + dy));
                    }
                    _ => {
                        result.quad_to(cp, end);
                    }
                },
                PathSegment::CurveTo(cp1, cp2, end) => match handle.kind {
                    HandleKind::ControlPoint1 => {
                        result.curve_to(Point::new(cp1.x + dx, cp1.y + dy), cp2, end);
                    }
                    HandleKind::ControlPoint2 => {
                        result.curve_to(cp1, Point::new(cp2.x + dx, cp2.y + dy), end);
                    }
                    HandleKind::Anchor => {
                        result.curve_to(cp1, cp2, Point::new(end.x + dx, end.y + dy));
                    }
                    _ => {
                        result.curve_to(cp1, cp2, end);
                    }
                },
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        } else {
            // Copy segment unchanged.
            match *seg {
                PathSegment::MoveTo(p) => result.move_to(p),
                PathSegment::LineTo(p) => result.line_to(p),
                PathSegment::QuadTo(cp, end) => result.quad_to(cp, end),
                PathSegment::CurveTo(cp1, cp2, end) => result.curve_to(cp1, cp2, end),
                PathSegment::ClosePath => result.close(),
            }
        }
    }

    result
}

/// Build a new `BezPath` with the node at `element_index` removed.
///
/// Removing an anchor point means removing the segment that draws to it.
/// If the removed element is a `MoveTo`, the next segment (if any) is
/// promoted to a `MoveTo` using its endpoint.
fn path_without_node(path: &BezPath, element_index: usize) -> BezPath {
    let elements = path.elements();
    if elements.is_empty() || element_index >= elements.len() {
        return path.clone();
    }

    // If only one or two elements, removing a node leaves nothing useful.
    if elements.len() <= 2 {
        return BezPath::new();
    }

    let mut result = BezPath::new();
    let removed = &elements[element_index];
    let is_move = matches!(removed, PathSegment::MoveTo(_));

    let mut skip_next_promote = false;

    for (i, seg) in elements.iter().enumerate() {
        if i == element_index {
            // If we are removing a MoveTo, the next drawing command needs to
            // become a MoveTo instead.
            if is_move {
                skip_next_promote = true;
            }
            continue;
        }

        if skip_next_promote {
            skip_next_promote = false;
            // Promote the endpoint of this segment to a MoveTo.
            let endpoint = match *seg {
                PathSegment::MoveTo(p) => p,
                PathSegment::LineTo(p) => p,
                PathSegment::QuadTo(_, p) => p,
                PathSegment::CurveTo(_, _, p) => p,
                PathSegment::ClosePath => {
                    // ClosePath right after a removed MoveTo: skip it.
                    continue;
                }
            };
            result.move_to(endpoint);
            continue;
        }

        match *seg {
            PathSegment::MoveTo(p) => result.move_to(p),
            PathSegment::LineTo(p) => result.line_to(p),
            PathSegment::QuadTo(cp, end) => result.quad_to(cp, end),
            PathSegment::CurveTo(cp1, cp2, end) => result.curve_to(cp1, cp2, end),
            PathSegment::ClosePath => result.close(),
        }
    }

    result
}

/// Toggle segment type at `element_index` between line and cubic curve.
///
/// - If the segment is a `LineTo`, convert it to a `CurveTo` with control
///   points at 1/3 and 2/3 of the line (producing a visually identical curve
///   that can then be adjusted).
/// - If the segment is a `CurveTo`, convert it to a `LineTo` to the endpoint
///   (losing the curve shape).
/// - `QuadTo` segments are converted to `LineTo`.
/// - Other segment types are left unchanged.
fn path_with_toggled_segment(path: &BezPath, element_index: usize) -> BezPath {
    let elements = path.elements();
    if element_index >= elements.len() {
        return path.clone();
    }

    let mut result = BezPath::new();

    // We need the "current point" before the target element in order to
    // compute control points when converting LineTo -> CurveTo.
    let mut current_point = Point::ZERO;

    for (i, seg) in elements.iter().enumerate() {
        if i == element_index {
            match *seg {
                PathSegment::LineTo(end) => {
                    // Convert to cubic with control points at 1/3 and 2/3.
                    let cp1 = current_point.lerp(end, 1.0 / 3.0);
                    let cp2 = current_point.lerp(end, 2.0 / 3.0);
                    result.curve_to(cp1, cp2, end);
                    current_point = end;
                }
                PathSegment::CurveTo(_cp1, _cp2, end) => {
                    result.line_to(end);
                    current_point = end;
                }
                PathSegment::QuadTo(_cp, end) => {
                    result.line_to(end);
                    current_point = end;
                }
                // MoveTo and ClosePath are not drawing segments; leave as is.
                PathSegment::MoveTo(p) => {
                    result.move_to(p);
                    current_point = p;
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        } else {
            match *seg {
                PathSegment::MoveTo(p) => {
                    result.move_to(p);
                    current_point = p;
                }
                PathSegment::LineTo(p) => {
                    result.line_to(p);
                    current_point = p;
                }
                PathSegment::QuadTo(cp, end) => {
                    result.quad_to(cp, end);
                    current_point = end;
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    result.curve_to(cp1, cp2, end);
                    current_point = end;
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        }
    }

    result
}

/// Try to find the first selected path element in the selection set.
/// Returns `(NodeId, &PathAttrs)` or `None`.
fn selected_path<'a>(
    doc: &'a SvgDocument,
    selection: &SelectionSet,
) -> Option<(NodeId, &'a BezPath)> {
    for &id in selection.selected() {
        if let Some(node) = doc.get_node(id) {
            if let SvgElement::Path(ref attrs) = node.element {
                return Some((id, &attrs.d));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

impl Tool for NodeTool {
    fn name(&self) -> &str {
        "Node"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        let tol = self.tolerance / viewport.zoom;

        // Find the path we are editing (first selected path).
        let (path_node_id, path_data) = match selected_path(doc, selection) {
            Some(pair) => pair,
            None => return ToolResponse::None,
        };

        // Hit-test against nodes and handles of this path.
        if let Some(handle) = find_nearest_handle(path_data, doc_pt, tol) {
            // Remember which node is selected for key events.
            if handle.kind == HandleKind::Anchor {
                self.selected_element = Some(handle.element_index);
                self.selected_path = Some(path_node_id);
            }

            self.state = NodeState::Dragging {
                path_node_id,
                original_path: path_data.clone(),
                handle,
                start_doc: doc_pt,
                last_doc: doc_pt,
            };
            return ToolResponse::Handled;
        }

        // Click on empty space -- deselect handle.
        self.selected_element = None;
        self.selected_path = None;
        ToolResponse::None
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if let NodeState::Dragging {
            ref mut last_doc, ..
        } = self.state
        {
            *last_doc = viewport.screen_to_document(Point::new(event.x, event.y));
            ToolResponse::Handled
        } else {
            ToolResponse::None
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let state = std::mem::replace(&mut self.state, NodeState::Idle);

        match state {
            NodeState::Dragging {
                path_node_id,
                original_path,
                handle,
                start_doc,
                last_doc: _,
            } => {
                let end_doc = viewport.screen_to_document(Point::new(event.x, event.y));
                let delta = Point::new(end_doc.x - start_doc.x, end_doc.y - start_doc.y);

                // If the drag distance is negligible, treat as a click (select).
                if delta.x.abs() < 0.001 && delta.y.abs() < 0.001 {
                    return ToolResponse::Handled;
                }

                let new_path = path_with_moved_handle(&original_path, &handle, delta);
                let cmd = ModifyPathCommand::new(path_node_id, original_path, new_path);
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            NodeState::Idle => ToolResponse::None,
        }
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        match event.key {
            // Delete / Backspace: remove the selected node from the path.
            Key::Delete | Key::Backspace => {
                let elem_idx = match self.selected_element {
                    Some(idx) => idx,
                    None => return ToolResponse::None,
                };
                let path_node_id = match self.selected_path {
                    Some(id) => id,
                    None => return ToolResponse::None,
                };
                let path_data = match doc.get_node(path_node_id) {
                    Some(node) => match &node.element {
                        SvgElement::Path(attrs) => &attrs.d,
                        _ => return ToolResponse::None,
                    },
                    None => return ToolResponse::None,
                };

                let new_path = path_without_node(path_data, elem_idx);
                let cmd = ModifyPathCommand::new(path_node_id, path_data.clone(), new_path);
                self.selected_element = None;
                self.selected_path = None;
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }

            // 'T' key: toggle segment type (line <-> cubic).
            Key::T => {
                let elem_idx = match self.selected_element {
                    Some(idx) => idx,
                    None => return ToolResponse::None,
                };
                let (path_node_id, path_data) = match selected_path(doc, selection) {
                    Some(pair) => pair,
                    None => return ToolResponse::None,
                };

                let new_path = path_with_toggled_segment(path_data, elem_idx);
                if new_path == *path_data {
                    return ToolResponse::Handled;
                }
                let cmd = ModifyPathCommand::new(path_node_id, path_data.clone(), new_path);
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }

            _ => ToolResponse::None,
        }
    }

    fn on_activate(&mut self) {
        self.state = NodeState::Idle;
        self.selected_element = None;
        self.selected_path = None;
    }

    fn on_deactivate(&mut self) {
        self.state = NodeState::Idle;
        self.selected_element = None;
        self.selected_path = None;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::BezPath;

    #[test]
    fn test_find_anchor_on_move_to() {
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(100.0, 10.0));
        path.line_to(Point::new(100.0, 100.0));

        let hit = find_nearest_handle(&path, Point::new(10.5, 10.5), 5.0);
        assert!(hit.is_some());
        let h = hit.unwrap();
        assert_eq!(h.element_index, 0);
        assert_eq!(h.kind, HandleKind::Anchor);
    }

    #[test]
    fn test_find_anchor_on_line_to() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));

        let hit = find_nearest_handle(&path, Point::new(99.5, 0.5), 5.0);
        assert!(hit.is_some());
        let h = hit.unwrap();
        assert_eq!(h.element_index, 1);
        assert_eq!(h.kind, HandleKind::Anchor);
    }

    #[test]
    fn test_find_control_point() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(10.0, 50.0),
            Point::new(90.0, 50.0),
            Point::new(100.0, 0.0),
        );

        // Click near cp1.
        let hit = find_nearest_handle(&path, Point::new(10.5, 50.5), 5.0);
        assert!(hit.is_some());
        let h = hit.unwrap();
        assert_eq!(h.element_index, 1);
        assert_eq!(h.kind, HandleKind::ControlPoint1);
    }

    #[test]
    fn test_no_hit_when_far() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(10.0, 0.0));

        let hit = find_nearest_handle(&path, Point::new(500.0, 500.0), 5.0);
        assert!(hit.is_none());
    }

    #[test]
    fn test_move_anchor() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));

        let handle = HitHandle {
            element_index: 1,
            kind: HandleKind::Anchor,
            position: Point::new(100.0, 0.0),
        };

        let moved = path_with_moved_handle(&path, &handle, Point::new(10.0, 20.0));
        let elems = moved.elements();
        assert_eq!(elems.len(), 3);
        match elems[1] {
            PathSegment::LineTo(p) => {
                assert!((p.x - 110.0).abs() < 1e-10);
                assert!((p.y - 20.0).abs() < 1e-10);
            }
            _ => panic!("expected LineTo"),
        }
    }

    #[test]
    fn test_move_control_point() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(10.0, 50.0),
            Point::new(90.0, 50.0),
            Point::new(100.0, 0.0),
        );

        let handle = HitHandle {
            element_index: 1,
            kind: HandleKind::ControlPoint1,
            position: Point::new(10.0, 50.0),
        };

        let moved = path_with_moved_handle(&path, &handle, Point::new(5.0, -10.0));
        let elems = moved.elements();
        match elems[1] {
            PathSegment::CurveTo(cp1, cp2, end) => {
                assert!((cp1.x - 15.0).abs() < 1e-10);
                assert!((cp1.y - 40.0).abs() < 1e-10);
                // cp2 and end should be unchanged.
                assert!((cp2.x - 90.0).abs() < 1e-10);
                assert!((end.x - 100.0).abs() < 1e-10);
            }
            _ => panic!("expected CurveTo"),
        }
    }

    #[test]
    fn test_remove_node() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));

        let reduced = path_without_node(&path, 1);
        let elems = reduced.elements();
        assert_eq!(elems.len(), 2);
        match elems[0] {
            PathSegment::MoveTo(p) => assert!((p.x).abs() < 1e-10),
            _ => panic!("expected MoveTo"),
        }
        match elems[1] {
            PathSegment::LineTo(p) => assert!((p.x - 100.0).abs() < 1e-10),
            _ => panic!("expected LineTo"),
        }
    }

    #[test]
    fn test_remove_move_to_promotes_next() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));

        let reduced = path_without_node(&path, 0);
        let elems = reduced.elements();
        // After removing MoveTo(0,0), LineTo(50,0) should become MoveTo(50,0).
        assert_eq!(elems.len(), 2);
        assert!(matches!(elems[0], PathSegment::MoveTo(_)));
    }

    #[test]
    fn test_toggle_line_to_curve() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(90.0, 0.0));

        let toggled = path_with_toggled_segment(&path, 1);
        let elems = toggled.elements();
        assert_eq!(elems.len(), 2);
        match elems[1] {
            PathSegment::CurveTo(cp1, cp2, end) => {
                assert!((cp1.x - 30.0).abs() < 1e-10, "cp1.x={}", cp1.x);
                assert!((cp2.x - 60.0).abs() < 1e-10, "cp2.x={}", cp2.x);
                assert!((end.x - 90.0).abs() < 1e-10);
            }
            _ => panic!("expected CurveTo after toggle, got {:?}", elems[1]),
        }
    }

    #[test]
    fn test_toggle_curve_to_line() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(10.0, 50.0),
            Point::new(90.0, 50.0),
            Point::new(100.0, 0.0),
        );

        let toggled = path_with_toggled_segment(&path, 1);
        let elems = toggled.elements();
        assert_eq!(elems.len(), 2);
        match elems[1] {
            PathSegment::LineTo(p) => {
                assert!((p.x - 100.0).abs() < 1e-10);
                assert!((p.y).abs() < 1e-10);
            }
            _ => panic!("expected LineTo after toggle, got {:?}", elems[1]),
        }
    }

    #[test]
    fn test_toggle_roundtrip() {
        // LineTo -> CurveTo -> LineTo should land on the same endpoint.
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));

        let as_curve = path_with_toggled_segment(&path, 1);
        let back_to_line = path_with_toggled_segment(&as_curve, 1);
        let elems = back_to_line.elements();
        match elems[1] {
            PathSegment::LineTo(p) => {
                assert!((p.x - 100.0).abs() < 1e-10);
                assert!((p.y).abs() < 1e-10);
            }
            _ => panic!("expected LineTo after double toggle"),
        }
    }
}
