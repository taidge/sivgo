//! Star tool: click-drag to create an n-pointed star shape.

use std::f64::consts::PI;

use sivgo_dom::{PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// The star creation tool.
///
/// Click-drag to create a star centered at the mouse-down point, with the
/// outer radius determined by the distance to the current mouse position.
#[derive(Debug)]
pub struct StarTool {
    /// Center point in document space (recorded on mouse down).
    start: Option<Point>,
    /// Whether we are actively dragging.
    dragging: bool,
    /// Number of points on the star.
    pub num_points: u32,
    /// Ratio of inner radius to outer radius (0.0 .. 1.0).
    pub inner_ratio: f64,
}

impl StarTool {
    pub fn new() -> Self {
        Self {
            start: None,
            dragging: false,
            num_points: 5,
            inner_ratio: 0.4,
        }
    }

    /// Create a star tool with custom parameters.
    pub fn with_params(num_points: u32, inner_ratio: f64) -> Self {
        Self {
            start: None,
            dragging: false,
            num_points: num_points.max(3),
            inner_ratio: inner_ratio.clamp(0.05, 0.95),
        }
    }
}

impl Default for StarTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a `BezPath` for an n-pointed star.
///
/// The star is centered at `center` with outer radius `outer_r` and inner
/// radius `inner_r`. Points alternate between outer and inner radii.
/// The first outer point is at the top (12 o'clock position, i.e. -PI/2).
fn build_star_path(center: Point, outer_r: f64, inner_r: f64, num_points: u32) -> BezPath {
    let n = num_points as usize;
    let total_vertices = n * 2;
    let angle_step = PI / n as f64;
    // Start angle: top of the star (12 o'clock)
    let start_angle = -PI / 2.0;

    let mut path = BezPath::new();

    for i in 0..total_vertices {
        let angle = start_angle + (i as f64) * angle_step;
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        let x = center.x + r * angle.cos();
        let y = center.y + r * angle.sin();
        let pt = Point::new(x, y);

        if i == 0 {
            path.move_to(pt);
        } else {
            path.line_to(pt);
        }
    }

    path.close();
    path
}

impl Tool for StarTool {
    fn name(&self) -> &str {
        "Star"
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
        self.start = Some(doc_pt);
        self.dragging = true;
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        if self.dragging {
            ToolResponse::Handled
        } else {
            ToolResponse::None
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.dragging {
            return ToolResponse::None;
        }
        self.dragging = false;

        let center = match self.start.take() {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let dx = end.x - center.x;
        let dy = end.y - center.y;
        let outer_r = (dx * dx + dy * dy).sqrt();

        // Don't create a star with negligible radius.
        if outer_r < 0.5 {
            return ToolResponse::None;
        }

        let inner_r = outer_r * self.inner_ratio;
        let bez = build_star_path(center, outer_r, inner_r, self.num_points);

        let element = SvgElement::Path(PathAttrs {
            d: bez,
            ..Default::default()
        });

        let mut pres = PresentationAttrs::default();
        pres.fill = Some(Paint::Color(Color::rgb(100, 149, 237))); // cornflower blue
        pres.stroke = Some(Paint::Color(Color::BLACK));
        pres.stroke_width = Some(SvgLength::new(1.0, LengthUnit::None));

        let parent = match doc.root() {
            Some(r) => r,
            None => return ToolResponse::None,
        };

        let cmd = AddNodeCommand::new(parent, element, pres, None);
        ToolResponse::ExecuteCommand(Box::new(cmd))
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

    fn on_activate(&mut self) {
        self.start = None;
        self.dragging = false;
    }

    fn on_deactivate(&mut self) {
        self.start = None;
        self.dragging = false;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_path_has_correct_vertex_count() {
        let center = Point::new(100.0, 100.0);
        let path = build_star_path(center, 50.0, 20.0, 5);
        let elems = path.elements();
        // 1 MoveTo + 9 LineTo + 1 ClosePath = 11 elements for a 5-pointed star
        assert_eq!(elems.len(), 11);
    }

    #[test]
    fn star_path_six_points() {
        let center = Point::new(0.0, 0.0);
        let path = build_star_path(center, 100.0, 40.0, 6);
        let elems = path.elements();
        // 1 MoveTo + 11 LineTo + 1 ClosePath = 13 elements for a 6-pointed star
        assert_eq!(elems.len(), 13);
    }

    #[test]
    fn star_first_point_at_top() {
        let center = Point::new(0.0, 0.0);
        let outer_r = 100.0;
        let path = build_star_path(center, outer_r, 40.0, 5);
        let elems = path.elements();
        // First element should be MoveTo at (0, -100) approximately
        match elems[0] {
            sivgo_math::PathSegment::MoveTo(p) => {
                assert!(p.x.abs() < 1e-10, "x should be ~0, got {}", p.x);
                assert!(
                    (p.y - (-outer_r)).abs() < 1e-10,
                    "y should be ~-100, got {}",
                    p.y
                );
            }
            _ => panic!("expected MoveTo as first element"),
        }
    }
}
