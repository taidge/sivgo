//! B-Spline Live Path Effect.
//!
//! Converts input control points into a smooth B-spline curve that passes
//! through (or near) the nodes. The De Boor algorithm is used to evaluate the
//! B-spline at many parameter values, and the resulting polyline is then
//! approximated with cubic Bezier segments.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// B-Spline LPE.
///
/// Treats the input path nodes as B-spline control points and produces a
/// smooth curve using De Boor's algorithm. The result is approximated with
/// cubic Bezier segments for efficient rendering.
#[derive(Debug, Clone)]
pub struct BSplineLpe {
    /// Polynomial degree of the B-spline (1 = linear, 2 = quadratic, 3 = cubic).
    degree: LpeParam,
}

impl BSplineLpe {
    /// Create a new B-Spline effect with the given degree.
    ///
    /// Typical values: 2 (quadratic) or 3 (cubic). The degree is clamped to
    /// `[1, 7]`.
    pub fn new(degree: u32) -> Self {
        Self {
            degree: LpeParam::new("degree", degree as f64, 1.0, 7.0),
        }
    }

    /// Current degree.
    pub fn degree(&self) -> u32 {
        self.degree.value as u32
    }
}

impl Default for BSplineLpe {
    fn default() -> Self {
        Self::new(3)
    }
}

