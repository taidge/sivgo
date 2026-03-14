//! Geometric intersection routines: line-line, line-curve, curve-curve.

use crate::Point;

/// A point where two geometric primitives intersect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intersection {
    /// The intersection point.
    pub point: Point,
    /// Parameter on the first curve (0..1).
    pub t1: f64,
    /// Parameter on the second curve (0..1).
    pub t2: f64,
}

/// Compute the intersection of two line segments.
///
/// Line 1: `a1`→`a2`, Line 2: `b1`→`b2`.
/// Returns `None` if lines are parallel or the intersection lies outside
/// both segments.
pub fn line_line_intersection(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Intersection> {
    let d1x = a2.x - a1.x;
    let d1y = a2.y - a1.y;
    let d2x = b2.x - b1.x;
    let d2y = b2.y - b1.y;

    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 1e-12 {
        return None; // parallel
    }

    let dx = b1.x - a1.x;
    let dy = b1.y - a1.y;
    let t = (dx * d2y - dy * d2x) / denom;
    let u = (dx * d1y - dy * d1x) / denom;

    if t < -1e-12 || t > 1.0 + 1e-12 || u < -1e-12 || u > 1.0 + 1e-12 {
        return None; // outside segments
    }

    let t = t.clamp(0.0, 1.0);
    let u = u.clamp(0.0, 1.0);
    Some(Intersection {
        point: Point::new(a1.x + t * d1x, a1.y + t * d1y),
        t1: t,
        t2: u,
    })
}

/// Find intersections between a line segment and a cubic Bezier curve.
///
/// Uses recursive subdivision with a tolerance-based stopping criterion.
pub fn line_cubic_intersections(
    l0: Point,
    l1: Point,
    c0: Point,
    c1: Point,
    c2: Point,
    c3: Point,
    tolerance: f64,
) -> Vec<Intersection> {
    let mut results = Vec::new();
    line_cubic_recurse(
        l0,
        l1,
        c0,
        c1,
        c2,
        c3,
        0.0,
        1.0,
        tolerance,
        20,
        &mut results,
    );
    results
}

fn line_cubic_recurse(
    l0: Point,
    l1: Point,
    c0: Point,
    c1: Point,
    c2: Point,
    c3: Point,
    t_lo: f64,
    t_hi: f64,
    tolerance: f64,
    depth: u32,
    results: &mut Vec<Intersection>,
) {
    // Bounding box rejection
    let cbb = bezier_bbox(c0, c1, c2, c3);
    let lbb = line_bbox(l0, l1);
    if !bbox_overlap(cbb, lbb, tolerance) {
        return;
    }

    if depth == 0 || (t_hi - t_lo) < 1e-10 {
        // Approximate: intersect the line with the chord c0→c3
        if let Some(ix) = line_line_intersection(l0, l1, c0, c3) {
            let t_curve = t_lo + ix.t2 * (t_hi - t_lo);
            results.push(Intersection {
                point: ix.point,
                t1: ix.t1,
                t2: t_curve,
            });
        }
        return;
    }

    let t_mid = (t_lo + t_hi) * 0.5;
    let (left, right) = crate::subdivide_cubic(c0, c1, c2, c3, 0.5);
    line_cubic_recurse(
        l0,
        l1,
        left.0,
        left.1,
        left.2,
        left.3,
        t_lo,
        t_mid,
        tolerance,
        depth - 1,
        results,
    );
    line_cubic_recurse(
        l0,
        l1,
        right.0,
        right.1,
        right.2,
        right.3,
        t_mid,
        t_hi,
        tolerance,
        depth - 1,
        results,
    );
}

/// Find intersections between a line segment and a quadratic Bezier curve.
pub fn line_quad_intersections(
    l0: Point,
    l1: Point,
    q0: Point,
    q1: Point,
    q2: Point,
    tolerance: f64,
) -> Vec<Intersection> {
    let mut results = Vec::new();
    line_quad_recurse(l0, l1, q0, q1, q2, 0.0, 1.0, tolerance, 20, &mut results);
    results
}

fn line_quad_recurse(
    l0: Point,
    l1: Point,
    q0: Point,
    q1: Point,
    q2: Point,
    t_lo: f64,
    t_hi: f64,
    tolerance: f64,
    depth: u32,
    results: &mut Vec<Intersection>,
) {
    let qbb = quad_bbox(q0, q1, q2);
    let lbb = line_bbox(l0, l1);
    if !bbox_overlap(qbb, lbb, tolerance) {
        return;
    }

    if depth == 0 || (t_hi - t_lo) < 1e-10 {
        if let Some(ix) = line_line_intersection(l0, l1, q0, q2) {
            let t_curve = t_lo + ix.t2 * (t_hi - t_lo);
            results.push(Intersection {
                point: ix.point,
                t1: ix.t1,
                t2: t_curve,
            });
        }
        return;
    }

    let t_mid = (t_lo + t_hi) * 0.5;
    let (left, right) = crate::subdivide_quad(q0, q1, q2, 0.5);
    line_quad_recurse(
        l0,
        l1,
        left.0,
        left.1,
        left.2,
        t_lo,
        t_mid,
        tolerance,
        depth - 1,
        results,
    );
    line_quad_recurse(
        l0,
        l1,
        right.0,
        right.1,
        right.2,
        t_mid,
        t_hi,
        tolerance,
        depth - 1,
        results,
    );
}

/// Find intersections between two cubic Bezier curves.
pub fn cubic_cubic_intersections(
    a0: Point,
    a1: Point,
    a2: Point,
    a3: Point,
    b0: Point,
    b1: Point,
    b2: Point,
    b3: Point,
    tolerance: f64,
) -> Vec<Intersection> {
    let mut results = Vec::new();
    cubic_cubic_recurse(
        a0,
        a1,
        a2,
        a3,
        0.0,
        1.0,
        b0,
        b1,
        b2,
        b3,
        0.0,
        1.0,
        tolerance,
        16,
        &mut results,
    );
    // Deduplicate near-coincident intersections.
    results.dedup_by(|a, b| {
        (a.point.x - b.point.x).abs() < tolerance && (a.point.y - b.point.y).abs() < tolerance
    });
    results
}

fn cubic_cubic_recurse(
    a0: Point,
    a1: Point,
    a2: Point,
    a3: Point,
    at_lo: f64,
    at_hi: f64,
    b0: Point,
    b1: Point,
    b2: Point,
    b3: Point,
    bt_lo: f64,
    bt_hi: f64,
    tolerance: f64,
    depth: u32,
    results: &mut Vec<Intersection>,
) {
    let abb = bezier_bbox(a0, a1, a2, a3);
    let bbb = bezier_bbox(b0, b1, b2, b3);
    if !bbox_overlap(abb, bbb, tolerance) {
        return;
    }

    // Check if bboxes are small enough to approximate with chord intersection.
    let a_diag = ((abb.2 - abb.0).powi(2) + (abb.3 - abb.1).powi(2)).sqrt();
    let b_diag = ((bbb.2 - bbb.0).powi(2) + (bbb.3 - bbb.1).powi(2)).sqrt();
    if depth == 0
        || (a_diag < tolerance && b_diag < tolerance)
        || ((at_hi - at_lo) < 1e-10 && (bt_hi - bt_lo) < 1e-10)
    {
        if let Some(ix) = line_line_intersection(a0, a3, b0, b3) {
            let t1 = at_lo + ix.t1 * (at_hi - at_lo);
            let t2 = bt_lo + ix.t2 * (bt_hi - bt_lo);
            results.push(Intersection {
                point: ix.point,
                t1,
                t2,
            });
        }
        return;
    }

    let at_mid = (at_lo + at_hi) * 0.5;
    let bt_mid = (bt_lo + bt_hi) * 0.5;
    let (al, ar) = crate::subdivide_cubic(a0, a1, a2, a3, 0.5);
    let (bl, br) = crate::subdivide_cubic(b0, b1, b2, b3, 0.5);

    cubic_cubic_recurse(
        al.0,
        al.1,
        al.2,
        al.3,
        at_lo,
        at_mid,
        bl.0,
        bl.1,
        bl.2,
        bl.3,
        bt_lo,
        bt_mid,
        tolerance,
        depth - 1,
        results,
    );
    cubic_cubic_recurse(
        al.0,
        al.1,
        al.2,
        al.3,
        at_lo,
        at_mid,
        br.0,
        br.1,
        br.2,
        br.3,
        bt_mid,
        bt_hi,
        tolerance,
        depth - 1,
        results,
    );
    cubic_cubic_recurse(
        ar.0,
        ar.1,
        ar.2,
        ar.3,
        at_mid,
        at_hi,
        bl.0,
        bl.1,
        bl.2,
        bl.3,
        bt_lo,
        bt_mid,
        tolerance,
        depth - 1,
        results,
    );
    cubic_cubic_recurse(
        ar.0,
        ar.1,
        ar.2,
        ar.3,
        at_mid,
        at_hi,
        br.0,
        br.1,
        br.2,
        br.3,
        bt_mid,
        bt_hi,
        tolerance,
        depth - 1,
        results,
    );
}

// Bounding-box helpers

type Bbox = (f64, f64, f64, f64); // (min_x, min_y, max_x, max_y)

fn bezier_bbox(p0: Point, p1: Point, p2: Point, p3: Point) -> Bbox {
    let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
    let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
    let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
    let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);
    (min_x, min_y, max_x, max_y)
}

