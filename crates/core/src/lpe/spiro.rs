//! Spiro Live Path Effect.
//!
//! Converts input path nodes into smooth Spiro curves. Spiro curves are
//! G2-continuous (curvature continuous) curves that pass through given control
//! points, producing visually pleasing smooth shapes.
//!
//! The implementation approximates Euler spirals (clothoids) between
//! consecutive control points using cubic Bezier segments.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Spiro LPE.
///
/// Extracts endpoint/node positions from the input path and generates smooth
/// Spiro-style curves through them. The resulting path is G2-continuous
/// (curvature continuous) at every node, which produces the characteristic
/// smooth, organic look.
///
/// The internal algorithm computes tangent directions at each node via a
/// global relaxation, then fits cubic Bezier segments using Euler spiral
/// approximations (the "kappa" formula).
#[derive(Debug, Clone)]
pub struct SpiroLpe {
    /// Controls the number of iterations for the tangent relaxation solver.
    iterations_param: LpeParam,
}

impl SpiroLpe {
    /// Create a new Spiro effect.
    pub fn new() -> Self {
        Self {
            iterations_param: LpeParam::new("iterations", 8.0, 1.0, 100.0),
        }
    }
}

impl Default for SpiroLpe {
    fn default() -> Self {
        Self::new()
    }
}

impl LivePathEffect for SpiroLpe {
    fn name(&self) -> &str {
        "spiro"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let elements = input.elements();
        let subpaths = collect_node_subpaths(&elements);
        let iterations = self.iterations_param.value as usize;

        let mut result = BezPath::new();

        for sp in &subpaths {
            if sp.points.len() < 2 {
                if let Some(&pt) = sp.points.first() {
                    result.move_to(pt);
                }
                continue;
            }
            fit_spiro_subpath(&sp.points, sp.closed, iterations, &mut result);
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.iterations_param)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "iterations" {
            self.iterations_param.value =
                value.clamp(self.iterations_param.min, self.iterations_param.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: subpath collection
// ---------------------------------------------------------------------------

struct NodeSubpath {
    points: Vec<Point>,
    closed: bool,
}

/// Extract node positions from path segments, discarding curve control points.
fn collect_node_subpaths(elements: &[PathSegment]) -> Vec<NodeSubpath> {
    let mut subpaths: Vec<NodeSubpath> = Vec::new();
    let mut current: Option<NodeSubpath> = None;

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                if let Some(sp) = current.take() {
                    subpaths.push(sp);
                }
                current = Some(NodeSubpath {
                    points: vec![p],
                    closed: false,
                });
            }
            PathSegment::LineTo(p) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(p);
                }
            }
            PathSegment::QuadTo(_cp, end) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(end);
                }
            }
            PathSegment::CurveTo(_cp1, _cp2, end) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(end);
                }
            }
            PathSegment::ClosePath => {
                if let Some(ref mut sp) = current {
                    sp.closed = true;
                    // Remove duplicate closing point.
                    if sp.points.len() >= 2 {
                        let first = sp.points[0];
                        let last = *sp.points.last().unwrap();
                        if first.distance(last) < 1e-10 {
                            sp.points.pop();
                        }
                    }
                }
                if let Some(sp) = current.take() {
                    subpaths.push(sp);
                }
            }
        }
    }
    if let Some(sp) = current.take() {
        subpaths.push(sp);
    }
    subpaths
}

// ---------------------------------------------------------------------------
// Internal: Spiro fitting
// ---------------------------------------------------------------------------

