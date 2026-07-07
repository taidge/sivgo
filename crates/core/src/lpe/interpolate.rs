//! Interpolate Live Path Effect.
//!
//! Generates a series of intermediate paths between the input path and a
//! reference path. Each intermediate path is a linear interpolation of the
//! corresponding points from the two paths.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Interpolate LPE.
///
/// Produces `steps` intermediate paths linearly interpolated between the
/// input path and a stored reference path. Both paths are flattened to
/// polylines before interpolation so that corresponding vertices can be
/// paired by index.
#[derive(Debug, Clone)]
pub struct InterpolateLpe {
    /// Number of intermediate paths to generate (excluding the two
    /// endpoints).
    steps: LpeParam,
    /// The reference (target) path to interpolate towards.
    reference_path: BezPath,
}

impl InterpolateLpe {
    /// Create a new Interpolate effect.
    ///
    /// * `steps` - Number of intermediate paths (minimum 1).
    /// * `reference_path` - The target path to interpolate towards.
    pub fn new(steps: u32, reference_path: BezPath) -> Self {
        Self {
            steps: LpeParam::new("steps", steps as f64, 1.0, 1000.0),
            reference_path,
        }
    }

    /// Set the reference path.
    pub fn set_reference_path(&mut self, path: BezPath) {
        self.reference_path = path;
    }

    /// Get a reference to the reference path.
    pub fn reference_path(&self) -> &BezPath {
        &self.reference_path
    }
}

impl LivePathEffect for InterpolateLpe {
    fn name(&self) -> &str {
        "interpolate"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() || self.reference_path.is_empty() {
            return input.clone();
        }

        let steps = self.steps.value as usize;

        // Flatten both paths to polylines.
        let flat_a = input.flatten(1.0);
        let flat_b = self.reference_path.flatten(1.0);

        let pts_a = collect_points(&flat_a.elements());
        let pts_b = collect_points(&flat_b.elements());

        if pts_a.is_empty() || pts_b.is_empty() {
            return input.clone();
        }

        // Resample both point lists to the same length so that
        // corresponding vertices can be paired.
        let count = pts_a.len().max(pts_b.len());
        let resampled_a = resample(&pts_a, count);
        let resampled_b = resample(&pts_b, count);

        let mut result = BezPath::new();

        for step in 1..=steps {
            let t = step as f64 / (steps + 1) as f64;

            // Interpolate each point pair.
            let first = resampled_a[0].lerp(resampled_b[0], t);
            result.move_to(first);

            for i in 1..count {
                let p = resampled_a[i].lerp(resampled_b[i], t);
                result.line_to(p);
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.steps)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "steps" {
            self.steps.value = value.clamp(self.steps.min, self.steps.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Collect all endpoint positions from a flattened path into a single list.
fn collect_points(elements: &[PathSegment]) -> Vec<Point> {
    let mut pts = Vec::new();
    for el in elements {
        match *el {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                pts.push(p);
            }
            PathSegment::QuadTo(_, end) | PathSegment::CurveTo(_, _, end) => {
                pts.push(end);
            }
            PathSegment::ClosePath => {}
        }
    }
    pts
}

/// Resample a point list to exactly `count` points by linear interpolation
/// along the polyline.
fn resample(points: &[Point], count: usize) -> Vec<Point> {
    if points.len() == count {
        return points.to_vec();
    }
    if points.len() < 2 || count < 2 {
        return vec![points[0]; count];
    }

    // Compute cumulative arc lengths.
    let n = points.len();
    let mut dists = Vec::with_capacity(n);
    dists.push(0.0_f64);
    for i in 1..n {
        dists.push(dists[i - 1] + points[i - 1].distance(points[i]));
    }
    let total = *dists.last().unwrap();

    if total < 1e-10 {
        return vec![points[0]; count];
    }

    let mut result = Vec::with_capacity(count);
    let mut seg_idx = 0_usize;

    for i in 0..count {
        let target_d = total * (i as f64) / ((count - 1) as f64);

        // Advance segment index.
        while seg_idx + 1 < n - 1 && dists[seg_idx + 1] < target_d {
            seg_idx += 1;
        }

        let seg_start_d = dists[seg_idx];
        let seg_end_d = dists[seg_idx + 1];
        let seg_len = seg_end_d - seg_start_d;

        if seg_len < 1e-10 {
            result.push(points[seg_idx]);
        } else {
            let t = (target_d - seg_start_d) / seg_len;
            result.push(points[seg_idx].lerp(points[seg_idx + 1], t));
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal_line(y: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, y));
        p.line_to(Point::new(100.0, y));
        p
    }

    fn square_at(y: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, y));
        p.line_to(Point::new(100.0, y));
        p.line_to(Point::new(100.0, y + 100.0));
        p.line_to(Point::new(0.0, y + 100.0));
        p.close();
        p
    }

    #[test]
    fn test_interpolate_produces_intermediate_paths() {
        let ref_path = horizontal_line(100.0);
        let lpe = InterpolateLpe::new(3, ref_path);
        let input = horizontal_line(0.0);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());

        // 3 intermediate paths => 3 MoveTo commands.
        let elems = output.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(
            move_count, 3,
            "expected 3 intermediate paths, got {}",
            move_count
        );
    }

    #[test]
    fn test_interpolate_midpoint() {
        let ref_path = horizontal_line(100.0);
        let lpe = InterpolateLpe::new(1, ref_path);
        let input = horizontal_line(0.0);
        let output = lpe.apply(&input);

        // With 1 step, the single intermediate path should be at y=50.
        let elems = output.elements();
        if let PathSegment::MoveTo(p) = elems[0] {
            assert!(
                (p.y - 50.0).abs() < 1.0,
                "midpoint y should be ~50, got {}",
                p.y
            );
        } else {
            panic!("expected MoveTo as first element");
        }
    }

    #[test]
    fn test_interpolate_empty_input() {
        let ref_path = horizontal_line(100.0);
        let lpe = InterpolateLpe::new(3, ref_path);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_interpolate_empty_reference() {
        let ref_path = BezPath::new();
        let lpe = InterpolateLpe::new(3, ref_path);
        let input = horizontal_line(0.0);
        let output = lpe.apply(&input);
        // Should return input unchanged.
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_interpolate_params() {
        let ref_path = horizontal_line(100.0);
        let mut lpe = InterpolateLpe::new(5, ref_path);
        let params = lpe.params();
        assert_eq!(params[0].name, "steps");
        assert!((params[0].value - 5.0).abs() < 1e-10);

        lpe.set_param("steps", 10.0);
        assert!((lpe.steps.value - 10.0).abs() < 1e-10);

        // Clamped
        lpe.set_param("steps", 5000.0);
        assert!((lpe.steps.value - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_interpolate_name() {
        let ref_path = horizontal_line(100.0);
        let lpe = InterpolateLpe::new(3, ref_path);
        assert_eq!(lpe.name(), "interpolate");
    }

    #[test]
    fn test_interpolate_between_squares() {
        let ref_path = square_at(200.0);
        let lpe = InterpolateLpe::new(2, ref_path);
        let input = square_at(0.0);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // Should produce 2 intermediate paths.
        let elems = output.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(move_count, 2);
    }
}
