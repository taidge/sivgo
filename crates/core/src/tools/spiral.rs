//! Spiral tool: click-drag to create an Archimedean spiral path.

use sivgo_dom::{PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point, Rect};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::drag::{drag_rect, fit_path_to_rect};
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// The spiral creation tool.
///
/// Creates an Archimedean spiral (r = a + b*theta) as a cubic-bezier
/// approximated SVG path inside the dragged rectangle.
#[derive(Debug)]
pub struct SpiralTool {
    /// Center point in document space (mouse-down location).
    start: Option<Point>,
    /// Whether we are actively dragging.
    dragging: bool,
    /// Number of full turns of the spiral.
    pub turns: f64,
    /// Divergence exponent: 1.0 = linear (Archimedean), >1 = accelerating, <1 = decelerating.
    pub divergence: f64,
    /// Inner radius as a fraction of the outer radius (0.0 = starts at center).
    pub inner_radius: f64,
}

impl SpiralTool {
    pub fn new() -> Self {
        Self {
            start: None,
            dragging: false,
            turns: 3.0,
            divergence: 1.0,
            inner_radius: 0.0,
        }
    }
}

impl Default for SpiralTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SpiralTool {
    fn name(&self) -> &str {
        "Spiral"
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

        let start = match self.start.take() {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let bounds = drag_rect(start, end, event.modifiers.ctrl, event.modifiers.shift);

        // Don't create a spiral that is too small.
        if bounds.width() < 1.0 || bounds.height() < 1.0 {
            return ToolResponse::None;
        }

        let path =
            build_spiral_path_in_rect(bounds, self.turns, self.divergence, self.inner_radius);

        let element = SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        });

        let pres = PresentationAttrs {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(Color::rgb(100, 149, 237))), // cornflower blue
            stroke_width: Some(SvgLength::new(1.0, LengthUnit::None)),
            ..Default::default()
        };

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

/// Build a BezPath representing an Archimedean spiral.
///
/// The spiral follows the polar equation `r = a + b * theta^divergence` where:
/// - `a` = `inner_radius * outer_radius` (the starting radius)
/// - The curve grows outward so that at `theta = turns * 2 * PI`, `r = outer_radius`.
/// - `divergence` controls how quickly the radius grows (1.0 = linear growth).
///
/// The spiral is approximated by subdividing each full turn into segments and
/// fitting cubic Bezier curves to each segment.
fn build_spiral_path(
    center: Point,
    outer_radius: f64,
    turns: f64,
    divergence: f64,
    inner_radius_frac: f64,
) -> BezPath {
    let mut path = BezPath::new();

    let inner_r = inner_radius_frac.clamp(0.0, 0.99) * outer_radius;
    let total_angle = turns * std::f64::consts::TAU;

    // Number of cubic bezier segments per full turn.
    // More segments = smoother approximation.
    let segments_per_turn = 12;
    let total_segments = ((turns * segments_per_turn as f64).ceil() as usize).max(1);
    let d_theta = total_angle / total_segments as f64;

    // Compute radius at a given angle via the Archimedean spiral formula.
    // r(theta) = inner_r + (outer_radius - inner_r) * (theta / total_angle)^divergence
    let radius_at = |theta: f64| -> f64 {
        if total_angle < 1e-12 {
            return outer_radius;
        }
        let t = (theta / total_angle).clamp(0.0, 1.0);
        inner_r + (outer_radius - inner_r) * t.powf(divergence)
    };

    // Convert polar to cartesian relative to center.
    let polar_to_xy = |theta: f64, r: f64| -> Point {
        Point::new(center.x + r * theta.cos(), center.y + r * theta.sin())
    };

    // Start at theta = 0
    let r0 = radius_at(0.0);
    let p0 = polar_to_xy(0.0, r0);
    path.move_to(p0);

    for i in 0..total_segments {
        let theta0 = i as f64 * d_theta;
        let theta1 = ((i + 1) as f64 * d_theta).min(total_angle);

        // Use two intermediate parameter values (at 1/3 and 2/3) to construct
        // cubic bezier control points via Hermite-style fitting.
        //
        // For each segment we sample the spiral at the endpoints and compute
        // tangent directions. The cubic bezier control points are placed along
        // the tangent at a distance of (arc_param / 3) from each endpoint,
        // which gives a good cubic approximation for smooth curves.

        let r_0 = radius_at(theta0);
        let r_1 = radius_at(theta1);
        let p_start = polar_to_xy(theta0, r_0);
        let p_end = polar_to_xy(theta1, r_1);

        // Derivative of (r*cos(theta), r*sin(theta)):
        //   dx/dtheta = r'*cos(theta) - r*sin(theta)
        //   dy/dtheta = r'*sin(theta) + r*cos(theta)
        //
        // r'(theta) = (outer_radius - inner_r) * divergence * (theta / total_angle)^(divergence-1) / total_angle
        let dr_at = |theta: f64| -> f64 {
            if total_angle < 1e-12 {
                return 0.0;
            }
            let t = (theta / total_angle).clamp(0.0, 1.0);
            if divergence == 1.0 {
                (outer_radius - inner_r) / total_angle
            } else {
                if t < 1e-15 && divergence < 1.0 {
                    // Avoid division by zero for sub-linear divergence at t=0
                    return 0.0;
                }
                (outer_radius - inner_r) * divergence * t.powf(divergence - 1.0) / total_angle
            }
        };

        let tangent_at = |theta: f64| -> (f64, f64) {
            let r = radius_at(theta);
            let dr = dr_at(theta);
            let dx = dr * theta.cos() - r * theta.sin();
            let dy = dr * theta.sin() + r * theta.cos();
            (dx, dy)
        };

        let seg_angle = theta1 - theta0;
        // Scale factor for control point placement: seg_angle / 3 gives a
        // good cubic approximation of the parametric curve.
        let scale = seg_angle / 3.0;

        let (dx0, dy0) = tangent_at(theta0);
        let (dx1, dy1) = tangent_at(theta1);

        let cp1 = Point::new(p_start.x + dx0 * scale, p_start.y + dy0 * scale);
        let cp2 = Point::new(p_end.x - dx1 * scale, p_end.y - dy1 * scale);

        path.curve_to(cp1, cp2, p_end);
    }

    path
}

