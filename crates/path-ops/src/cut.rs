//! Cut operation -- split a path at intersections with another path.

use sivgo_math::{BezPath, Point};

use crate::flatten::flatten_path;

/// Split the first path at all intersection points with the second path.
///
/// Both paths are flattened to line segments, intersections are found, and the
/// first path is split into fragments at those intersection points.
///
/// # Arguments
/// * `subject` - The path to be split.
/// * `knife` - The path whose intersections with `subject` define the split points.
/// * `tolerance` - Curve flattening tolerance.
///
/// # Returns
/// A `Vec<BezPath>` containing the fragments of `subject` split at intersection
/// points. If there are no intersections, returns a single-element vec with the
/// original (flattened) path.
pub fn cut(subject: &BezPath, knife: &BezPath, tolerance: f64) -> Vec<BezPath> {
    let subj_contours = flatten_path(subject, tolerance);
    let knife_contours = flatten_path(knife, tolerance);

    if subj_contours.is_empty() {
        return Vec::new();
    }
    if knife_contours.is_empty() {
        // No knife => return subject as-is (flattened).
        return contours_to_paths(&subj_contours);
    }

    // Collect all knife line segments.
    let knife_segments = contours_to_segments(&knife_contours);

    let mut all_fragments: Vec<BezPath> = Vec::new();

    for contour in &subj_contours {
        if contour.len() < 2 {
            continue;
        }
        let fragments = split_contour_at_intersections(contour, &knife_segments);
        all_fragments.extend(fragments);
    }

    if all_fragments.is_empty() {
        // No intersections found, return original flattened contours.
        return contours_to_paths(&subj_contours);
    }

    all_fragments
}

/// A line segment defined by two endpoints.
struct Segment {
    p0: [f64; 2],
    p1: [f64; 2],
}

/// Convert flattened contours into line segments.
fn contours_to_segments(contours: &[Vec<[f64; 2]>]) -> Vec<Segment> {
    let mut segments = Vec::new();
    for contour in contours {
        for i in 0..contour.len().saturating_sub(1) {
            segments.push(Segment {
                p0: contour[i],
                p1: contour[i + 1],
            });
        }
        // Close the contour: add segment from last to first if the contour has
        // at least 3 points (polygon).
        if contour.len() >= 3 {
            let first = contour[0];
            let last = contour[contour.len() - 1];
            let dx = last[0] - first[0];
            let dy = last[1] - first[1];
            if dx * dx + dy * dy > 1e-12 {
                segments.push(Segment {
                    p0: last,
                    p1: first,
                });
            }
        }
    }
    segments
}

/// An intersection on a subject contour edge.
struct EdgeIntersection {
    /// Index of the subject segment (edge from point[idx] to point[idx+1]).
    edge_idx: usize,
    /// Parameter along the subject segment (0..1).
    t: f64,
    /// The intersection point.
    point: [f64; 2],
}

/// Find all intersections of a subject contour with knife segments, then split.
fn split_contour_at_intersections(
    contour: &[[f64; 2]],
    knife_segments: &[Segment],
) -> Vec<BezPath> {
    let n = contour.len();
    if n < 2 {
        return Vec::new();
    }

    // Find all intersections along the subject contour's edges.
    let mut intersections: Vec<EdgeIntersection> = Vec::new();

    for i in 0..n - 1 {
        let a1 = contour[i];
        let a2 = contour[i + 1];

        for seg in knife_segments {
            if let Some((t, point)) = segment_segment_intersection(a1, a2, seg.p0, seg.p1) {
                // Skip intersections at the very start/end of a segment to avoid
                // duplicates at shared vertices.
                if t > 1e-9 && t < 1.0 - 1e-9 {
                    intersections.push(EdgeIntersection {
                        edge_idx: i,
                        t,
                        point,
                    });
                }
            }
        }
    }

    if intersections.is_empty() {
        // No intersections: return the contour as a single path.
        let mut path = BezPath::new();
        path.move_to(Point::new(contour[0][0], contour[0][1]));
        for pt in &contour[1..] {
            path.line_to(Point::new(pt[0], pt[1]));
        }
        return vec![path];
    }

    // Sort intersections by edge index, then by t within the edge.
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
        // Process all intersections on this edge.
        while ix_cursor < intersections.len() && intersections[ix_cursor].edge_idx == i {
            let ix = &intersections[ix_cursor];
            // Add the intersection point and start a new fragment.
            current.line_to(Point::new(ix.point[0], ix.point[1]));
            fragments.push(current);
            current = BezPath::new();
            current.move_to(Point::new(ix.point[0], ix.point[1]));
            ix_cursor += 1;
        }
        // Add the endpoint of this edge.
        current.line_to(Point::new(contour[i + 1][0], contour[i + 1][1]));
    }

    // Push the last fragment if it has content beyond just a MoveTo.
    if current.element_count() > 1 {
        fragments.push(current);
    }

    fragments
}

/// Compute the intersection of two line segments.
///
/// Returns `Some((t, point))` where `t` is the parameter along the first
/// segment and `point` is the intersection location. Returns `None` if the
/// segments are parallel or don't intersect.
fn segment_segment_intersection(
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
    let u = (dx * d1y - dy * d1x) / denom;

    if t < -1e-12 || t > 1.0 + 1e-12 || u < -1e-12 || u > 1.0 + 1e-12 {
        return None; // outside segments
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

    /// Helper: create a rectangle path.
    fn make_rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(x, y));
        path.line_to(Point::new(x + w, y));
        path.line_to(Point::new(x + w, y + h));
        path.line_to(Point::new(x, y + h));
        path.close();
        path
    }

    /// Helper: create a vertical line path.
    fn make_vline(x: f64, y0: f64, y1: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(x, y0));
        path.line_to(Point::new(x, y1));
        path
    }

    #[test]
    fn cut_rect_with_vertical_line() {
        // A rectangle from (0,0) to (10,10) cut by a vertical line at x=5.
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let knife = make_vline(5.0, -5.0, 15.0);
        let fragments = cut(&rect, &knife, 0.25);
        // The vertical line should intersect the top and bottom edges of the rect,
        // producing at least 2 fragments.
        assert!(
            fragments.len() >= 2,
            "expected at least 2 fragments, got {}",
            fragments.len()
        );
    }

    #[test]
    fn cut_with_no_intersection() {
        // Two non-overlapping shapes.
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let knife = make_vline(20.0, 0.0, 10.0);
        let fragments = cut(&rect, &knife, 0.25);
        // No intersection => single fragment per contour.
        assert_eq!(fragments.len(), 1);
    }

    #[test]
    fn cut_empty_subject() {
        let empty = BezPath::new();
        let knife = make_vline(5.0, 0.0, 10.0);
        let fragments = cut(&empty, &knife, 0.25);
        assert!(fragments.is_empty());
    }

    #[test]
    fn cut_empty_knife() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let empty = BezPath::new();
        let fragments = cut(&rect, &empty, 0.25);
        // No knife => return original.
        assert_eq!(fragments.len(), 1);
    }

    #[test]
    fn cut_horizontal_line_through_rect() {
        let rect = make_rect(0.0, 0.0, 10.0, 10.0);
        let mut knife = BezPath::new();
        knife.move_to(Point::new(-5.0, 5.0));
        knife.line_to(Point::new(15.0, 5.0));
        let fragments = cut(&rect, &knife, 0.25);
        assert!(
            fragments.len() >= 2,
            "expected at least 2 fragments, got {}",
            fragments.len()
        );
    }
}
