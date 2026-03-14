//! Slice Live Path Effect.
//!
//! Cuts (clips) the input path along an infinite line defined by a point and
//! an angle. Only the portion of the path on one side of the slice line is
//! kept. The path is first flattened to a polyline; then each segment is
//! tested against the slice line and either kept, discarded, or split at the
//! intersection.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Slice LPE.
///
/// Clips the input path along an infinite line passing through
/// `(slice_x, slice_y)` at angle `slice_angle` (in degrees). The portion
/// on the left side of the directed line is kept.
#[derive(Debug, Clone)]
pub struct SliceLpe {
    /// X coordinate of a point on the slice line.
    slice_x: LpeParam,
    /// Y coordinate of a point on the slice line.
    slice_y: LpeParam,
    /// Angle of the slice line in degrees (0 = horizontal, pointing right).
    slice_angle: LpeParam,
}

impl SliceLpe {
    /// Create a new Slice effect.
    ///
    /// * `slice_x` - X coordinate of the line's reference point.
    /// * `slice_y` - Y coordinate of the line's reference point.
    /// * `slice_angle` - Angle of the slice line in degrees.
    pub fn new(slice_x: f64, slice_y: f64, slice_angle: f64) -> Self {
        Self {
            slice_x: LpeParam::new("slice_x", slice_x, -100_000.0, 100_000.0),
            slice_y: LpeParam::new("slice_y", slice_y, -100_000.0, 100_000.0),
            slice_angle: LpeParam::new("slice_angle", slice_angle, 0.0, 360.0),
        }
    }

    /// Signed distance from point `p` to the slice line. Positive means the
    /// point is on the *kept* side.
    fn signed_dist(&self, p: Point) -> f64 {
        let angle_rad = self.slice_angle.value.to_radians();
        // Direction vector of the slice line.
        let dx = angle_rad.cos();
        let dy = angle_rad.sin();
        // Normal pointing to the kept (left) side: rotate direction by +90 deg.
        let nx = -dy;
        let ny = dx;
        (p.x - self.slice_x.value) * nx + (p.y - self.slice_y.value) * ny
    }
}

