//! Path offset (inset/outset) by moving line segments parallel to themselves.

use sivgo_math::BezPath;

use crate::flatten::flatten_path;
use crate::refit::polygon_to_path;

/// Offset a path by the given distance.
///
/// Positive `distance` produces an outset (path grows), negative produces an
/// inset (path shrinks). The path is first flattened to line segments, each
/// segment is offset perpendicular to its direction, and corner joins are
/// computed using a simple miter approach.
///
/// # Arguments
/// * `path` - The path to offset.
/// * `distance` - Offset distance (positive = outset, negative = inset).
/// * `tolerance` - Curve flattening tolerance.
pub fn offset_path(path: &BezPath, distance: f64, tolerance: f64) -> BezPath {
    if distance.abs() < 1e-12 {
        return path.clone();
    }

    let contours = flatten_path(path, tolerance);
    let mut result_contours: Vec<Vec<[f64; 2]>> = Vec::new();

    for contour in &contours {
        if contour.len() < 3 {
            continue;
        }
        if let Some(offset_contour) = offset_contour(contour, distance) {
            if offset_contour.len() >= 3 {
                result_contours.push(offset_contour);
            }
        }
    }

    polygon_to_path(&result_contours)
}

/// Compute the signed area of a polygon contour (shoelace formula).
///
/// Positive = counter-clockwise in standard math coords (CW in screen coords).
/// Negative = clockwise in standard math coords (CCW in screen coords).
fn signed_area(contour: &[[f64; 2]]) -> f64 {
    let n = contour.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += contour[i][0] * contour[j][1];
        area -= contour[j][0] * contour[i][1];
    }
    area * 0.5
}

/// Offset a single closed contour by the given distance.
///
/// Returns `None` if the contour degenerates to nothing.
fn offset_contour(contour: &[[f64; 2]], distance: f64) -> Option<Vec<[f64; 2]>> {
    let n = contour.len();
    if n < 3 {
        return None;
    }

    // Determine winding direction to ensure the offset normal points outward.
    // For SVG (screen coords, Y-down):
    //   positive signed_area => CCW in math coords => CW on screen (typical SVG outer contour)
    //   negative signed_area => CW in math coords => CCW on screen
    //
    // For an outward offset (positive distance), we want the normal to point
    // away from the polygon interior. For a CW screen contour (positive signed_area),
    // the outward normal for edge (dx, dy) is (dy, -dx) (i.e., to the right of the edge direction).
    let area = signed_area(contour);
    // If area > 0 (CW on screen), outward normal is (dy, -dx).
    // If area < 0 (CCW on screen), outward normal is (-dy, dx).
    // We multiply distance by the sign to handle both cases uniformly.
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };

    // Compute offset edges (each edge shifted perpendicular by `distance`).
    let mut offset_edges: Vec<([f64; 2], [f64; 2])> = Vec::with_capacity(n);

    for i in 0..n {
        let j = (i + 1) % n;
        let dx = contour[j][0] - contour[i][0];
        let dy = contour[j][1] - contour[i][1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            offset_edges.push((contour[i], contour[j]));
            continue;
        }
        // Outward normal, adjusted for winding direction.
        let nx = sign * dy / len * distance;
        let ny = sign * -dx / len * distance;

        offset_edges.push((
            [contour[i][0] + nx, contour[i][1] + ny],
            [contour[j][0] + nx, contour[j][1] + ny],
        ));
    }

    // Compute intersection of consecutive offset edges to get the offset vertices.
    let mut result: Vec<[f64; 2]> = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i == 0 { n - 1 } else { i - 1 };
        let (a1, a2) = offset_edges[prev];
        let (b1, b2) = offset_edges[i];

        if let Some(pt) = line_line_intersection(a1, a2, b1, b2) {
            result.push(pt);
        } else {
            // Parallel edges -- just use the start of the current offset edge.
            result.push(b1);
        }
    }

    if result.len() >= 3 {
        Some(result)
    } else {
        None
    }
}

/// Compute the intersection of two infinite lines, each defined by two points.
///
/// Returns `None` if the lines are parallel or coincident.
fn line_line_intersection(
    a1: [f64; 2],
    a2: [f64; 2],
    b1: [f64; 2],
    b2: [f64; 2],
) -> Option<[f64; 2]> {
    let d1x = a2[0] - a1[0];
    let d1y = a2[1] - a1[1];
    let d2x = b2[0] - b1[0];
    let d2y = b2[1] - b1[1];

    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 1e-12 {
        return None;
    }

    let t = ((b1[0] - a1[0]) * d2y - (b1[1] - a1[1]) * d2x) / denom;
    Some([a1[0] + t * d1x, a1[1] + t * d1y])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

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
    fn offset_zero_returns_same_bbox() {
        let rect = make_rect(10.0, 10.0, 100.0, 50.0);
        let result = offset_path(&rect, 0.0, 0.25);
        let bb = result.bounding_box();
        let orig_bb = rect.bounding_box();
        assert!((bb.x0 - orig_bb.x0).abs() < 1e-6);
        assert!((bb.y0 - orig_bb.y0).abs() < 1e-6);
        assert!((bb.x1 - orig_bb.x1).abs() < 1e-6);
        assert!((bb.y1 - orig_bb.y1).abs() < 1e-6);
    }

    #[test]
    fn outset_grows_bounding_box() {
        let rect = make_rect(10.0, 10.0, 100.0, 50.0);
        let result = offset_path(&rect, 5.0, 0.25);
        let bb = result.bounding_box();
        // For a simple rectangle, the outset should grow the bbox by approximately
        // the offset distance on each side.
        assert!(bb.x0 < 10.0, "expected x0 < 10, got {}", bb.x0);
        assert!(bb.y0 < 10.0, "expected y0 < 10, got {}", bb.y0);
        assert!(bb.x1 > 110.0, "expected x1 > 110, got {}", bb.x1);
        assert!(bb.y1 > 60.0, "expected y1 > 60, got {}", bb.y1);
    }

    #[test]
    fn inset_shrinks_bounding_box() {
        let rect = make_rect(10.0, 10.0, 100.0, 50.0);
        let result = offset_path(&rect, -5.0, 0.25);
        let bb = result.bounding_box();
        assert!(bb.x0 > 10.0, "expected x0 > 10, got {}", bb.x0);
        assert!(bb.y0 > 10.0, "expected y0 > 10, got {}", bb.y0);
        assert!(bb.x1 < 110.0, "expected x1 < 110, got {}", bb.x1);
        assert!(bb.y1 < 60.0, "expected y1 < 60, got {}", bb.y1);
    }
}