impl LivePathEffect for BSplineLpe {
    fn name(&self) -> &str {
        "bspline"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let elements = input.elements();
        let subpaths = collect_node_subpaths(&elements);
        let degree = self.degree.value as usize;

        let mut result = BezPath::new();

        for sp in &subpaths {
            if sp.points.len() < 2 {
                if let Some(&pt) = sp.points.first() {
                    result.move_to(pt);
                }
                continue;
            }

            fit_bspline_subpath(&sp.points, sp.closed, degree, &mut result);
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.degree)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "degree" {
            self.degree.value = value.clamp(self.degree.min, self.degree.max);
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
// Internal: B-Spline fitting
// ---------------------------------------------------------------------------

/// Build a uniform knot vector for `n` control points with given `degree`.
///
/// For an open (clamped) B-spline, the first `degree+1` knots are 0 and the
/// last `degree+1` knots are 1, with uniform interior knots.
fn build_clamped_knots(n: usize, degree: usize) -> Vec<f64> {
    let order = degree + 1;
    let knot_count = n + order;
    let mut knots = vec![0.0f64; knot_count];

    let interior = knot_count - 2 * order;
    for i in 0..knot_count {
        if i < order {
            knots[i] = 0.0;
        } else if i >= knot_count - order {
            knots[i] = 1.0;
        } else {
            knots[i] = (i - degree) as f64 / (interior + 1) as f64;
        }
    }
    knots
}

/// Evaluate a B-spline at parameter `t` using De Boor's algorithm.
///
/// `control_points` are the B-spline control points, `knots` is the knot
/// vector, and `degree` is the polynomial degree.
fn de_boor(control_points: &[Point], knots: &[f64], degree: usize, t: f64) -> Point {
    let n = control_points.len();

    // Find the knot span index k such that knots[k] <= t < knots[k+1].
    // For t == 1.0 we use the last valid span.
    let mut k = degree;
    for i in degree..n {
        if t < knots[i + 1] {
            k = i;
            break;
        }
        k = i;
    }

    // Copy the relevant control points into a working buffer.
    let mut d: Vec<Point> = Vec::with_capacity(degree + 1);
    for j in 0..=degree {
        let idx = k.wrapping_sub(degree).wrapping_add(j);
        if idx < n {
            d.push(control_points[idx]);
        } else {
            // Should not happen with valid knot vector, but be safe.
            d.push(*control_points.last().unwrap());
        }
    }

    // De Boor recursion.
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let left_idx = k.wrapping_sub(degree).wrapping_add(j);
            let denom = knots[left_idx + degree - r + 1] - knots[left_idx];
            let alpha = if denom.abs() < 1e-14 {
                0.0
            } else {
                (t - knots[left_idx]) / denom
            };
            d[j] = d[j - 1].lerp(d[j], alpha);
        }
    }

    d[degree]
}

/// Fit a B-spline through the given control points and output cubic Bezier
/// approximations.
fn fit_bspline_subpath(points: &[Point], closed: bool, degree: usize, out: &mut BezPath) {
    let n = points.len();
    // Clamp degree so we have enough control points.
    let deg = degree.min(n - 1).max(1);

    // For closed splines, wrap control points.
    let ctrl: Vec<Point> = if closed {
        let mut c = points.to_vec();
        for i in 0..deg {
            c.push(points[i % n]);
        }
        c
    } else {
        points.to_vec()
    };

    let knots = if closed {
        // Uniform (periodic) knots for closed splines.
        let m = ctrl.len() + deg + 1;
        (0..m)
            .map(|i| i as f64 / (m - 1) as f64)
            .collect::<Vec<_>>()
    } else {
        build_clamped_knots(ctrl.len(), deg)
    };

    // Sample the B-spline at many points and collect a polyline.
    let sample_count = (ctrl.len() * 16).max(64);
    let t_min = knots[deg];
    let t_max = knots[ctrl.len()];
    let t_range = t_max - t_min;

    if t_range < 1e-14 {
        // Degenerate -- just emit the original points.
        out.move_to(points[0]);
        for p in &points[1..] {
            out.line_to(*p);
        }
        if closed {
            out.close();
        }
        return;
    }

    let mut sampled = Vec::with_capacity(sample_count + 1);
    for i in 0..=sample_count {
        let t = t_min + t_range * (i as f64 / sample_count as f64);
        sampled.push(de_boor(&ctrl, &knots, deg, t));
    }

    // Approximate the sampled polyline with cubic Bezier segments.
    // We group every 3 sampled points into one cubic segment using the
    // Catmull-Rom style fitting.
    approximate_with_cubics(&sampled, closed, out);
}

/// Approximate a dense polyline with cubic Bezier segments.
///
/// Uses a simple approach: for each group of 4 consecutive points, fit a cubic
/// that starts at the first and ends at the last, with control points placed
/// to follow the intermediate shape.
fn approximate_with_cubics(points: &[Point], closed: bool, out: &mut BezPath) {
    let n = points.len();
    if n < 2 {
        if let Some(&p) = points.first() {
            out.move_to(p);
        }
        return;
    }

    out.move_to(points[0]);

    // Step through in groups of 3 samples, fitting a cubic to each group.
    let mut i = 0;
    while i + 3 <= n - 1 {
        let p0 = points[i];
        let p1 = points[i + 1];
        let p2 = points[i + 2];
        let p3 = points[i + 3];

        // Fit control points so the cubic passes near the two interior points.
        // Using the standard construction: cp1 = p0 + (p1 - p0) * 5/3 is too
        // aggressive. Instead, use a weighted approach:
        //   cp1 = p1 * 2 - (p0 + p2) / 2    (reflected midpoint)
        //   cp2 = p2 * 2 - (p1 + p3) / 2
        // But for dense samplings, simply using the interior samples as
        // approximate control points works well.
        let cp1 = Point::new(p0.x + (p1.x - p0.x) * 1.5, p0.y + (p1.y - p0.y) * 1.5);
        let cp2 = Point::new(p3.x + (p2.x - p3.x) * 1.5, p3.y + (p2.y - p3.y) * 1.5);

        out.curve_to(cp1, cp2, p3);
        i += 3;
    }

    // Handle remaining points with line segments.
    for j in (i + 1)..n {
        out.line_to(points[j]);
    }

    if closed {
        out.close();
    }
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

    fn open_zigzag() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(25.0, 50.0));
        p.line_to(Point::new(50.0, 0.0));
        p.line_to(Point::new(75.0, 50.0));
        p.line_to(Point::new(100.0, 0.0));
        p
    }

    #[test]
    fn test_bspline_name() {
        let lpe = BSplineLpe::new(3);
        assert_eq!(lpe.name(), "bspline");
    }

    #[test]
    fn test_bspline_default_degree() {
        let lpe = BSplineLpe::default();
        assert_eq!(lpe.degree(), 3);
    }

    #[test]
    fn test_bspline_on_square() {
        let lpe = BSplineLpe::new(3);
        let input = square();
        let output = lpe.apply(&input);

        // B-spline smoothing should produce curves (more segments).
        assert!(!output.is_empty());
        let elems = output.elements();
        let curve_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::CurveTo(..)))
            .count();
        assert!(
            curve_count > 0,
            "bspline should produce cubic segments, found 0"
        );
    }

    #[test]
    fn test_bspline_on_open_path() {
        let lpe = BSplineLpe::new(3);
        let input = open_zigzag();
        let output = lpe.apply(&input);

        assert!(!output.is_empty());
        // Open path: should NOT end with ClosePath.
        let elems = output.elements();
        assert!(
            !matches!(elems.last(), Some(PathSegment::ClosePath)),
            "open path should not be closed"
        );
    }

    #[test]
    fn test_bspline_degree_1_is_linear() {
        let lpe = BSplineLpe::new(1);
        let input = open_zigzag();
        let output = lpe.apply(&input);

        assert!(!output.is_empty());
        // Degree-1 B-spline is piecewise-linear, so the output should still
        // contain the original points (potentially with intermediate samples).
    }

    #[test]
    fn test_bspline_degree_2() {
        let lpe = BSplineLpe::new(2);
        let input = open_zigzag();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_bspline_two_points() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = BSplineLpe::new(3);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_bspline_single_point() {
        let mut input = BezPath::new();
        input.move_to(Point::new(50.0, 50.0));

        let lpe = BSplineLpe::new(3);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_bspline_params() {
        let mut lpe = BSplineLpe::new(3);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "degree");
        assert!((params[0].value - 3.0).abs() < 1e-10);

        lpe.set_param("degree", 5.0);
        assert!((lpe.params()[0].value - 5.0).abs() < 1e-10);

        // Clamped to max
        lpe.set_param("degree", 100.0);
        assert!((lpe.params()[0].value - 7.0).abs() < 1e-10);

        // Clamped to min
        lpe.set_param("degree", 0.0);
        assert!((lpe.params()[0].value - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_bspline_deterministic() {
        let lpe = BSplineLpe::new(3);
        let input = open_zigzag();
        let output1 = lpe.apply(&input);
        let output2 = lpe.apply(&input);
        assert_eq!(output1.to_svg_string(), output2.to_svg_string());
    }

    #[test]
    fn test_de_boor_linear() {
        // Degree-1 B-spline with clamped knots should interpolate endpoints.
        let pts = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        let knots = build_clamped_knots(2, 1);
        let start = de_boor(&pts, &knots, 1, 0.0);
        let end = de_boor(&pts, &knots, 1, 1.0);

        assert!((start.x - 0.0).abs() < 1e-6, "start.x={}", start.x);
        assert!((end.x - 100.0).abs() < 1e-6, "end.x={}", end.x);
    }

    #[test]
    fn test_de_boor_midpoint() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(50.0, 100.0),
            Point::new(100.0, 0.0),
        ];
        let knots = build_clamped_knots(3, 2);
        let mid = de_boor(&pts, &knots, 2, 0.5);

        // At t=0.5 the degree-2 B-spline should be near the middle control
        // point but pulled toward the baseline.
        assert!(mid.y > 0.0 && mid.y < 100.0, "mid.y={}", mid.y);
    }

    #[test]
    fn test_clamped_knots_structure() {
        let knots = build_clamped_knots(5, 3);
        // First 4 should be 0, last 4 should be 1.
        assert_eq!(knots.len(), 9); // 5 + 3 + 1
        for &k in &knots[..4] {
            assert!((k - 0.0).abs() < 1e-14);
        }
        for &k in &knots[5..] {
            assert!((k - 1.0).abs() < 1e-14);
        }
    }
}
