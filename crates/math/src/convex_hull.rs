//! Convex hull computation using Andrew's monotone chain algorithm.

use crate::Point;

/// Compute the convex hull of a set of 2D points.
///
/// Returns the hull vertices in counter-clockwise order. If fewer than 3
/// points are given, returns them as-is (after deduplication for collinear
/// cases).
pub fn convex_hull(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }

    // Sort by x, then by y.
    let mut sorted: Vec<Point> = points.to_vec();
    sorted.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Andrew's monotone chain.
    let mut hull: Vec<Point> = Vec::with_capacity(2 * n);

    // Lower hull
    for &p in &sorted {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }

    // Upper hull
    let lower_len = hull.len() + 1;
    for &p in sorted.iter().rev() {
        while hull.len() >= lower_len && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
        {
            hull.pop();
        }
        hull.push(p);
    }

    hull.pop(); // remove the duplicate of the first point
    hull
}

/// 2D cross product of vectors OA and OB (positive if counter-clockwise).
fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_hull() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(0.5, 0.5), // interior point
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4);
    }

    #[test]
    fn collinear_points() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 2); // only endpoints
    }

    #[test]
    fn triangle() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(1.0, 2.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 3);
    }

    #[test]
    fn single_point() {
        let pts = vec![Point::new(5.0, 5.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 1);
    }

    #[test]
    fn empty() {
        let hull = convex_hull(&[]);
        assert!(hull.is_empty());
    }
}
