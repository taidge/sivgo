//! Pencil tool: freehand drawing with smoothing.
//!
//! Collects raw points during a mouse drag, then fits smooth cubic Bezier
//! curves through them on mouse-up.

use sivgo_dom::types::{Color, LengthUnit, LineCap, LineJoin, Paint, SvgLength};
use sivgo_dom::{PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// A freehand drawing tool that collects raw points and produces a smooth path.
#[derive(Debug)]
pub struct PencilTool {
    /// Raw points collected during the current drag.
    points: Vec<Point>,
    /// Whether we are currently in a drag.
    drawing: bool,
    /// Smoothing amount in the range `[0.0, 1.0]`.
    ///
    /// - `0.0` – no smoothing (line segments through every point)
    /// - `1.0` – maximum smoothing (aggressive point reduction, wider curves)
    ///
    /// The default is `0.5` (moderate smoothing).
    smoothing: f64,
    /// Minimum distance (in document coordinates) between consecutive raw
    /// points.  Points closer than this are discarded to avoid clutter.
    min_distance: f64,
}

impl PencilTool {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            drawing: false,
            smoothing: 0.5,
            min_distance: 2.0,
        }
    }

    /// Set the smoothing amount (`0.0` to `1.0`).
    pub fn set_smoothing(&mut self, amount: f64) {
        self.smoothing = amount.clamp(0.0, 1.0);
    }

    /// Return the current smoothing amount.
    pub fn smoothing(&self) -> f64 {
        self.smoothing
    }

    // ------------------------------------------------------------------
    // Path construction
    // ------------------------------------------------------------------

    /// Build a smooth `BezPath` from the collected raw points.
    ///
    /// The algorithm works as follows:
    ///
    /// 1. Simplify the point list using the Ramer-Douglas-Peucker algorithm
    ///    with a tolerance derived from `self.smoothing`.
    /// 2. Fit cubic Bezier segments through groups of simplified points.
    ///    For every segment between two simplified points, control-point
    ///    placement uses the Catmull-Rom-to-Bezier conversion so that the
    ///    resulting curve passes through each simplified point with C1
    ///    continuity.
    fn build_smooth_path(&self) -> BezPath {
        let pts = &self.points;

        if pts.len() < 2 {
            return BezPath::new();
        }

        // --- Step 1: simplify ---
        // Map smoothing 0..1 to tolerance 0.5 .. 8.0.
        let tolerance = 0.5 + self.smoothing * 7.5;
        let simplified = rdp_simplify(pts, tolerance);

        if simplified.len() < 2 {
            return BezPath::new();
        }

        // --- Step 2: fit cubic Bezier curves ---
        let mut path = BezPath::new();
        path.move_to(simplified[0]);

        if simplified.len() == 2 {
            // Only two points – draw a straight line.
            path.line_to(simplified[1]);
            return path;
        }

        // Use Catmull-Rom to Bezier conversion for smooth curves through all
        // simplified points.  We need "virtual" points before the first and
        // after the last to compute tangents at the endpoints.
        let n = simplified.len();
        for i in 0..n - 1 {
            let p0 = if i == 0 {
                simplified[0]
            } else {
                simplified[i - 1]
            };
            let p1 = simplified[i];
            let p2 = simplified[i + 1];
            let p3 = if i + 2 < n {
                simplified[i + 2]
            } else {
                simplified[n - 1]
            };

            // Catmull-Rom to cubic Bezier control points.
            // The standard conversion for a uniform Catmull-Rom spline:
            //   cp1 = p1 + (p2 - p0) / 6
            //   cp2 = p2 - (p3 - p1) / 6
            let cp1 = Point::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
            let cp2 = Point::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);

            path.curve_to(cp1, cp2, p2);
        }

        path
    }

    /// Finalize the current stroke: build a smooth path and emit an
    /// `AddNodeCommand`.
    fn finalize(&mut self, doc: &SvgDocument) -> ToolResponse {
        if self.points.len() < 2 {
            self.points.clear();
            self.drawing = false;
            return ToolResponse::None;
        }

        let path = self.build_smooth_path();
        if path.is_empty() {
            self.points.clear();
            self.drawing = false;
            return ToolResponse::None;
        }

        let element = SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        });

        let pres = PresentationAttrs {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(SvgLength::new(2.0, LengthUnit::None)),
            stroke_linecap: Some(LineCap::Round),
            stroke_linejoin: Some(LineJoin::Round),
            ..Default::default()
        };

        let parent = match doc.root() {
            Some(r) => r,
            None => {
                self.points.clear();
                self.drawing = false;
                return ToolResponse::None;
            }
        };

        self.points.clear();
        self.drawing = false;
        let cmd = AddNodeCommand::new(parent, element, pres, None);
        ToolResponse::ExecuteCommand(Box::new(cmd))
    }
}