fn build_spiral_path_in_rect(
    bounds: Rect,
    turns: f64,
    divergence: f64,
    inner_radius_frac: f64,
) -> BezPath {
    let base = build_spiral_path(
        Point::new(0.0, 0.0),
        1.0,
        turns,
        divergence,
        inner_radius_frac,
    );
    fit_path_to_rect(&base, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_spiral_path_not_empty() {
        let center = Point::new(100.0, 100.0);
        let path = build_spiral_path(center, 50.0, 3.0, 1.0, 0.0);
        assert!(!path.is_empty(), "spiral path should not be empty");
        // Should have move_to + N curve_to segments
        assert!(path.element_count() > 1);
    }

    #[test]
    fn test_build_spiral_with_inner_radius() {
        let center = Point::new(0.0, 0.0);
        let path = build_spiral_path(center, 100.0, 2.0, 1.0, 0.5);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_build_spiral_nonlinear_divergence() {
        let center = Point::new(50.0, 50.0);
        let path = build_spiral_path(center, 80.0, 4.0, 2.0, 0.0);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_spiral_produces_valid_svg() {
        let center = Point::new(100.0, 100.0);
        let path = build_spiral_path(center, 50.0, 3.0, 1.0, 0.0);
        let svg = path.to_svg_string();
        assert!(!svg.is_empty(), "SVG string should not be empty");
        // Should start with M (moveto)
        assert!(
            svg.starts_with('M'),
            "SVG path should start with M, got: {}",
            svg
        );
        // Should contain C (cubic bezier)
        assert!(
            svg.contains('C'),
            "SVG path should contain cubic bezier commands"
        );
    }

    #[test]
    fn test_spiral_small_radius_rejected() {
        // The tool rejects spirals smaller than 1.0 in radius, but the builder
        // itself can handle any positive radius. This test verifies the builder
        // works with a very small radius.
        let center = Point::new(0.0, 0.0);
        let path = build_spiral_path(center, 0.5, 1.0, 1.0, 0.0);
        assert!(!path.is_empty());
    }

    #[test]
    fn spiral_path_in_rect_matches_drag_bounds() {
        let bounds = Rect::new(10.0, 20.0, 90.0, 70.0);
        let path = build_spiral_path_in_rect(bounds, 3.0, 1.0, 0.0);
        let actual = path.bounding_box();

        assert!((actual.x0 - bounds.x0).abs() < 1e-6);
        assert!((actual.y0 - bounds.y0).abs() < 1e-6);
        assert!((actual.x1 - bounds.x1).abs() < 1e-6);
        assert!((actual.y1 - bounds.y1).abs() < 1e-6);
    }
}
