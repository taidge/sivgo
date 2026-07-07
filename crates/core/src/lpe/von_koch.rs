//! Von Koch Live Path Effect.
//!
//! Applies the Koch snowflake fractal subdivision to every line segment in
//! the input path. Each straight segment is replaced by the classic Koch
//! curve pattern (equilateral triangle bump), and this is repeated for the
//! requested number of iterations.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Von Koch LPE.
///
/// Replaces each line segment in the input path with a Koch curve pattern,
/// applied recursively for the configured number of iterations.
#[derive(Debug, Clone)]
pub struct VonKochLpe {
    /// Number of recursive iterations (1-6).
    iterations: LpeParam,
    /// Scale factor applied to the equilateral triangle bump (1.0 = standard
    /// Koch curve).
    scale: LpeParam,
}

impl VonKochLpe {
    /// Create a new Von Koch effect.
    ///
    /// * `iterations` - Number of recursive subdivisions (clamped to 1..6).
    /// * `scale` - Relative height of the triangular bump (1.0 = standard).
    pub fn new(iterations: u32, scale: f64) -> Self {
        Self {
            iterations: LpeParam::new("iterations", iterations as f64, 1.0, 6.0),
            scale: LpeParam::new("scale", scale, 0.01, 10.0),
        }
    }
}

impl LivePathEffect for VonKochLpe {
    fn name(&self) -> &str {
        "von_koch"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let iters = self.iterations.value as u32;
        let scale = self.scale.value;

        // Flatten the input to line segments.
        let flat = input.flatten(1.0);
        let elements = flat.elements();

        // Collect segments as pairs of points, respecting subpath structure.
        let mut result = BezPath::new();
        let mut current_pos: Option<Point> = None;
        let mut subpath_start: Option<Point> = None;

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    current_pos = Some(p);
                    subpath_start = Some(p);
                    result.move_to(p);
                }
                PathSegment::LineTo(p) => {
                    if let Some(start) = current_pos {
                        // Replace this segment with a Koch subdivision.
                        let koch_pts = koch_subdivide(start, p, iters, scale);
                        // Skip the first point (it is the current position).
                        for point in koch_pts.iter().skip(1) {
                            result.line_to(*point);
                        }
                    }
                    current_pos = Some(p);
                }
                PathSegment::ClosePath => {
                    // Close back to the subpath start with Koch subdivision.
                    if let (Some(start), Some(sp_start)) = (current_pos, subpath_start) {
                        if start.distance(sp_start) > 1e-10 {
                            let koch_pts = koch_subdivide(start, sp_start, iters, scale);
                            for point in koch_pts.iter().skip(1) {
                                result.line_to(*point);
                            }
                        }
                    }
                    result.close();
                    current_pos = subpath_start;
                }
                PathSegment::QuadTo(_, end) | PathSegment::CurveTo(_, _, end) => {
                    if let Some(start) = current_pos {
                        let koch_pts = koch_subdivide(start, end, iters, scale);
                        for point in koch_pts.iter().skip(1) {
                            result.line_to(*point);
                        }
                    }
                    current_pos = Some(end);
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.iterations)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "iterations" => {
                self.iterations.value = value.clamp(self.iterations.min, self.iterations.max);
            }
            "scale" => {
                self.scale.value = value.clamp(self.scale.min, self.scale.max);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively subdivide the segment from `a` to `b` using the Koch pattern.
/// Returns the list of points along the subdivided curve (including `a` and
/// `b`).
fn koch_subdivide(a: Point, b: Point, iterations: u32, scale: f64) -> Vec<Point> {
    if iterations == 0 {
        return vec![a, b];
    }

    // Split segment into thirds.
    let p1 = a.lerp(b, 1.0 / 3.0);
    let p3 = a.lerp(b, 2.0 / 3.0);

    // The apex of the equilateral triangle bump.
    let mid = a.lerp(b, 0.5);
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    // Perpendicular direction, scaled by sqrt(3)/6 for an equilateral triangle
    // of side length = 1/3 of segment length, then multiplied by `scale`.
    let h = (3.0_f64).sqrt() / 6.0 * scale;
    let p2 = Point::new(mid.x - dy * h, mid.y + dx * h);

    // Recursively subdivide each of the four sub-segments.
    let next = iterations - 1;
    let mut pts = koch_subdivide(a, p1, next, scale);
    pts.pop(); // remove duplicate junction point
    let mut seg2 = koch_subdivide(p1, p2, next, scale);
    pts.append(&mut seg2);
    pts.pop();
    let mut seg3 = koch_subdivide(p2, p3, next, scale);
    pts.append(&mut seg3);
    pts.pop();
    let mut seg4 = koch_subdivide(p3, b, next, scale);
    pts.append(&mut seg4);

    pts
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(50.0, 86.6));
        p.close();
        p
    }

    fn line_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p
    }

    #[test]
    fn test_koch_one_iteration_on_line() {
        let lpe = VonKochLpe::new(1, 1.0);
        let input = line_path();
        let output = lpe.apply(&input);
        // One iteration turns 1 segment into 4 segments => 5 points (1 MoveTo + 4 LineTo).
        assert_eq!(
            output.element_count(),
            5,
            "1 iteration should produce 5 elements, got {}",
            output.element_count()
        );
    }

    #[test]
    fn test_koch_two_iterations_more_segments() {
        let lpe1 = VonKochLpe::new(1, 1.0);
        let lpe2 = VonKochLpe::new(2, 1.0);
        let input = line_path();
        let out1 = lpe1.apply(&input);
        let out2 = lpe2.apply(&input);
        assert!(
            out2.element_count() > out1.element_count(),
            "2 iterations ({}) should have more elements than 1 ({})",
            out2.element_count(),
            out1.element_count()
        );
    }

    #[test]
    fn test_koch_on_triangle() {
        let lpe = VonKochLpe::new(2, 1.0);
        let input = triangle();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // A triangle has 3 edges; each gets Koch subdivision.
        assert!(
            output.element_count() > input.element_count(),
            "Koch on triangle should add many segments"
        );
    }

    #[test]
    fn test_koch_empty_input() {
        let lpe = VonKochLpe::new(1, 1.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_koch_params() {
        let mut lpe = VonKochLpe::new(3, 1.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "iterations");
        assert!((params[0].value - 3.0).abs() < 1e-10);

        lpe.set_param("iterations", 5.0);
        assert!((lpe.iterations.value - 5.0).abs() < 1e-10);

        lpe.set_param("scale", 0.5);
        assert!((lpe.scale.value - 0.5).abs() < 1e-10);

        // Clamped
        lpe.set_param("iterations", 10.0);
        assert!((lpe.iterations.value - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_koch_name() {
        let lpe = VonKochLpe::new(1, 1.0);
        assert_eq!(lpe.name(), "von_koch");
    }

    #[test]
    fn test_koch_subdivide_helper() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(90.0, 0.0);
        let pts = koch_subdivide(a, b, 1, 1.0);
        // 1 iteration: a -> p1 -> apex -> p3 -> b = 5 points.
        assert_eq!(pts.len(), 5);
        // First and last should be the original endpoints.
        assert!((pts[0].x - 0.0).abs() < 1e-10);
        assert!((pts[4].x - 90.0).abs() < 1e-10);
    }
}
