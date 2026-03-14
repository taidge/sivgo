//! Marker tool: place and edit markers on path endpoints.
//!
//! States:
//! - **Idle**: waiting for input.
//! - **Placing**: user has clicked on a path element; the marker is toggled
//!   on the nearest endpoint (start, mid, or end).
//!
//! On click on a path element: toggle a marker on the nearest endpoint.
//! Uses a built-in arrow marker definition.

use sivgo_dom::types::{LengthUnit, SvgLength};
use sivgo_dom::{
    DefsAttrs, MarkerAttrs, NodeId, PathAttrs, PresentationAttrs, SvgDocument, SvgElement, SvgNode,
};
use sivgo_math::{BezPath, PathSegment, Point};

use crate::command::Command;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// MarkerTool
// ---------------------------------------------------------------------------

/// Interactive tool for placing SVG markers on path endpoints.
///
/// Clicking on a path toggles a marker on its nearest endpoint (start, mid,
/// or end). A built-in arrow marker definition is created in `<defs>` if one
/// does not already exist.
#[derive(Debug)]
pub struct MarkerTool {
    /// The built-in marker XML id (created lazily in defs).
    builtin_marker_id: String,
}

impl MarkerTool {
    pub fn new() -> Self {
        Self {
            builtin_marker_id: "sivgo-arrow-marker".to_string(),
        }
    }
}

impl Default for MarkerTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Determine which endpoint type (start, mid, end) is closest to the given
/// document point for a path element.
///
/// Returns `"start"`, `"mid"`, or `"end"`.
fn nearest_endpoint_type(path: &BezPath, click_pt: Point) -> &'static str {
    let segments = path.elements();
    if segments.is_empty() {
        return "start";
    }

    let mut first_pt: Option<Point> = None;
    let mut last_pt: Option<Point> = None;
    let mut mid_pts: Vec<Point> = Vec::new();

    for seg in segments {
        let pt = match seg {
            PathSegment::MoveTo(p) => Some(p),
            PathSegment::LineTo(p) => Some(p),
            PathSegment::QuadTo(_, p) => Some(p),
            PathSegment::CurveTo(_, _, p) => Some(p),
            PathSegment::ClosePath => None,
        };

        if let Some(p) = pt {
            if first_pt.is_none() {
                first_pt = Some(p);
            } else {
                // Previous "last" becomes a mid point.
                if let Some(prev_last) = last_pt {
                    mid_pts.push(prev_last);
                }
            }
            last_pt = Some(p);
        }
    }

    let start = first_pt.unwrap_or(Point::new(0.0, 0.0));
    let end = last_pt.unwrap_or(start);

    let dist_start = (click_pt.x - start.x).powi(2) + (click_pt.y - start.y).powi(2);
    let dist_end = (click_pt.x - end.x).powi(2) + (click_pt.y - end.y).powi(2);

    // Check mid points.
    let dist_mid = mid_pts
        .iter()
        .map(|p| (click_pt.x - p.x).powi(2) + (click_pt.y - p.y).powi(2))
        .fold(f64::MAX, f64::min);

    if dist_start <= dist_end && dist_start <= dist_mid {
        "start"
    } else if dist_end <= dist_mid {
        "end"
    } else {
        "mid"
    }
}

