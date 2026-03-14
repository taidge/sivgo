//! Perspective Envelope Live Path Effect.
//!
//! Distorts the input path using a perspective (projective) transform
//! defined by four destination corner points. Unlike the bilinear
//! [`EnvelopeLpe`], this uses a true perspective mapping via
//! [`PerspectiveTransform`], producing geometrically correct foreshortening.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, PerspectiveTransform, Point};

/// Perspective Envelope LPE.
///
/// Maps the input path's bounding-box quad to a destination quadrilateral
/// using a projective (perspective) transform computed via
/// [`PerspectiveTransform::from_quad_to_quad`].
#[derive(Debug, Clone)]
pub struct PerspectiveEnvelopeLpe {
    /// Destination corners: top-left, top-right, bottom-right, bottom-left.
    corners: [Point; 4],
    /// Dummy param for the trait (controls flattening tolerance for curves).
    tolerance: LpeParam,
}

impl PerspectiveEnvelopeLpe {
    /// Create a new Perspective Envelope effect.
    ///
    /// * `top_left`, `top_right`, `bottom_right`, `bottom_left` - The four
    ///   destination corners in clockwise order.
    pub fn new(top_left: Point, top_right: Point, bottom_right: Point, bottom_left: Point) -> Self {
        Self {
            corners: [top_left, top_right, bottom_right, bottom_left],
            tolerance: LpeParam::new("tolerance", 1.0, 0.01, 100.0),
        }
    }

    /// Get the destination corners `[TL, TR, BR, BL]`.
    pub fn corners(&self) -> &[Point; 4] {
        &self.corners
    }

    /// Set a corner by index (0=TL, 1=TR, 2=BR, 3=BL).
    pub fn set_corner(&mut self, index: usize, pt: Point) {
        if index < 4 {
            self.corners[index] = pt;
        }
    }
}

impl LivePathEffect for PerspectiveEnvelopeLpe {
    fn name(&self) -> &str {
        "perspective_envelope"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let bb = input.bounding_box();
        let w = bb.width();
        let h = bb.height();

        if w < 1e-12 || h < 1e-12 {
            return input.clone();
        }

        // Source quad: the bounding box corners [TL, TR, BR, BL].
        let src = [
            Point::new(bb.x0, bb.y0),
            Point::new(bb.x1, bb.y0),
            Point::new(bb.x1, bb.y1),
            Point::new(bb.x0, bb.y1),
        ];

        let transform = match PerspectiveTransform::from_quad_to_quad(&src, &self.corners) {
            Some(t) => t,
            None => return input.clone(),
        };

        let elements = input.elements();
        let mut result = BezPath::new();

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    let mapped = transform.transform_point(p).unwrap_or(p);
                    result.move_to(mapped);
                }
                PathSegment::LineTo(p) => {
                    let mapped = transform.transform_point(p).unwrap_or(p);
                    result.line_to(mapped);
                }
                PathSegment::QuadTo(cp, end) => {
                    let cp_m = transform.transform_point(cp).unwrap_or(cp);
                    let end_m = transform.transform_point(end).unwrap_or(end);
                    result.quad_to(cp_m, end_m);
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    let cp1_m = transform.transform_point(cp1).unwrap_or(cp1);
                    let cp2_m = transform.transform_point(cp2).unwrap_or(cp2);
                    let end_m = transform.transform_point(end).unwrap_or(end);
                    result.curve_to(cp1_m, cp2_m, end_m);
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.tolerance)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "tolerance" {
            self.tolerance.value = value.clamp(self.tolerance.min, self.tolerance.max);
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
    fn test_perspective_identity() {
        // Destination matches source bbox -- should be identity.
        let lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        let input = square();
        let output = lpe.apply(&input);
        // Should be approximately the same.
        let bb = output.bounding_box();
        assert!((bb.x0 - 0.0).abs() < 1e-6);
        assert!((bb.y0 - 0.0).abs() < 1e-6);
        assert!((bb.x1 - 100.0).abs() < 1e-6);
        assert!((bb.y1 - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_perspective_scale() {
        // Double the size.
        let lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(200.0, 0.0),
            Point::new(200.0, 200.0),
            Point::new(0.0, 200.0),
        );
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!((bb.x1 - 200.0).abs() < 1e-4, "x1={}", bb.x1);
        assert!((bb.y1 - 200.0).abs() < 1e-4, "y1={}", bb.y1);
    }

    #[test]
    fn test_perspective_trapezoid() {
        // Create a perspective effect (narrower top).
        let lpe = PerspectiveEnvelopeLpe::new(
            Point::new(25.0, 0.0),
            Point::new(75.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        let bb = output.bounding_box();
        // The top edge should be narrower than the bottom.
        assert!(bb.x1 > 50.0);
    }

    #[test]
    fn test_perspective_empty_path() {
        let lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_perspective_params() {
        let mut lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "tolerance");

        lpe.set_param("tolerance", 2.0);
        assert!((lpe.tolerance.value - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_perspective_name() {
        let lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        assert_eq!(lpe.name(), "perspective_envelope");
    }

    #[test]
    fn test_perspective_set_corner() {
        let mut lpe = PerspectiveEnvelopeLpe::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        );
        lpe.set_corner(0, Point::new(10.0, 10.0));
        assert_eq!(lpe.corners()[0], Point::new(10.0, 10.0));
    }
}
