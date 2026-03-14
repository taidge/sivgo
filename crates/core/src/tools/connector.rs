//! Connector tool: draw lines connecting two elements.
//!
//! ## Features
//!
//! - **17.5.1** Create connectors between objects
//! - **17.5.2** Auto-routing around obstacles (simple axis-aligned avoidance)
//! - **17.5.3** Connector types: Straight, Polyline, Orthogonal

use sivgo_dom::types::{Color, LengthUnit, LineCap, Paint, SvgLength};
use sivgo_dom::{LineAttrs, NodeId, PolylineAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{Point, Rect};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::{element_bounding_box, SelectionSet};
use crate::tool::{Tool, ToolResponse};
use crate::tools::select::hit_test_at;
use crate::viewport::Viewport;

/// The connector routing type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    /// A direct straight line between source and target centers.
    Straight,
    /// A polyline with intermediate waypoints that avoids bounding boxes of
    /// other elements in the document (simple obstacle avoidance).
    Polyline,
    /// An orthogonal (right-angle) connector: horizontal then vertical segments
    /// connecting the two endpoints.
    Orthogonal,
}

impl Default for ConnectorType {
    fn default() -> Self {
        Self::Straight
    }
}

/// State of the connector tool.
#[derive(Debug)]
enum ConnectorState {
    /// Waiting for the user to press on the first element.
    Idle,
    /// The user pressed on an element; waiting for release on another element.
    Started {
        /// The node id of the first (source) element.
        source_id: NodeId,
        /// The center of the source element's bounding box.
        source_center: Point,
    },
}

/// The connector tool.
///
/// Draw a connector between two elements by pressing on one and releasing on
/// another. The connector type and routing behaviour can be configured.
#[derive(Debug)]
pub struct ConnectorTool {
    state: ConnectorState,
    /// The connector type to use (17.5.3).
    connector_type: ConnectorType,
    /// Whether to attempt auto-routing around obstacles (17.5.2).
    auto_route: bool,
    /// Margin in document units to keep between routed paths and obstacles.
    route_margin: f64,
}

impl ConnectorTool {
    pub fn new() -> Self {
        Self {
            state: ConnectorState::Idle,
            connector_type: ConnectorType::Straight,
            auto_route: false,
            route_margin: 10.0,
        }
    }

    /// Set the connector type.
    #[allow(dead_code)]
    pub fn set_connector_type(&mut self, ct: ConnectorType) {
        self.connector_type = ct;
    }

    /// Get the current connector type.
    #[allow(dead_code)]
    pub fn connector_type(&self) -> ConnectorType {
        self.connector_type
    }

    /// Enable or disable auto-routing around obstacles (17.5.2).
    #[allow(dead_code)]
    pub fn set_auto_route(&mut self, enabled: bool) {
        self.auto_route = enabled;
    }

    /// Whether auto-routing is enabled.
    #[allow(dead_code)]
    pub fn auto_route(&self) -> bool {
        self.auto_route
    }

    /// Set the margin for routed connectors.
    #[allow(dead_code)]
    pub fn set_route_margin(&mut self, margin: f64) {
        self.route_margin = margin.max(0.0);
    }