impl Default for PencilTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for PencilTool {
    fn name(&self) -> &str {
        "Pencil"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        self.points.clear();
        self.points.push(doc_pt);
        self.drawing = true;
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.drawing {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        // Only record if the point is far enough from the last recorded point
        // to avoid excessive density.
        if let Some(&last) = self.points.last() {
            if doc_pt.distance(last) < self.min_distance / viewport.zoom {
                return ToolResponse::Handled;
            }
        }

        self.points.push(doc_pt);
        ToolResponse::Handled
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.drawing {
            return ToolResponse::None;
        }

        // Record the final point.
        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        if let Some(&last) = self.points.last() {
            if doc_pt.distance(last) >= self.min_distance / viewport.zoom {
                self.points.push(doc_pt);
            }
        }

        self.finalize(doc)
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
            self.points.clear();
            self.drawing = false;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.points.clear();
        self.drawing = false;
    }

    fn on_deactivate(&mut self) {
        self.points.clear();
        self.drawing = false;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

// ---------------------------------------------------------------------------
// Ramer-Douglas-Peucker path simplification
// ---------------------------------------------------------------------------

/// Simplify a polyline using the Ramer-Douglas-Peucker algorithm.
///
/// Points whose perpendicular distance to the line between the first and last
/// point is less than `epsilon` are removed.
fn rdp_simplify(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    // Find the point with the maximum distance from the line (first..last).
    let first = points[0];
    let last = *points.last().unwrap();
    let mut max_dist = 0.0_f64;
    let mut max_idx = 0;

    for (i, &p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let d = perpendicular_distance(p, first, last);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify.
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        // Remove the duplicate shared point.
        left.pop();
        left.extend(right);
        left
    } else {
        // All intermediate points are within tolerance.
        vec![first, last]
    }
}

/// Perpendicular distance from `point` to the line through `a` and `b`.
fn perpendicular_distance(point: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-12 {
        // a and b are essentially the same point.
        return point.distance(a);
    }

    let numerator = ((dy * point.x) - (dx * point.y) + (b.x * a.y) - (b.y * a.x)).abs();
    numerator / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_setter_round_trips() {
        let mut tool = PencilTool::new();
        tool.set_smoothing(0.6);
        assert_eq!(tool.smoothing(), 0.6);
    }

    #[test]
    fn rdp_keeps_endpoints() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.1),
            Point::new(2.0, 0.0),
        ];
        let simplified = rdp_simplify(&pts, 0.5);
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0], pts[0]);
        assert_eq!(simplified[1], pts[2]);
    }

    #[test]
    fn rdp_preserves_sharp_corner() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 10.0),
            Point::new(10.0, 0.0),
        ];
        let simplified = rdp_simplify(&pts, 0.5);
        assert_eq!(simplified.len(), 3);
    }

    #[test]
    fn build_path_two_points() {
        let mut tool = PencilTool::new();
        tool.points = vec![Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        let path = tool.build_smooth_path();
        assert!(!path.is_empty());
        // Should contain MoveTo + LineTo (2 elements).
        assert_eq!(path.element_count(), 2);
    }

    #[test]
    fn build_path_many_points_produces_curves() {
        let mut tool = PencilTool::new();
        // Generate a quarter-circle worth of raw points.
        for i in 0..50 {
            let t = i as f64 / 49.0;
            let angle = t * std::f64::consts::FRAC_PI_2;
            tool.points
                .push(Point::new(100.0 * angle.cos(), 100.0 * angle.sin()));
        }
        let path = tool.build_smooth_path();
        assert!(!path.is_empty());
        // Should have been simplified and contain curves.
        let elems = path.elements();
        let has_curve = elems
            .iter()
            .any(|e| matches!(e, sivgo_math::PathSegment::CurveTo(..)));
        assert!(
            has_curve,
            "expected cubic bezier segments in the output path"
        );
    }

    #[test]
    fn smoothing_affects_simplification() {
        let mut tool = PencilTool::new();
        // Generate a slightly noisy line.
        for i in 0..20 {
            let x = i as f64 * 5.0;
            let y = if i % 2 == 0 { 1.0 } else { -1.0 };
            tool.points.push(Point::new(x, y));
        }

        // Low smoothing should keep more detail.
        tool.set_smoothing(0.0);
        let path_low = tool.build_smooth_path();

        // High smoothing should simplify more.
        tool.set_smoothing(1.0);
        let path_high = tool.build_smooth_path();

        assert!(
            path_high.element_count() <= path_low.element_count(),
            "higher smoothing should produce fewer (or equal) segments: low={}, high={}",
            path_low.element_count(),
            path_high.element_count(),
        );
    }
}
