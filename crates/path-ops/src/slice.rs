//! Slice operation -- divide a path by cutting it with an infinite line.

use sivgo_math::{BezPath, Point};

use crate::flatten::flatten_path;

/// Split a path at all intersections with an infinite line defined by two points.
///
/// The line extends infinitely in both directions through `line_p0` and `line_p1`.
/// The path is flattened to line segments, intersections with the infinite line
/// are found, and the path is split at those points.
///
/// # Arguments
/// * `path` - The path to be sliced.
/// * `line_p0` - First point defining the slice line.
/// * `line_p1` - Second point defining the slice line.
/// * `tolerance` - Curve flattening tolerance.
///
/// # Returns
/// A `Vec<BezPath>` containing the fragments. If there are no intersections,
/// returns a single-element vec with the original (flattened) path.
pub fn slice(path: &BezPath, line_p0: Point, line_p1: Point, tolerance: f64) -> Vec<BezPath> {
    let contours = flatten_path(path, tolerance);

    if contours.is_empty() {
        return Vec::new();
    }

    // Check that the line is well-defined (two distinct points).
    let ldx = line_p1.x - line_p0.x;
    let ldy = line_p1.y - line_p0.y;
    if ldx * ldx + ldy * ldy < 1e-20 {
        // Degenerate line, return original.
        return contours_to_paths(&contours);
    }

    let line = [line_p0.x, line_p0.y, line_p1.x, line_p1.y];

    let mut all_fragments: Vec<BezPath> = Vec::new();

    for contour in &contours {
        if contour.len() < 2 {
            continue;
        }
        let fragments = split_contour_by_line(contour, &line);
        all_fragments.extend(fragments);
    }

    if all_fragments.is_empty() {
        return contours_to_paths(&contours);
    }

    all_fragments
}

/// An intersection on a contour edge with the slice line.
struct EdgeIntersection {
    edge_idx: usize,
    t: f64,
    point: [f64; 2],
}