    /// Collect bounding boxes of all leaf elements except source and target.
    fn collect_obstacles(
        &self,
        doc: &SvgDocument,
        source_id: NodeId,
        target_id: NodeId,
    ) -> Vec<Rect> {
        let root = match doc.root() {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut obstacles = Vec::new();
        for id in doc.traverse_pre_order(root) {
            if id == source_id || id == target_id {
                continue;
            }
            let node = match doc.get_node(id) {
                Some(n) => n,
                None => continue,
            };
            match &node.element {
                SvgElement::Svg(_) | SvgElement::Group(_) | SvgElement::Defs(_) => continue,
                _ => {}
            }
            if let Some(bb) = element_bounding_box(&node.element) {
                // Inflate by margin.
                let inflated = Rect::new(
                    bb.x0 - self.route_margin,
                    bb.y0 - self.route_margin,
                    bb.x1 + self.route_margin,
                    bb.y1 + self.route_margin,
                );
                obstacles.push(inflated);
            }
        }
        obstacles
    }

    /// Build connector points between `src` and `dst` using current settings.
    fn build_connector_points(
        &self,
        src: Point,
        dst: Point,
        doc: &SvgDocument,
        source_id: NodeId,
        target_id: NodeId,
    ) -> Vec<(f64, f64)> {
        match self.connector_type {
            ConnectorType::Straight => {
                vec![(src.x, src.y), (dst.x, dst.y)]
            }
            ConnectorType::Orthogonal => {
                // Two-segment orthogonal route: horizontal then vertical.
                // Use midpoint x for the bend.
                if self.auto_route {
                    self.orthogonal_auto_route(src, dst, doc, source_id, target_id)
                } else {
                    vec![(src.x, src.y), (dst.x, src.y), (dst.x, dst.y)]
                }
            }
            ConnectorType::Polyline => {
                if self.auto_route {
                    self.polyline_auto_route(src, dst, doc, source_id, target_id)
                } else {
                    vec![(src.x, src.y), (dst.x, dst.y)]
                }
            }
        }
    }

    /// Simple orthogonal auto-routing: try horizontal-then-vertical, and if
    /// the bend point collides with an obstacle, route via a midpoint offset.
    fn orthogonal_auto_route(
        &self,
        src: Point,
        dst: Point,
        doc: &SvgDocument,
        source_id: NodeId,
        target_id: NodeId,
    ) -> Vec<(f64, f64)> {
        let obstacles = self.collect_obstacles(doc, source_id, target_id);
        let bend = Point::new(dst.x, src.y);

        // Check if the bend point is inside any obstacle.
        let bend_blocked = obstacles.iter().any(|r| r.contains_point(bend));

        if !bend_blocked {
            return vec![(src.x, src.y), (bend.x, bend.y), (dst.x, dst.y)];
        }

        // Try vertical-then-horizontal.
        let bend2 = Point::new(src.x, dst.y);
        let bend2_blocked = obstacles.iter().any(|r| r.contains_point(bend2));

        if !bend2_blocked {
            return vec![(src.x, src.y), (bend2.x, bend2.y), (dst.x, dst.y)];
        }

        // Both direct bends are blocked: route via an offset midpoint.
        let offset_y = if src.y < dst.y {
            src.y.min(dst.y) - self.route_margin * 3.0
        } else {
            src.y.max(dst.y) + self.route_margin * 3.0
        };

        vec![
            (src.x, src.y),
            (src.x, offset_y),
            (dst.x, offset_y),
            (dst.x, dst.y),
        ]
    }

    /// Simple polyline auto-routing around obstacles by inserting a midpoint
    /// offset perpendicular to the line direction.
    fn polyline_auto_route(
        &self,
        src: Point,
        dst: Point,
        doc: &SvgDocument,
        source_id: NodeId,
        target_id: NodeId,
    ) -> Vec<(f64, f64)> {
        let obstacles = self.collect_obstacles(doc, source_id, target_id);

        // Check if the direct path (midpoint) intersects any obstacle.
        let mid = Point::new((src.x + dst.x) / 2.0, (src.y + dst.y) / 2.0);
        let blocked = obstacles.iter().any(|r| r.contains_point(mid));

        if !blocked {
            return vec![(src.x, src.y), (dst.x, dst.y)];
        }

        // Route around by inserting an offset waypoint perpendicular to the
        // src->dst direction.
        let dx = dst.x - src.x;
        let dy = dst.y - src.y;
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        let perp_x = -dy / len;
        let perp_y = dx / len;
        let offset = self.route_margin * 3.0;

        let wp = Point::new(mid.x + perp_x * offset, mid.y + perp_y * offset);

        vec![(src.x, src.y), (wp.x, wp.y), (dst.x, dst.y)]
    }

    /// Create the connector element (line or polyline) from the built points.
    fn create_connector_element(&self, points: &[(f64, f64)]) -> SvgElement {
        if points.len() == 2 {
            SvgElement::Line(LineAttrs {
                x1: SvgLength::new(points[0].0, LengthUnit::None),
                y1: SvgLength::new(points[0].1, LengthUnit::None),
                x2: SvgLength::new(points[1].0, LengthUnit::None),
                y2: SvgLength::new(points[1].1, LengthUnit::None),
            })
        } else {
            SvgElement::Polyline(PolylineAttrs {
                points: points.to_vec(),
            })
        }
    }
}

impl Default for ConnectorTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ConnectorTool {
    fn name(&self) -> &str {
        "Connector"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        // Hit-test to find the source element.
        let source_id = match hit_test_at(doc, doc_pt) {
            Some(id) => id,
            None => {
                self.state = ConnectorState::Idle;
                return ToolResponse::Handled;
            }
        };

        // Get the bounding box center of the source element.
        let source_center = match doc.get_node(source_id) {
            Some(node) => match element_bounding_box(&node.element) {
                Some(bb) => bb.center(),
                None => doc_pt,
            },
            None => doc_pt,
        };

        self.state = ConnectorState::Started {
            source_id,
            source_center,
        };
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        match &self.state {
            ConnectorState::Started { .. } => ToolResponse::Handled,
            ConnectorState::Idle => ToolResponse::None,
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let (source_id, source_center) = match &self.state {
            ConnectorState::Started {
                source_id,
                source_center,
            } => (*source_id, *source_center),
            ConnectorState::Idle => return ToolResponse::None,
        };

        self.state = ConnectorState::Idle;

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        // Hit-test to find the target element.
        let target_id = match hit_test_at(doc, doc_pt) {
            Some(id) => id,
            None => return ToolResponse::Handled,
        };

        // Don't connect an element to itself.
        if target_id == source_id {
            return ToolResponse::Handled;
        }

        // Get the bounding box center of the target element.
        let target_center = match doc.get_node(target_id) {
            Some(node) => match element_bounding_box(&node.element) {
                Some(bb) => bb.center(),
                None => doc_pt,
            },
            None => doc_pt,
        };

        // Build the connector points with optional routing.
        let points =
            self.build_connector_points(source_center, target_center, doc, source_id, target_id);

        let element = self.create_connector_element(&points);

        let mut pres = PresentationAttrs::default();
        pres.fill = Some(Paint::None);
        pres.stroke = Some(Paint::Color(Color::BLACK));
        pres.stroke_width = Some(SvgLength::new(2.0, LengthUnit::None));
        pres.stroke_linecap = Some(LineCap::Round);

        let parent = match doc.root() {
            Some(r) => r,
            None => return ToolResponse::None,
        };

        let cmd = AddNodeCommand::new(parent, element, pres, None);
        ToolResponse::ExecuteCommand(Box::new(cmd))
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        use crate::input::Key;
        if event.key == Key::Escape {
            self.state = ConnectorState::Idle;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.state = ConnectorState::Idle;
    }

    fn on_deactivate(&mut self) {
        self.state = ConnectorState::Idle;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{RectAttrs, SvgDocument, SvgElement, SvgSvgAttrs};

    fn make_doc_with_two_rects() -> (SvgDocument, NodeId, NodeId) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let r1 = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(10.0, LengthUnit::None),
            width: SvgLength::new(30.0, LengthUnit::None),
            height: SvgLength::new(30.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, r1);

        let r2 = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(200.0, LengthUnit::None),
            y: SvgLength::new(200.0, LengthUnit::None),
            width: SvgLength::new(30.0, LengthUnit::None),
            height: SvgLength::new(30.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, r2);

        (doc, r1, r2)
    }

    #[test]
    fn connector_tool_defaults() {
        let tool = ConnectorTool::new();
        assert_eq!(tool.connector_type(), ConnectorType::Straight);
        assert!(!tool.auto_route());
    }

    #[test]
    fn straight_connector_produces_two_points() {
        let (doc, r1, r2) = make_doc_with_two_rects();
        let tool = ConnectorTool::new();
        let src = Point::new(25.0, 25.0);
        let dst = Point::new(215.0, 215.0);
        let points = tool.build_connector_points(src, dst, &doc, r1, r2);
        assert_eq!(points.len(), 2);
        assert!((points[0].0 - 25.0).abs() < 1e-10);
        assert!((points[1].0 - 215.0).abs() < 1e-10);
    }

    #[test]
    fn orthogonal_connector_produces_three_points() {
        let (doc, r1, r2) = make_doc_with_two_rects();
        let mut tool = ConnectorTool::new();
        tool.set_connector_type(ConnectorType::Orthogonal);
        let src = Point::new(25.0, 25.0);
        let dst = Point::new(215.0, 215.0);
        let points = tool.build_connector_points(src, dst, &doc, r1, r2);
        // Orthogonal should have 3 or more points (bend point).
        assert!(
            points.len() >= 3,
            "Orthogonal should have >= 3 points, got {}",
            points.len()
        );
        // First point is src.
        assert!((points[0].0 - src.x).abs() < 1e-10);
        // Last point is dst.
        let last = points.last().unwrap();
        assert!((last.0 - dst.x).abs() < 1e-10);
    }

    #[test]
    fn orthogonal_auto_route_avoids_obstacles() {
        let (doc, r1, r2) = make_doc_with_two_rects();
        let mut tool = ConnectorTool::new();
        tool.set_connector_type(ConnectorType::Orthogonal);
        tool.set_auto_route(true);
        tool.set_route_margin(5.0);

        let src = Point::new(25.0, 25.0);
        let dst = Point::new(215.0, 215.0);
        let points = tool.build_connector_points(src, dst, &doc, r1, r2);
        assert!(points.len() >= 3);
    }

    #[test]
    fn polyline_auto_route_inserts_waypoint_when_blocked() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let r1 = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(0.0, LengthUnit::None),
            y: SvgLength::new(0.0, LengthUnit::None),
            width: SvgLength::new(10.0, LengthUnit::None),
            height: SvgLength::new(10.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, r1);

        let r2 = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(190.0, LengthUnit::None),
            y: SvgLength::new(0.0, LengthUnit::None),
            width: SvgLength::new(10.0, LengthUnit::None),
            height: SvgLength::new(10.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, r2);

        // Place an obstacle right in the middle.
        let obstacle = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(90.0, LengthUnit::None),
            y: SvgLength::new(0.0, LengthUnit::None),
            width: SvgLength::new(20.0, LengthUnit::None),
            height: SvgLength::new(20.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, obstacle);

        let mut tool = ConnectorTool::new();
        tool.set_connector_type(ConnectorType::Polyline);
        tool.set_auto_route(true);
        tool.set_route_margin(5.0);

        let src = Point::new(5.0, 5.0);
        let dst = Point::new(195.0, 5.0);
        let points = tool.build_connector_points(src, dst, &doc, r1, r2);

        // Should have an intermediate waypoint to avoid the obstacle.
        assert!(
            points.len() >= 3,
            "Auto-routed polyline should have >= 3 points, got {}",
            points.len()
        );
    }

    #[test]
    fn create_connector_element_line_for_two_points() {
        let tool = ConnectorTool::new();
        let points = vec![(0.0, 0.0), (100.0, 100.0)];
        let elem = tool.create_connector_element(&points);
        assert_eq!(elem.tag_name(), "line");
    }

    #[test]
    fn create_connector_element_polyline_for_three_points() {
        let tool = ConnectorTool::new();
        let points = vec![(0.0, 0.0), (50.0, 0.0), (100.0, 100.0)];
        let elem = tool.create_connector_element(&points);
        assert_eq!(elem.tag_name(), "polyline");
    }
}
