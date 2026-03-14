//! Simplify Live Path Effect.
//!
//! Reduces path complexity by removing redundant nodes while maintaining the
//! overall shape within a user-controlled tolerance. Internally uses the
//! Ramer-Douglas-Peucker algorithm on flattened segments, then re-fits smooth
//! cubic Bezier curves through the surviving points.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Vector};

/// Simplify LPE.
///
/// Flattens the input path, applies Ramer-Douglas-Peucker simplification to
/// reduce the number of nodes, then re-fits cubic Bezier curves through the
/// remaining points for a smooth result.
#[derive(Debug, Clone)]
pub struct SimplifyLpe {
    /// Maximum allowed deviation from the original path (in user units).
    /// Larger values produce simpler paths with fewer nodes.
    threshold: LpeParam,
}

impl SimplifyLpe {
    /// Create a new Simplify effect.
    ///
    /// * `threshold` - Maximum perpendicular distance a point can deviate
    ///   before it is kept. Higher values remove more nodes.
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold: LpeParam::new("threshold", threshold, 0.0, 100_000.0),
        }
    }
}

impl LivePathEffect for SimplifyLpe {
    fn name(&self) -> &str {
        "simplify"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let threshold = self.threshold.value;
        if threshold <= 0.0 {
            return input.clone();
        }

        let elements = input.elements();
        let subpaths = collect_subpaths(&elements);

        let mut result = BezPath::new();

        for sp in &subpaths {
            if sp.points.len() < 3 {
                // Nothing to simplify.
                emit_points(&sp.points, sp.closed, &mut result);
                continue;
            }

            // Apply Ramer-Douglas-Peucker.
            let simplified = rdp_simplify(&sp.points, threshold);

            if simplified.len() < 2 {
                emit_points(&simplified, sp.closed, &mut result);
                continue;
            }

            // Re-fit with smooth cubic Bezier curves.
            fit_smooth_cubics(&simplified, sp.closed, &mut result);
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.threshold)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "threshold" {
            self.threshold.value = value.clamp(self.threshold.min, self.threshold.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Subpath collection
// ---------------------------------------------------------------------------

struct Subpath {
    points: Vec<Point>,
    closed: bool,
}

fn collect_subpaths(elements: &[PathSegment]) -> Vec<Subpath> {
    let mut subpaths: Vec<Subpath> = Vec::new();
    let mut current: Option<Subpath> = None;

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                if let Some(sp) = current.take() {
                    subpaths.push(sp);
                }
                current = Some(Subpath {
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
// Ramer-Douglas-Peucker simplification
// ---------------------------------------------------------------------------

/// Simplify a polyline using the Ramer-Douglas-Peucker algorithm.
fn rdp_simplify(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;

    rdp_recursive(points, 0, points.len() - 1, epsilon, &mut keep);

    points
        .iter()
        .zip(keep.iter())
        .filter_map(|(&p, &k)| if k { Some(p) } else { None })
        .collect()
}

fn rdp_recursive(points: &[Point], start: usize, end: usize, epsilon: f64, keep: &mut [bool]) {
    if end <= start + 1 {
        return;
    }

    let mut max_dist = 0.0;
    let mut max_idx = start;

    let a = points[start];
    let b = points[end];

    for i in (start + 1)..end {
        let d = perpendicular_distance(points[i], a, b);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        keep[max_idx] = true;
        rdp_recursive(points, start, max_idx, epsilon, keep);
        rdp_recursive(points, max_idx, end, epsilon, keep);
    }
}

/// Perpendicular distance from point `p` to the line defined by `a`-`b`.
fn perpendicular_distance(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let ab_len = ab.length();
    if ab_len < 1e-12 {
        return p.distance(a);
    }
    let ap = p - a;
    (ap.x * ab.y - ap.y * ab.x).abs() / ab_len
}

// ---------------------------------------------------------------------------
// Re-fit smooth cubic Beziers
// ---------------------------------------------------------------------------

/// Fit smooth cubic Bezier segments through the simplified points.
///
/// Uses Catmull-Rom-style tangent estimation: at each interior point the
/// tangent direction is the chord from the previous to the next point, and
/// the control arm length is 1/3 of the distance to the adjacent point.
fn fit_smooth_cubics(points: &[Point], closed: bool, out: &mut BezPath) {
    let n = points.len();
    if n < 2 {
        if let Some(&p) = points.first() {
            out.move_to(p);
        }
        return;
    }

    if n == 2 {
        out.move_to(points[0]);
        out.line_to(points[1]);
        if closed {
            out.close();
        }
        return;
    }

    // Compute tangent directions at each point.
    let seg_count = if closed { n } else { n - 1 };
    let mut tangents = vec![Vector::ZERO; n];

    if closed {
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;
            tangents[i] = (points[next] - points[prev]).normalize();
        }
    } else {
        tangents[0] = (points[1] - points[0]).normalize();
        tangents[n - 1] = (points[n - 1] - points[n - 2]).normalize();
        for i in 1..n - 1 {
            tangents[i] = (points[i + 1] - points[i - 1]).normalize();
        }
    }

    out.move_to(points[0]);

    for i in 0..seg_count {
        let j = (i + 1) % n;
        let dist = points[i].distance(points[j]);
        let arm = dist / 3.0;

        let cp1 = Point::new(
            points[i].x + tangents[i].x * arm,
            points[i].y + tangents[i].y * arm,
        );
        let cp2 = Point::new(
            points[j].x - tangents[j].x * arm,
            points[j].y - tangents[j].y * arm,
        );

        out.curve_to(cp1, cp2, points[j]);
    }

    if closed {
        out.close();
    }
}

/// Emit raw points as a polyline (fallback for tiny subpaths).
fn emit_points(points: &[Point], closed: bool, out: &mut BezPath) {
    if points.is_empty() {
        return;
    }
    out.move_to(points[0]);
    for &p in &points[1..] {
        out.line_to(p);
    }
    if closed {
        out.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a complex polyline.
    fn zigzag() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        for i in 1..=20 {
            let x = i as f64 * 10.0;
            let y = if i % 2 == 0 { 0.0 } else { 1.0 }; // tiny zigzag
            p.line_to(Point::new(x, y));
        }
        p
    }

    #[test]
    fn test_simplify_reduces_elements() {
        let lpe = SimplifyLpe::new(2.0); // threshold > zigzag amplitude of 1.0
        let input = zigzag();
        let output = lpe.apply(&input);

        assert!(
            output.element_count() < input.element_count(),
            "simplify should reduce elements: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_simplify_zero_threshold_is_identity() {
        let lpe = SimplifyLpe::new(0.0);
        let input = zigzag();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_simplify_on_square() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));
        input.line_to(Point::new(100.0, 100.0));
        input.line_to(Point::new(0.0, 100.0));
        input.close();

        let lpe = SimplifyLpe::new(1.0);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_simplify_two_points() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = SimplifyLpe::new(10.0);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_simplify_params() {
        let mut lpe = SimplifyLpe::new(5.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "threshold");
        assert!((params[0].value - 5.0).abs() < 1e-10);

        lpe.set_param("threshold", 20.0);
        assert!((lpe.params()[0].value - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_rdp_simplify_collinear() {
        // Points on a straight line should reduce to two endpoints.
        let points: Vec<Point> = (0..10).map(|i| Point::new(i as f64 * 10.0, 0.0)).collect();
        let simplified = rdp_simplify(&points, 1.0);
        assert_eq!(simplified.len(), 2);
    }

    #[test]
    fn test_name() {
        let lpe = SimplifyLpe::new(1.0);
        assert_eq!(lpe.name(), "simplify");
    }
}
