//! Envelope Live Path Effect.
//!
//! Distorts the input path using bilinear interpolation between four
//! user-defined corner points, warping the path's bounding box to an
//! arbitrary quadrilateral.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Envelope LPE.
///
/// Maps the input path's bounding box to a quadrilateral defined by four
/// corner points using bilinear interpolation. Each point `(x, y)` in the
/// input is first normalised to `(u, v)` in `[0, 1]^2` relative to the
/// bounding box, then mapped to:
///
/// ```text
/// P(u,v) = (1-u)(1-v)*TL + u*(1-v)*TR + (1-u)*v*BL + u*v*BR
/// ```
#[derive(Debug, Clone)]
pub struct EnvelopeLpe {
    /// Top-left corner of the destination quadrilateral.
    pub top_left: Point,
    /// Top-right corner.
    pub top_right: Point,
    /// Bottom-left corner.
    pub bottom_left: Point,
    /// Bottom-right corner.
    pub bottom_right: Point,
    /// Dummy param for the trait (controls subdivision density for curves).
    subdivisions: LpeParam,
}

impl EnvelopeLpe {
    /// Create a new Envelope effect with four corner points.
    pub fn new(top_left: Point, top_right: Point, bottom_left: Point, bottom_right: Point) -> Self {
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            subdivisions: LpeParam::new("subdivisions", 8.0, 1.0, 100.0),
        }
    }

    /// Create an envelope that matches the input bounding box (identity map).
    pub fn from_rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self::new(
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x0, y1),
            Point::new(x1, y1),
        )
    }

    /// Bilinearly interpolate a normalised (u, v) coordinate.
    fn map_uv(&self, u: f64, v: f64) -> Point {
        let x = (1.0 - u) * (1.0 - v) * self.top_left.x
            + u * (1.0 - v) * self.top_right.x
            + (1.0 - u) * v * self.bottom_left.x
            + u * v * self.bottom_right.x;
        let y = (1.0 - u) * (1.0 - v) * self.top_left.y
            + u * (1.0 - v) * self.top_right.y
            + (1.0 - u) * v * self.bottom_left.y
            + u * v * self.bottom_right.y;
        Point::new(x, y)
    }

    /// Map a point from input space to envelope space.
    fn map_point(&self, p: Point, x0: f64, y0: f64, w: f64, h: f64) -> Point {
        let u = if w.abs() > 1e-12 { (p.x - x0) / w } else { 0.5 };
        let v = if h.abs() > 1e-12 { (p.y - y0) / h } else { 0.5 };
        self.map_uv(u, v)
    }
}

impl LivePathEffect for EnvelopeLpe {
    fn name(&self) -> &str {
        "envelope"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let bb = input.bounding_box();
        let w = bb.width();
        let h = bb.height();

        let elements = input.elements();
        let mut result = BezPath::new();

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    result.move_to(self.map_point(p, bb.x0, bb.y0, w, h));
                }
                PathSegment::LineTo(p) => {
                    result.line_to(self.map_point(p, bb.x0, bb.y0, w, h));
                }
                PathSegment::QuadTo(cp, end) => {
                    result.quad_to(
                        self.map_point(cp, bb.x0, bb.y0, w, h),
                        self.map_point(end, bb.x0, bb.y0, w, h),
                    );
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    result.curve_to(
                        self.map_point(cp1, bb.x0, bb.y0, w, h),
                        self.map_point(cp2, bb.x0, bb.y0, w, h),
                        self.map_point(end, bb.x0, bb.y0, w, h),
                    );
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.subdivisions)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "subdivisions" {
            self.subdivisions.value = value.clamp(self.subdivisions.min, self.subdivisions.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    #[test]
    fn test_envelope_identity() {
        // Envelope matching the input bbox should be (approximately) identity.
        let lpe = EnvelopeLpe::from_rect(0.0, 0.0, 100.0, 100.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_envelope_scale() {
        // Envelope that doubles the size.
        let lpe = EnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(200.0, 0.0),
            Point::new(0.0, 200.0),
            Point::new(200.0, 200.0),
        );
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!((bb.x1 - 200.0).abs() < 1e-6, "x1={}", bb.x1);
        assert!((bb.y1 - 200.0).abs() < 1e-6, "y1={}", bb.y1);
    }

    #[test]
    fn test_envelope_trapezoid() {
        // Narrow the top edge to create a trapezoid.
        let lpe = EnvelopeLpe::new(
            Point::new(25.0, 0.0),
            Point::new(75.0, 0.0),
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
        );
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        // Top edge is narrower: x range starts at 25.
        assert!(bb.x0 < 1.0, "x0={}", bb.x0);
    }

    #[test]
    fn test_envelope_bilinear_center() {
        let lpe = EnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
        );
        // Center of the unit square should map to center of the quad.
        let center = lpe.map_uv(0.5, 0.5);
        assert!((center.x - 50.0).abs() < 1e-10, "cx={}", center.x);
        assert!((center.y - 50.0).abs() < 1e-10, "cy={}", center.y);
    }

    #[test]
    fn test_envelope_corners() {
        let lpe = EnvelopeLpe::new(
            Point::new(10.0, 20.0),
            Point::new(90.0, 20.0),
            Point::new(10.0, 80.0),
            Point::new(90.0, 80.0),
        );
        let tl = lpe.map_uv(0.0, 0.0);
        let tr = lpe.map_uv(1.0, 0.0);
        let bl = lpe.map_uv(0.0, 1.0);
        let br = lpe.map_uv(1.0, 1.0);
        assert!((tl.x - 10.0).abs() < 1e-10);
        assert!((tr.x - 90.0).abs() < 1e-10);
        assert!((bl.y - 80.0).abs() < 1e-10);
        assert!((br.y - 80.0).abs() < 1e-10);
    }

    #[test]
    fn test_envelope_empty_path() {
        let lpe = EnvelopeLpe::from_rect(0.0, 0.0, 100.0, 100.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_envelope_params() {
        let mut lpe = EnvelopeLpe::from_rect(0.0, 0.0, 100.0, 100.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "subdivisions");

        lpe.set_param("subdivisions", 16.0);
        assert!((lpe.subdivisions.value - 16.0).abs() < 1e-10);
    }

    #[test]
    fn test_envelope_name() {
        let lpe = EnvelopeLpe::from_rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(lpe.name(), "envelope");
    }
}
