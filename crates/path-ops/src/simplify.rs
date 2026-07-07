//! Path simplification using the Ramer-Douglas-Peucker algorithm.

use sivgo_math::BezPath;

use crate::flatten::flatten_path;
use crate::refit::polygon_to_path;

/// Simplify a path by removing points that don't contribute significantly
/// to its shape.
///
/// The path is first flattened to line segments, then each contour is
/// simplified using the Ramer-Douglas-Peucker algorithm with the given
/// `tolerance`. Points whose perpendicular distance to the simplified line
/// is less than `tolerance` are removed.
///
/// # Arguments
/// * `path` - The path to simplify.
/// * `tolerance` - Maximum allowed deviation from the original path.
pub fn simplify_path(path: &BezPath, tolerance: f64) -> BezPath {
    let contours = flatten_path(path, tolerance * 0.5); // flatten at finer tolerance than simplify
    let mut simplified: Vec<Vec<[f64; 2]>> = Vec::new();

    for contour in &contours {
        if contour.len() < 3 {
            simplified.push(contour.clone());
            continue;
        }
        let simple = rdp(contour, tolerance);
        if simple.len() >= 3 {
            simplified.push(simple);
        }
    }

    polygon_to_path(&simplified)
}

/// Ramer-Douglas-Peucker algorithm.
///
/// Recursively removes points from a polyline that are within `epsilon`
/// of the simplified line, preserving the overall shape within tolerance.
fn rdp(points: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }

    // Find the point farthest from the line between first and last.
    let first = points[0];
    let last = points[n - 1];
    let mut max_dist = 0.0;
    let mut max_idx = 0;

    for (i, point) in points.iter().enumerate().take(n - 1).skip(1) {
        let d = perpendicular_distance(*point, first, last);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify each sub-segment.
        let mut left = rdp(&points[..=max_idx], epsilon);
        let right = rdp(&points[max_idx..], epsilon);
        // Remove the duplicate point at the junction.
        left.pop();
        left.extend(right);
        left
    } else {
        // All intermediate points are within tolerance -- just keep endpoints.
        vec![first, last]
    }
}

/// Perpendicular distance from point `p` to the line defined by `a` and `b`.
fn perpendicular_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-20 {
        let ex = p[0] - a[0];
        let ey = p[1] - a[1];
        return (ex * ex + ey * ey).sqrt();
    }
    let cross = (p[0] - a[0]) * dy - (p[1] - a[1]) * dx;
    cross.abs() / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    #[test]
    fn simplify_reduces_point_count() {
        // Create a path with a curve that gets flattened to many segments.
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 0.0),
        );
        path.close();

        let flattened = flatten_path(&path, 0.1);
        let orig_points: usize = flattened.iter().map(|c| c.len()).sum();

        let simplified = simplify_path(&path, 2.0);
        let simplified_contours = flatten_path(&simplified, 0.1);
        let simp_points: usize = simplified_contours.iter().map(|c| c.len()).sum();

        assert!(
            simp_points < orig_points,
            "simplified ({}) should have fewer points than original ({})",
            simp_points,
            orig_points
        );
    }

    #[test]
    fn simplify_preserves_rectangle() {
        // A rectangle should not lose points (all corners are essential).
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.line_to(Point::new(0.0, 100.0));
        path.close();

        let simplified = simplify_path(&path, 1.0);
        let contours = flatten_path(&simplified, 0.1);
        // Should still have 4 points for the rectangle.
        assert_eq!(contours.len(), 1);
        assert_eq!(contours[0].len(), 4);
    }

    #[test]
    fn rdp_simple() {
        let points = vec![
            [0.0, 0.0],
            [1.0, 0.1],
            [2.0, -0.1],
            [3.0, 0.0],
            [4.0, 5.0],
            [5.0, 6.0],
            [6.0, 7.0],
        ];
        let result = rdp(&points, 0.5);
        // Should simplify the first near-linear segment.
        assert!(result.len() < points.len());
        // Endpoints should be preserved.
        assert_eq!(result[0], points[0]);
        assert_eq!(*result.last().unwrap(), *points.last().unwrap());
    }
}