/// Split a single flattened contour at intersections with an infinite line.
fn split_contour_by_line(contour: &[[f64; 2]], line: &[f64; 4]) -> Vec<BezPath> {
    let n = contour.len();
    if n < 2 {
        return Vec::new();
    }

    let lx0 = line[0];
    let ly0 = line[1];
    let lx1 = line[2];
    let ly1 = line[3];

    // Find all intersections along the contour's edges with the infinite line.
    let mut intersections: Vec<EdgeIntersection> = Vec::new();

    for i in 0..n - 1 {
        let a1 = contour[i];
        let a2 = contour[i + 1];

        if let Some((t, point)) = segment_infinite_line_intersection(a1, a2, [lx0, ly0], [lx1, ly1])
        {
            if t > 1e-9 && t < 1.0 - 1e-9 {
                intersections.push(EdgeIntersection {
                    edge_idx: i,
                    t,
                    point,
                });
            }
        }
    }

    if intersections.is_empty() {
        let mut path = BezPath::new();
        path.move_to(Point::new(contour[0][0], contour[0][1]));
        for pt in &contour[1..] {
            path.line_to(Point::new(pt[0], pt[1]));
        }
        return vec![path];
    }

    // Sort by edge index, then by t.
    intersections.sort_by(|a, b| {
        a.edge_idx
            .cmp(&b.edge_idx)
            .then_with(|| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Deduplicate near-coincident intersections.
    intersections.dedup_by(|a, b| {
        let dx = a.point[0] - b.point[0];
        let dy = a.point[1] - b.point[1];
        dx * dx + dy * dy < 1e-10
    });

    // Build fragments by walking the contour and splitting at intersections.
    let mut fragments: Vec<BezPath> = Vec::new();
    let mut current = BezPath::new();
    current.move_to(Point::new(contour[0][0], contour[0][1]));

    let mut ix_cursor = 0;

    for i in 0..n - 1 {
        while ix_cursor < intersections.len() && intersections[ix_cursor].edge_idx == i {
            let ix = &intersections[ix_cursor];
            current.line_to(Point::new(ix.point[0], ix.point[1]));
            fragments.push(current);
            current = BezPath::new();
            current.move_to(Point::new(ix.point[0], ix.point[1]));
            ix_cursor += 1;
        }
        current.line_to(Point::new(contour[i + 1][0], contour[i + 1][1]));
    }

    if current.element_count() > 1 {
        fragments.push(current);
    }

    fragments
}

/// Compute the intersection of a finite line segment with an infinite line.
///
/// The segment goes from `a1` to `a2`. The infinite line passes through `b1`
/// and `b2`.
///
/// Returns `Some((t, point))` where `t` is the parameter along the segment
/// (0..1) and `point` is the intersection. Returns `None` if parallel.
fn segment_infinite_line_intersection(
    a1: [f64; 2],
    a2: [f64; 2],
    b1: [f64; 2],
    b2: [f64; 2],
) -> Option<(f64, [f64; 2])> {
    let d1x = a2[0] - a1[0];
    let d1y = a2[1] - a1[1];
    let d2x = b2[0] - b1[0];
    let d2y = b2[1] - b1[1];

    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 1e-12 {
        return None; // parallel
    }

    let dx = b1[0] - a1[0];
    let dy = b1[1] - a1[1];
    let t = (dx * d2y - dy * d2x) / denom;

    // t must be within the segment [0, 1], but u is unconstrained (infinite line).
    if t < -1e-12 || t > 1.0 + 1e-12 {
        return None;
    }

    let t = t.clamp(0.0, 1.0);
    let px = a1[0] + t * d1x;
    let py = a1[1] + t * d1y;
    Some((t, [px, py]))
}

/// Convert flattened contours to individual BezPaths.
fn contours_to_paths(contours: &[Vec<[f64; 2]>]) -> Vec<BezPath> {
    let mut paths = Vec::new();
    for contour in contours {
        if contour.len() < 2 {
            continue;
        }
        let mut path = BezPath::new();
        path.move_to(Point::new(contour[0][0], contour[0][1]));
        for pt in &contour[1..] {
            path.line_to(Point::new(pt[0], pt[1]));
        }
        paths.push(path);
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(x, y));
        path.line_to(Point::new(x + w, y));
        path.line_to(Point::new(x + w, y + h));
        path.line_to(Point::new(x, y + h));
        path.close();
        path
    }

    #[test]
    fn slice_rect_with_vertical_line() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let fragments = slice(&rect, Point::new(5.0, -100.0), Point::new(5.0, 100.0), 0.25);
        assert!(
            fragments.len() >= 2,
            "expected at least 2 fragments, got {}",
            fragments.len()
        );
    }

    #[test]
    fn slice_rect_with_horizontal_line() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let fragments = slice(&rect, Point::new(-100.0, 5.0), Point::new(100.0, 5.0), 0.25);
        assert!(
            fragments.len() >= 2,
            "expected at least 2 fragments, got {}",
            fragments.len()
        );
    }

    #[test]
    fn slice_rect_with_diagonal_line() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        // Use a diagonal that clearly crosses through the interior of the edges
        // (not through corners) to ensure intersection points are found.
        let fragments = slice(&rect, Point::new(-1.0, 4.0), Point::new(11.0, 6.0), 0.25);
        // This line crosses the left edge (x=0) and right edge (x=10) of the rect.
        assert!(
            fragments.len() >= 2,
            "expected at least 2 fragments, got {}",
            fragments.len()
        );
    }

    #[test]
    fn slice_no_intersection() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        // Line entirely above the rect.
        let fragments = slice(
            &rect,
            Point::new(-100.0, -5.0),
            Point::new(100.0, -5.0),
            0.25,
        );
        assert_eq!(fragments.len(), 1, "no intersection => 1 fragment");
    }

    #[test]
    fn slice_empty_path() {
        let empty = BezPath::new();
        let fragments = slice(&empty, Point::new(0.0, 0.0), Point::new(10.0, 10.0), 0.25);
        assert!(fragments.is_empty());
    }

    #[test]
    fn slice_degenerate_line() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        // Two identical points => degenerate line.
        let fragments = slice(&rect, Point::new(5.0, 5.0), Point::new(5.0, 5.0), 0.25);
        // Should return original path.
        assert_eq!(fragments.len(), 1);
    }
}