impl Tool for MarkerTool {
    fn name(&self) -> &str {
        "Marker"
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

        // Require a selected path element.
        let element_id = match selection.primary() {
            Some(id) => id,
            None => return ToolResponse::None,
        };

        let node = match doc.get_node(element_id) {
            Some(n) => n,
            None => return ToolResponse::None,
        };

        let path_data = match &node.element {
            SvgElement::Path(attrs) => &attrs.d,
            _ => return ToolResponse::None,
        };

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        let endpoint = nearest_endpoint_type(path_data, doc_pt);

        let marker_url = format!("url(#{})", self.builtin_marker_id);
        let old_pres = node.presentation.clone();

        let cmd = ToggleMarkerCommand::new(
            element_id,
            endpoint.to_string(),
            marker_url,
            self.builtin_marker_id.clone(),
            old_pres,
        );
        ToolResponse::ExecuteCommand(Box::new(cmd))
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_mouse_up(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_key_down(
        &mut self,
        _event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_activate(&mut self) {}
    fn on_deactivate(&mut self) {}

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

// ---------------------------------------------------------------------------
// ToggleMarkerCommand
// ---------------------------------------------------------------------------

/// Toggles a marker on a specific endpoint of a path element.
///
/// If the endpoint already has this marker, it is removed (set to `None`).
/// If it does not have one, the marker URL is set.
///
/// Also ensures the built-in arrow marker definition exists in `<defs>`.
#[derive(Debug)]
struct ToggleMarkerCommand {
    target_id: NodeId,
    /// "start", "mid", or "end"
    endpoint: String,
    marker_url: String,
    marker_xml_id: String,
    old_pres: PresentationAttrs,

    // State filled on execute.
    created_defs: bool,
    defs_id: Option<NodeId>,
    created_marker: bool,
    marker_node_id: Option<NodeId>,
    /// The arrow path child of the marker element.
    arrow_path_id: Option<NodeId>,
    /// Whether the marker was toggled on (true) or off (false).
    toggled_on: Option<bool>,
}

impl ToggleMarkerCommand {
    fn new(
        target_id: NodeId,
        endpoint: String,
        marker_url: String,
        marker_xml_id: String,
        old_pres: PresentationAttrs,
    ) -> Self {
        Self {
            target_id,
            endpoint,
            marker_url,
            marker_xml_id,
            old_pres,
            created_defs: false,
            defs_id: None,
            created_marker: false,
            marker_node_id: None,
            arrow_path_id: None,
            toggled_on: None,
        }
    }

    fn find_defs(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        for &child_id in doc.children(root) {
            if let Some(node) = doc.get_node(child_id) {
                if matches!(node.element, SvgElement::Defs(_)) {
                    return Some(child_id);
                }
            }
        }
        None
    }

    fn find_marker_in_defs(doc: &SvgDocument, defs_id: NodeId, xml_id: &str) -> Option<NodeId> {
        for &child_id in doc.children(defs_id) {
            if let Some(node) = doc.get_node(child_id) {
                if node.xml_id.as_deref() == Some(xml_id) {
                    return Some(child_id);
                }
            }
        }
        None
    }
}

impl Command for ToggleMarkerCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let root = match doc.root() {
            Some(r) => r,
            None => return,
        };

        // 1. Ensure <defs> exists.
        let defs_id = if let Some(existing) = Self::find_defs(doc) {
            self.created_defs = false;
            existing
        } else {
            let id = if let Some(prev_id) = self.defs_id {
                let node = SvgNode::new(prev_id, SvgElement::Defs(DefsAttrs));
                doc.insert_node(node);
                doc.insert_child_at(root, prev_id, 0);
                prev_id
            } else {
                let id = doc.add_node(SvgElement::Defs(DefsAttrs));
                doc.insert_child_at(root, id, 0);
                id
            };
            self.created_defs = true;
            id
        };
        self.defs_id = Some(defs_id);

        // 2. Ensure the arrow marker definition exists.
        if Self::find_marker_in_defs(doc, defs_id, &self.marker_xml_id).is_none() {
            let marker_element = SvgElement::Marker(MarkerAttrs {
                ref_x: Some(SvgLength::new(10.0, LengthUnit::None)),
                ref_y: Some(SvgLength::new(5.0, LengthUnit::None)),
                marker_width: Some(SvgLength::new(10.0, LengthUnit::None)),
                marker_height: Some(SvgLength::new(10.0, LengthUnit::None)),
                orient: None,
                marker_units: None,
                view_box: None,
                preserve_aspect_ratio: None,
            });

            let marker_id = if let Some(prev_id) = self.marker_node_id {
                let mut node = SvgNode::new(prev_id, marker_element);
                node.xml_id = Some(self.marker_xml_id.clone());
                doc.insert_node(node);
                doc.append_child(defs_id, prev_id);
                prev_id
            } else {
                let id = doc.add_node(marker_element);
                if let Some(node) = doc.get_node_mut(id) {
                    node.xml_id = Some(self.marker_xml_id.clone());
                }
                doc.append_child(defs_id, id);
                id
            };
            self.marker_node_id = Some(marker_id);

            // Add an arrow path inside the marker.
            let mut arrow = BezPath::new();
            arrow.move_to(Point::new(0.0, 0.0));
            arrow.line_to(Point::new(10.0, 5.0));
            arrow.line_to(Point::new(0.0, 10.0));
            arrow.close();

            let arrow_element = SvgElement::Path(PathAttrs {
                d: arrow,
                ..Default::default()
            });

            let arrow_id = if let Some(prev_id) = self.arrow_path_id {
                let node = SvgNode::new(prev_id, arrow_element);
                doc.insert_node(node);
                doc.append_child(marker_id, prev_id);
                prev_id
            } else {
                let id = doc.add_node(arrow_element);
                doc.append_child(marker_id, id);
                id
            };
            self.arrow_path_id = Some(arrow_id);
            self.created_marker = true;
        } else {
            self.created_marker = false;
        }

        // 3. Toggle the marker on the target element.
        if let Some(node) = doc.get_node_mut(self.target_id) {
            let current = match self.endpoint.as_str() {
                "start" => &node.presentation.marker_start,
                "mid" => &node.presentation.marker_mid,
                "end" => &node.presentation.marker_end,
                _ => &None,
            };

            let already_set = current.as_ref() == Some(&self.marker_url);

            if already_set {
                // Remove the marker.
                match self.endpoint.as_str() {
                    "start" => node.presentation.marker_start = None,
                    "mid" => node.presentation.marker_mid = None,
                    "end" => node.presentation.marker_end = None,
                    _ => {}
                }
                self.toggled_on = Some(false);
            } else {
                // Set the marker.
                match self.endpoint.as_str() {
                    "start" => node.presentation.marker_start = Some(self.marker_url.clone()),
                    "mid" => node.presentation.marker_mid = Some(self.marker_url.clone()),
                    "end" => node.presentation.marker_end = Some(self.marker_url.clone()),
                    _ => {}
                }
                self.toggled_on = Some(true);
            }
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Restore presentation attrs.
        if let Some(node) = doc.get_node_mut(self.target_id) {
            node.presentation = self.old_pres.clone();
        }

        // Remove the marker definition if we created it.
        if self.created_marker {
            if let Some(id) = self.arrow_path_id {
                doc.remove_node(id);
            }
            if let Some(id) = self.marker_node_id {
                doc.remove_node(id);
            }
        }

        if self.created_defs {
            if let Some(id) = self.defs_id {
                doc.remove_node(id);
            }
        }
    }

    fn description(&self) -> &str {
        "Toggle marker"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::SvgSvgAttrs;

    fn make_doc_with_path() -> (SvgDocument, NodeId) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(50.0, 30.0));
        path.line_to(Point::new(90.0, 10.0));

        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        }));
        doc.append_child(svg_id, path_id);
        (doc, path_id)
    }

    #[test]
    fn marker_tool_ignores_non_path() {
        let mut tool = MarkerTool::new();
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(sivgo_dom::RectAttrs::default()));
        doc.append_child(svg_id, rect_id);

        let mut sel = SelectionSet::new();
        sel.select(rect_id);
        let vp = Viewport::new(800.0, 600.0);

        let resp = tool.on_mouse_down(MouseEvent::left(50.0, 50.0), &doc, &sel, &vp);
        assert!(matches!(resp, ToolResponse::None));
    }

    #[test]
    fn nearest_endpoint_start() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 50.0));
        path.line_to(Point::new(100.0, 0.0));

        // Click near (0, 0) should return "start".
        assert_eq!(nearest_endpoint_type(&path, Point::new(1.0, 1.0)), "start");
    }

    #[test]
    fn nearest_endpoint_end() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 50.0));
        path.line_to(Point::new(100.0, 0.0));

        // Click near (100, 0) should return "end".
        assert_eq!(nearest_endpoint_type(&path, Point::new(99.0, 1.0)), "end");
    }

    #[test]
    fn nearest_endpoint_mid() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 50.0));
        path.line_to(Point::new(100.0, 0.0));

        // Click near (50, 50) should return "mid".
        assert_eq!(nearest_endpoint_type(&path, Point::new(49.0, 49.0)), "mid");
    }

    #[test]
    fn toggle_marker_on_path() {
        let (mut doc, path_id) = make_doc_with_path();

        let old_pres = doc.get_node(path_id).unwrap().presentation.clone();
        let mut cmd = ToggleMarkerCommand::new(
            path_id,
            "end".to_string(),
            "url(#sivgo-arrow-marker)".to_string(),
            "sivgo-arrow-marker".to_string(),
            old_pres,
        );
        cmd.execute(&mut doc);

        // Marker should be set.
        let node = doc.get_node(path_id).unwrap();
        assert_eq!(
            node.presentation.marker_end.as_deref(),
            Some("url(#sivgo-arrow-marker)")
        );

        // A <defs> with the marker should exist.
        let root = doc.root().unwrap();
        let children = doc.children(root);
        assert!(children.len() >= 2);
    }

    #[test]
    fn undo_toggle_marker() {
        let (mut doc, path_id) = make_doc_with_path();

        let old_pres = doc.get_node(path_id).unwrap().presentation.clone();
        let mut cmd = ToggleMarkerCommand::new(
            path_id,
            "start".to_string(),
            "url(#sivgo-arrow-marker)".to_string(),
            "sivgo-arrow-marker".to_string(),
            old_pres,
        );
        cmd.execute(&mut doc);
        assert!(doc
            .get_node(path_id)
            .unwrap()
            .presentation
            .marker_start
            .is_some());

        cmd.undo(&mut doc);
        assert!(doc
            .get_node(path_id)
            .unwrap()
            .presentation
            .marker_start
            .is_none());
    }
}
