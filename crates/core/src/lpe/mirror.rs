//! Mirror Symmetry Live Path Effect.
//!
//! Reflects the input path across a user-defined axis and combines the
//! original with its reflection, producing a symmetric result.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Mirror Symmetry LPE.
///
/// Reflects all points of the input path across an axis line defined by an
/// origin point and an angle, then combines the original path with the
/// reflected copy into a single path.
#[derive(Debug, Clone)]
pub struct MirrorSymmetryLpe {
    /// Origin point of the mirror axis.
    axis_origin: Point,
    /// Angle of the mirror axis in radians (0 = horizontal, PI/2 = vertical).
    axis_angle: LpeParam,
}

impl MirrorSymmetryLpe {
    /// Create a new Mirror Symmetry effect.
    ///
    /// * `axis_origin` - A point on the mirror axis line.
    /// * `axis_angle` - The angle of the axis in radians.
    pub fn new(axis_origin: Point, axis_angle: f64) -> Self {
        Self {
            axis_origin,
            axis_angle: LpeParam::new(
                "axis_angle",
                axis_angle,
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
            ),
        }
    }

    /// Set the axis origin.
    pub fn set_axis_origin(&mut self, origin: Point) {
        self.axis_origin = origin;
    }

    /// Get the axis origin.
    pub fn axis_origin(&self) -> Point {
        self.axis_origin
    }

    /// Reflect a point across the axis line.
    fn reflect_point(&self, p: Point) -> Point {
        let angle = self.axis_angle.value;
        let (sin_a, cos_a) = angle.sin_cos();

        // Direction vector of the axis.
        let dx = cos_a;
        let dy = sin_a;

        // Vector from axis origin to the point.
        let vx = p.x - self.axis_origin.x;
        let vy = p.y - self.axis_origin.y;

        // Project onto the axis direction.
        let dot = vx * dx + vy * dy;

        // Reflected point: p' = 2 * proj - v + origin
        let rx = 2.0 * dot * dx - vx + self.axis_origin.x;
        let ry = 2.0 * dot * dy - vy + self.axis_origin.y;

        Point::new(rx, ry)
    }

    /// Reflect an entire path.
    fn reflect_path(&self, path: &BezPath) -> BezPath {
        let elements = path.elements();
        let mut reflected = BezPath::new();

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    reflected.move_to(self.reflect_point(p));
                }
                PathSegment::LineTo(p) => {
                    reflected.line_to(self.reflect_point(p));
                }
                PathSegment::QuadTo(cp, end) => {
                    reflected.quad_to(self.reflect_point(cp), self.reflect_point(end));
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    reflected.curve_to(
                        self.reflect_point(cp1),
                        self.reflect_point(cp2),
                        self.reflect_point(end),
                    );
                }
                PathSegment::ClosePath => {
                    reflected.close();
                }
            }
        }

        reflected
    }
}

impl LivePathEffect for MirrorSymmetryLpe {
    fn name(&self) -> &str {
        "mirror_symmetry"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let reflected = self.reflect_path(input);

        // Combine original + reflected.
        let mut result = input.clone();
        result.extend(&reflected);
        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.axis_angle)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "axis_angle" {
            self.axis_angle.value = value.clamp(self.axis_angle.min, self.axis_angle.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(10.0, 10.0));
        p.line_to(Point::new(50.0, 10.0));
        p.line_to(Point::new(50.0, 50.0));
        p.line_to(Point::new(10.0, 50.0));
        p.close();
        p
    }

    #[test]
    fn test_mirror_vertical_axis() {
        // Mirror across a vertical axis at x=50.
        let lpe = MirrorSymmetryLpe::new(Point::new(50.0, 0.0), FRAC_PI_2);
        let input = square();
        let output = lpe.apply(&input);

        // Output should have twice the elements of input (original + reflected).
        assert_eq!(
            output.element_count(),
            input.element_count() * 2,
            "mirrored path should have 2x elements"
        );
    }

    #[test]
    fn test_mirror_horizontal_axis() {
        // Mirror across a horizontal axis at y=30.
        let lpe = MirrorSymmetryLpe::new(Point::new(0.0, 30.0), 0.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(output.element_count(), input.element_count() * 2);
    }

    #[test]
    fn test_reflect_point_vertical() {
        let lpe = MirrorSymmetryLpe::new(Point::new(50.0, 0.0), FRAC_PI_2);
        let p = Point::new(30.0, 10.0);
        let r = lpe.reflect_point(p);
        // Reflecting (30, 10) across x=50 vertical gives (70, 10).
        assert!((r.x - 70.0).abs() < 1e-10, "rx={}", r.x);
        assert!((r.y - 10.0).abs() < 1e-10, "ry={}", r.y);
    }

    #[test]
    fn test_reflect_point_horizontal() {
        let lpe = MirrorSymmetryLpe::new(Point::new(0.0, 30.0), 0.0);
        let p = Point::new(10.0, 10.0);
        let r = lpe.reflect_point(p);
        // Reflecting (10, 10) across y=30 horizontal gives (10, 50).
        assert!((r.x - 10.0).abs() < 1e-10, "rx={}", r.x);
        assert!((r.y - 50.0).abs() < 1e-10, "ry={}", r.y);
    }

    #[test]
    fn test_reflect_point_on_axis() {
        let lpe = MirrorSymmetryLpe::new(Point::new(50.0, 0.0), FRAC_PI_2);
        let p = Point::new(50.0, 25.0);
        let r = lpe.reflect_point(p);
        // A point on the axis should map to itself.
        assert!((r.x - 50.0).abs() < 1e-10);
        assert!((r.y - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_empty_path() {
        let lpe = MirrorSymmetryLpe::new(Point::new(0.0, 0.0), 0.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_mirror_params() {
        let mut lpe = MirrorSymmetryLpe::new(Point::new(0.0, 0.0), 0.5);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "axis_angle");
        assert!((params[0].value - 0.5).abs() < 1e-10);

        lpe.set_param("axis_angle", PI);
        assert!((lpe.axis_angle.value - PI).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_name() {
        let lpe = MirrorSymmetryLpe::new(Point::new(0.0, 0.0), 0.0);
        assert_eq!(lpe.name(), "mirror_symmetry");
    }

    #[test]
    fn test_mirror_diagonal() {
        // Mirror across a 45-degree axis through the origin.
        let lpe = MirrorSymmetryLpe::new(Point::new(0.0, 0.0), std::f64::consts::FRAC_PI_4);
        let p = Point::new(10.0, 0.0);
        let r = lpe.reflect_point(p);
        // Reflecting (10, 0) across the line y=x gives (0, 10).
        assert!((r.x - 0.0).abs() < 1e-10, "rx={}", r.x);
        assert!((r.y - 10.0).abs() < 1e-10, "ry={}", r.y);
    }
}
