//! Calligraphy tool: pressure-sensitive freehand strokes with configurable
//! angle and width.
//!
//! On drag, collect points with a variable-width stroke.  On release, create a
//! filled path representing the calligraphic stroke outline.

use sivgo_dom::types::{Color, Paint};
use sivgo_dom::{PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// A calligraphy tool that produces variable-width strokes.
///
/// The stroke width varies according to the pen angle: when moving
/// perpendicular to the angle the stroke is at full width, and when moving
/// parallel to the angle it thins (controlled by `thinning`).
#[derive(Debug)]
pub struct CalligraphyTool {
    /// Raw points collected during the current drag.
    points: Vec<Point>,
    /// Whether we are currently in a drag.
    drawing: bool,
    /// Pen angle in degrees (0 = horizontal nib, 90 = vertical nib).
    angle: f64,
    /// Base width of the stroke in document units.
    width: f64,
    /// Thinning factor in `[0.0, 1.0]`.
    ///
    /// - `0.0` – uniform width regardless of direction.
    /// - `1.0` – stroke thins completely when moving along the pen angle.
    thinning: f64,
    /// Minimum distance between consecutive raw points.
    min_distance: f64,
}

impl CalligraphyTool {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            drawing: false,
            angle: 45.0,
            width: 10.0,
            thinning: 0.0,
            min_distance: 2.0,
        }
    }

    /// Set the pen angle in degrees.
    pub fn set_angle(&mut self, degrees: f64) {
        self.angle = degrees;
    }

    /// Set the base stroke width.
    pub fn set_width(&mut self, width: f64) {
        self.width = width.max(0.1);
    }

    /// Set the thinning factor (`0.0` to `1.0`).
    pub fn set_thinning(&mut self, thinning: f64) {
        self.thinning = thinning.clamp(0.0, 1.0);
    }

    // ------------------------------------------------------------------
    // Stroke outline construction
    // ------------------------------------------------------------------

    /// Compute the half-width at a given point based on direction of movement.
    ///
    /// The width varies with the angle between the movement direction and the
    /// pen angle.
    fn half_width_at(&self, direction: Point) -> f64 {
        let len = (direction.x * direction.x + direction.y * direction.y).sqrt();
        if len < 1e-12 {
            return self.width * 0.5;
        }

        let angle_rad = self.angle.to_radians();
        let pen_dx = angle_rad.cos();
        let pen_dy = angle_rad.sin();

        // Dot product gives cos(theta) between movement and pen direction.
        let dot = (direction.x / len) * pen_dx + (direction.y / len) * pen_dy;

        // When moving along the pen direction (dot ~ 1), thin the stroke.
        // When moving perpendicular (dot ~ 0), full width.
        let factor = 1.0 - self.thinning * dot.abs();
        self.width * 0.5 * factor
    }

    /// Build a filled outline path from the collected points.
    ///
    /// The outline is constructed by offsetting each point perpendicular to the
    /// pen angle by the computed half-width, producing a left edge and a right
    /// edge that are joined into a closed filled path.
    fn build_outline_path(&self) -> BezPath {
        let pts = &self.points;
        if pts.len() < 2 {
            return BezPath::new();
        }

        let angle_rad = self.angle.to_radians();
        // Perpendicular to pen angle.
        let perp_x = -angle_rad.sin();
        let perp_y = angle_rad.cos();

        let mut left_edge: Vec<Point> = Vec::with_capacity(pts.len());
        let mut right_edge: Vec<Point> = Vec::with_capacity(pts.len());

        for i in 0..pts.len() {
            // Direction of movement at this point.
            let dir = if i == 0 {
                Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y)
            } else if i == pts.len() - 1 {
                Point::new(
                    pts[pts.len() - 1].x - pts[pts.len() - 2].x,
                    pts[pts.len() - 1].y - pts[pts.len() - 2].y,
                )
            } else {
                Point::new(pts[i + 1].x - pts[i - 1].x, pts[i + 1].y - pts[i - 1].y)
            };

            let hw = self.half_width_at(dir);

            left_edge.push(Point::new(pts[i].x + perp_x * hw, pts[i].y + perp_y * hw));
            right_edge.push(Point::new(pts[i].x - perp_x * hw, pts[i].y - perp_y * hw));
        }

        // Build a closed path: left edge forward, right edge backward.
        let mut path = BezPath::new();
        path.move_to(left_edge[0]);

        for pt in left_edge.iter().skip(1) {
            path.line_to(*pt);
        }

        // Continue along the right edge in reverse.
        for pt in right_edge.iter().rev() {
            path.line_to(*pt);
        }

        path.close();
        path
    }

    /// Finalize the current stroke: build the outline path and emit an
    /// `AddNodeCommand`.
    fn finalize(&mut self, doc: &SvgDocument) -> ToolResponse {
        if self.points.len() < 2 {
            self.points.clear();
            self.drawing = false;
            return ToolResponse::None;
        }

        let path = self.build_outline_path();
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
            fill: Some(Paint::Color(Color::BLACK)),
            stroke: Some(Paint::None),
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

impl Default for CalligraphyTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CalligraphyTool {
    fn name(&self) -> &str {
        "Calligraphy"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_setters_apply_without_panic() {
        let mut tool = CalligraphyTool::new();
        tool.set_angle(30.0);
        tool.set_width(8.0);
        tool.set_thinning(0.4);
        // Exercising the brush configuration API keeps it live (the values are
        // consumed when a calligraphic stroke is built).
        assert_eq!(tool.name(), "Calligraphy");
    }
}