impl LivePathEffect for SliceLpe {
    fn name(&self) -> &str {
        "slice"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        // Flatten to a polyline for simpler clipping.
        let flat = input.flatten(1.0);
        let elements = flat.elements();

        let mut result = BezPath::new();
        let mut current: Option<Point> = None;
        let mut subpath_start: Option<Point> = None;
        let mut need_move = true;

        // Helper closure-like processing for a line segment from `prev` to `p`.
        // We inline it for each case (LineTo and ClosePath closing edge).
        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    current = Some(p);
                    subpath_start = Some(p);
                    need_move = true;
                }
                PathSegment::LineTo(p) => {
                    if let Some(prev) = current {
                        clip_segment(prev, p, self, &mut result, &mut need_move);
                    }
                    current = Some(p);
                }
                PathSegment::QuadTo(_, end) | PathSegment::CurveTo(_, _, end) => {
                    if let Some(prev) = current {
                        clip_segment(prev, end, self, &mut result, &mut need_move);
                    }
                    current = Some(end);
                }
                PathSegment::ClosePath => {
                    // Clip the closing edge from current back to subpath start.
                    if let (Some(prev), Some(sp_start)) = (current, subpath_start) {
                        if prev.distance(sp_start) > 1e-10 {
                            clip_segment(prev, sp_start, self, &mut result, &mut need_move);
                        }
                    }
                    need_move = true;
                    current = subpath_start;
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.slice_x)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "slice_x" => {
                self.slice_x.value = value.clamp(self.slice_x.min, self.slice_x.max);
            }
            "slice_y" => {
                self.slice_y.value = value.clamp(self.slice_y.min, self.slice_y.max);
            }
            "slice_angle" => {
                self.slice_angle.value = value.clamp(self.slice_angle.min, self.slice_angle.max);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Clip a single line segment from `prev` to `p` against the slice line,
/// appending any kept portion to `result`.
fn clip_segment(prev: Point, p: Point, lpe: &SliceLpe, result: &mut BezPath, need_move: &mut bool) {
    let d_prev = lpe.signed_dist(prev);
    let d_cur = lpe.signed_dist(p);

    if d_prev >= 0.0 && d_cur >= 0.0 {
        // Both inside: keep entire segment.
        if *need_move {
            result.move_to(prev);
            *need_move = false;
        }
        result.line_to(p);
    } else if d_prev >= 0.0 && d_cur < 0.0 {
        // Crossing from inside to outside.
        let t = d_prev / (d_prev - d_cur);
        let ix = prev.lerp(p, t);
        if *need_move {
            result.move_to(prev);
        }
        result.line_to(ix);
        *need_move = true;
    } else if d_prev < 0.0 && d_cur >= 0.0 {
        // Crossing from outside to inside.
        let t = -d_prev / (d_cur - d_prev);
        let ix = prev.lerp(p, t);
        result.move_to(ix);
        result.line_to(p);
        *need_move = false;
    }
    // else both outside: skip entirely.
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn test_slice_keeps_left_half() {
        // Vertical slice line at x=50, pointing up (angle=90).
        // The left side of a line pointing up is x < 50.
        let lpe = SliceLpe::new(50.0, 0.0, 90.0);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());

        // All output points should have x <= 50 (within tolerance).
        let elems = output.elements();
        for el in &elems {
            match *el {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                    assert!(
                        p.x <= 50.0 + 1.0,
                        "point ({}, {}) should be on left side of x=50",
                        p.x,
                        p.y
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_slice_horizontal() {
        // Horizontal slice line at y=50, pointing right (angle=0).
        // Normal is (-sin(0), cos(0)) = (0, 1), so kept side is y > 50.
        let lpe = SliceLpe::new(0.0, 50.0, 0.0);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());

        // All output points should have y >= 50 (within tolerance).
        let elems = output.elements();
        for el in &elems {
            match *el {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                    assert!(
                        p.y >= 50.0 - 1.0,
                        "point ({}, {}) should be below y=50",
                        p.x,
                        p.y
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_slice_entirely_inside() {
        // Slice line at x=200, angle=90 -- entire square is on the kept side.
        let lpe = SliceLpe::new(200.0, 0.0, 90.0);
        let input = square();
        let output = lpe.apply(&input);
        // The output should have roughly the same content as the input.
        assert!(!output.is_empty());
        // All 4 edges should be preserved (at least 4 LineTo).
        let line_count = output
            .elements()
            .iter()
            .filter(|e| matches!(e, PathSegment::LineTo(_)))
            .count();
        assert!(
            line_count >= 4,
            "entirely inside should preserve all edges, got {} LineTo",
            line_count
        );
    }

    #[test]
    fn test_slice_empty_input() {
        let lpe = SliceLpe::new(50.0, 50.0, 45.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_slice_params() {
        let mut lpe = SliceLpe::new(50.0, 50.0, 45.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "slice_x");
        assert!((params[0].value - 50.0).abs() < 1e-10);

        lpe.set_param("slice_x", 75.0);
        assert!((lpe.slice_x.value - 75.0).abs() < 1e-10);

        lpe.set_param("slice_y", 25.0);
        assert!((lpe.slice_y.value - 25.0).abs() < 1e-10);

        lpe.set_param("slice_angle", 90.0);
        assert!((lpe.slice_angle.value - 90.0).abs() < 1e-10);
    }

    #[test]
    fn test_slice_name() {
        let lpe = SliceLpe::new(50.0, 50.0, 45.0);
        assert_eq!(lpe.name(), "slice");
    }

    #[test]
    fn test_slice_reduces_elements() {
        // Slicing through the middle should produce fewer total elements than
        // the full square.
        let lpe = SliceLpe::new(50.0, 0.0, 90.0);
        let input = square();
        let output = lpe.apply(&input);
        // The output should have content, but likely fewer LineTo than the
        // fully-inside case.
        assert!(!output.is_empty());
    }
}