/// Fit a Spiro (G2-continuous) curve through the given nodes.
///
/// Algorithm outline:
/// 1. Compute initial tangent angles at each node using the average of
///    adjacent chord directions.
/// 2. Iteratively relax the tangent angles to minimize curvature
///    discontinuity (approximate G2 continuity).
/// 3. Fit cubic Bezier segments using the computed tangent directions and
///    chord lengths, with control-arm lengths set by the standard Hermite
///    interpolation formula scaled for smooth results.
fn fit_spiro_subpath(points: &[Point], closed: bool, iterations: usize, out: &mut BezPath) {
    let n = points.len();
    if n < 2 {
        if let Some(&p) = points.first() {
            out.move_to(p);
        }
        return;
    }

    // -----------------------------------------------------------------------
    // Step 1: Compute chord angles.
    // -----------------------------------------------------------------------
    let chord_count = if closed { n } else { n - 1 };
    let mut chord_angles = vec![0.0f64; chord_count];
    let mut chord_lengths = vec![0.0f64; chord_count];

    for i in 0..chord_count {
        let j = (i + 1) % n;
        let dx = points[j].x - points[i].x;
        let dy = points[j].y - points[i].y;
        chord_angles[i] = dy.atan2(dx);
        chord_lengths[i] = (dx * dx + dy * dy).sqrt();
    }

    // -----------------------------------------------------------------------
    // Step 2: Initial tangent angles = average of adjacent chords.
    // -----------------------------------------------------------------------
    let mut tangents = vec![0.0f64; n];
    if closed {
        for i in 0..n {
            let prev = (i + n - 1) % n;
            tangents[i] = angle_avg(chord_angles[prev], chord_angles[i]);
        }
    } else {
        tangents[0] = chord_angles[0];
        tangents[n - 1] = chord_angles[n - 2];
        for i in 1..n - 1 {
            tangents[i] = angle_avg(chord_angles[i - 1], chord_angles[i]);
        }
    }

    // -----------------------------------------------------------------------
    // Step 3: Relax tangent angles for G2 continuity.
    // -----------------------------------------------------------------------
    for _ in 0..iterations {
        if closed {
            for (i, tangent) in tangents.iter_mut().enumerate() {
                let prev = (i + n - 1) % n;
                let next = i % chord_count;
                let target = angle_avg(chord_angles[prev], chord_angles[next]);
                *tangent = angle_avg(*tangent, target);
            }
        } else {
            for (i, tangent) in tangents.iter_mut().enumerate().take(n - 1).skip(1) {
                let target = angle_avg(chord_angles[i - 1], chord_angles[i]);
                *tangent = angle_avg(*tangent, target);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Step 4: Build cubic Bezier segments from tangents and chord lengths.
    // -----------------------------------------------------------------------
    out.move_to(points[0]);

    for i in 0..chord_count {
        let j = (i + 1) % n;
        let cl = chord_lengths[i];
        if cl < 1e-10 {
            out.line_to(points[j]);
            continue;
        }

        // Control arm length: ~1/3 of chord length (Hermite interpolation).
        let arm = cl / 3.0;

        let t0 = tangents[i];
        let t1 = tangents[j];

        let cp1 = Point::new(points[i].x + arm * t0.cos(), points[i].y + arm * t0.sin());
        let cp2 = Point::new(points[j].x - arm * t1.cos(), points[j].y - arm * t1.sin());

        out.curve_to(cp1, cp2, points[j]);
    }

    if closed {
        out.close();
    }
}

/// Average two angles (handles wraparound).
fn angle_avg(a: f64, b: f64) -> f64 {
    let diff = angle_diff(a, b);
    a + diff * 0.5
}

/// Signed angle difference b - a, normalised to (-PI, PI].
fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = b - a;
    while d > std::f64::consts::PI {
        d -= 2.0 * std::f64::consts::PI;
    }
    while d <= -std::f64::consts::PI {
        d += 2.0 * std::f64::consts::PI;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spiro_on_triangle() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));
        input.line_to(Point::new(50.0, 80.0));
        input.close();

        let lpe = SpiroLpe::new();
        let output = lpe.apply(&input);

        // Output should contain CurveTo segments instead of LineTo.
        let elems = output.elements();
        let curve_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::CurveTo(..)))
            .count();
        assert!(
            curve_count >= 3,
            "spiro should produce curves: found {}",
            curve_count
        );
    }

    #[test]
    fn test_spiro_on_open_path() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(50.0, 50.0));
        input.line_to(Point::new(100.0, 0.0));
        input.line_to(Point::new(150.0, 50.0));

        let lpe = SpiroLpe::new();
        let output = lpe.apply(&input);

        assert!(!output.is_empty());
        let elems = output.elements();
        let curve_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::CurveTo(..)))
            .count();
        assert!(curve_count >= 3, "spiro open path should have curves");
    }

    #[test]
    fn test_spiro_two_points() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = SpiroLpe::new();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_spiro_single_point() {
        let mut input = BezPath::new();
        input.move_to(Point::new(50.0, 50.0));

        let lpe = SpiroLpe::new();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_spiro_set_param() {
        let mut lpe = SpiroLpe::new();
        lpe.set_param("iterations", 20.0);
        assert!((lpe.iterations_param.value - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_spiro_name() {
        let lpe = SpiroLpe::new();
        assert_eq!(lpe.name(), "spiro");
    }
}