fn quad_bbox(p0: Point, p1: Point, p2: Point) -> Bbox {
    let min_x = p0.x.min(p1.x).min(p2.x);
    let min_y = p0.y.min(p1.y).min(p2.y);
    let max_x = p0.x.max(p1.x).max(p2.x);
    let max_y = p0.y.max(p1.y).max(p2.y);
    (min_x, min_y, max_x, max_y)
}

fn line_bbox(p0: Point, p1: Point) -> Bbox {
    (
        p0.x.min(p1.x),
        p0.y.min(p1.y),
        p0.x.max(p1.x),
        p0.y.max(p1.y),
    )
}

fn bbox_overlap(a: Bbox, b: Bbox, margin: f64) -> bool {
    a.0 - margin <= b.2 && a.2 + margin >= b.0 && a.1 - margin <= b.3 && a.3 + margin >= b.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perpendicular_lines() {
        let ix = line_line_intersection(
            Point::new(0.0, 5.0),
            Point::new(10.0, 5.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 10.0),
        )
        .unwrap();
        assert!((ix.point.x - 5.0).abs() < 1e-6);
        assert!((ix.point.y - 5.0).abs() < 1e-6);
        assert!((ix.t1 - 0.5).abs() < 1e-6);
        assert!((ix.t2 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn parallel_lines_no_intersection() {
        let ix = line_line_intersection(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(10.0, 1.0),
        );
        assert!(ix.is_none());
    }

    #[test]
    fn non_overlapping_segments() {
        let ix = line_line_intersection(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(5.0, -1.0),
            Point::new(5.0, 1.0),
        );
        assert!(ix.is_none());
    }

    #[test]
    fn line_cubic_cross() {
        // A horizontal line crossing a simple S-curve
        let results = line_cubic_intersections(
            Point::new(0.0, 50.0),
            Point::new(100.0, 50.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            1.0,
        );
        assert!(!results.is_empty(), "should find at least one intersection");
    }

    #[test]
    fn cubic_cubic_crossing() {
        // Two simple curves that cross
        let results = cubic_cubic_intersections(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
            Point::new(0.0, 100.0),
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            1.0,
        );
        assert!(!results.is_empty(), "should find intersections");
    }
}
