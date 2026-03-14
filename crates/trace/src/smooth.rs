use sivgo_math::{BezPath, Point};

use crate::contour::Contour;

/// Convert contour points to a smooth BezPath using cubic Bezier interpolation.
///
/// `smoothing` controls how much the curves are smoothed (0.0 = straight lines, 1.0 = full
/// smoothing). The contour is subsampled to reduce the number of control points, then
/// Catmull-Rom-to-Bezier conversion is applied for smooth curves.
pub fn smooth_contour(contour: &Contour, smoothing: f64) -> BezPath {
    let points = &contour.points;

    if points.is_empty() {
        return BezPath::new();
    }

    if points.len() < 3 || smoothing <= 0.0 {
        return polyline_path(points);
    }

    let smoothing = smoothing.clamp(0.0, 1.0);

    // Subsample: reduce point count for cleaner curves.
    // Take every Nth point where N depends on the total count.
    let subsampled = subsample(points);

    if subsampled.len() < 3 {
        return polyline_path(&subsampled);
    }

    catmull_rom_to_bezpath(&subsampled, smoothing)
}

/// Create a simple polyline (straight lines) from integer coordinates.
fn polyline_path(points: &[(i32, i32)]) -> BezPath {
    let mut path = BezPath::new();
    if let Some(&(x, y)) = points.first() {
        path.move_to(Point::new(x as f64, y as f64));
        for &(x, y) in &points[1..] {
            path.line_to(Point::new(x as f64, y as f64));
        }
        path.close();
    }
    path
}

/// Subsample the contour points to reduce density while preserving shape.
///
/// Uses a combination of uniform subsampling and corner detection.
fn subsample(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let n = points.len();
    if n <= 8 {
        return points.to_vec();
    }

    // Step size: aim for roughly 1 point per 3-5 boundary pixels
    let step = ((n as f64).sqrt() / 2.0).max(2.0).round() as usize;

    let mut result = Vec::new();

    // Always include corners detected by direction change
    let mut i = 0;
    let mut last_included = 0;

    while i < n {
        if i == 0 {
            result.push(points[i]);
            last_included = i;
            i += 1;
            continue;
        }

        // Check if this is a corner
        let is_corner = is_corner_point(points, i);

        // Check if we've gone far enough from the last included point
        let dist_from_last = i - last_included;

        if is_corner || dist_from_last >= step {
            result.push(points[i]);
            last_included = i;
        }

        i += 1;
    }

    // Ensure the contour remains closed: don't duplicate the first point
    // if it's already close to the last one
    if result.len() > 2 {
        let first = result[0];
        let last = result[result.len() - 1];
        let dx = (first.0 - last.0).abs();
        let dy = (first.1 - last.1).abs();
        if dx <= 1 && dy <= 1 {
            result.pop();
        }
    }

    result
}

/// Detect if a point is a "corner" by measuring the angle change.
fn is_corner_point(points: &[(i32, i32)], idx: usize) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }

    let lookback = 3.min(idx);
    let lookahead = 3.min(n - 1 - idx);
    if lookback == 0 || lookahead == 0 {
        return false;
    }

    let prev = points[idx - lookback];
    let curr = points[idx];
    let next = points[idx + lookahead];

    let dx1 = (curr.0 - prev.0) as f64;
    let dy1 = (curr.1 - prev.1) as f64;
    let dx2 = (next.0 - curr.0) as f64;
    let dy2 = (next.1 - curr.1) as f64;

    let len1 = (dx1 * dx1 + dy1 * dy1).sqrt();
    let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();

    if len1 < 0.001 || len2 < 0.001 {
        return false;
    }

    let cos_angle = (dx1 * dx2 + dy1 * dy2) / (len1 * len2);
    // A corner has a sharp angle (cos < 0.5 ≈ 60° turn or sharper)
    cos_angle < 0.5
}

/// Convert a closed sequence of points to a smooth BezPath using Catmull-Rom-to-Bezier conversion.
///
/// The Catmull-Rom spline passes through all control points while maintaining C1 continuity.
/// We convert each Catmull-Rom segment to a cubic Bezier.
fn catmull_rom_to_bezpath(points: &[(i32, i32)], tension: f64) -> BezPath {
    let n = points.len();
    if n < 3 {
        return polyline_path(points);
    }

    let pts: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (x as f64, y as f64)).collect();

    let mut path = BezPath::new();

    // The alpha parameter for Catmull-Rom → Bezier conversion
    // tension=1.0 gives standard Catmull-Rom, 0.0 gives straight lines
    let alpha = tension / 3.0;

    // Move to the first point
    path.move_to(Point::new(pts[0].0, pts[0].1));

    for i in 0..n {
        // For a closed curve, wrap around
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];

        // Catmull-Rom to cubic Bezier control points
        let cp1x = p1.0 + alpha * (p2.0 - p0.0);
        let cp1y = p1.1 + alpha * (p2.1 - p0.1);
        let cp2x = p2.0 - alpha * (p3.0 - p1.0);
        let cp2y = p2.1 - alpha * (p3.1 - p1.1);

        path.curve_to(
            Point::new(cp1x, cp1y),
            Point::new(cp2x, cp2y),
            Point::new(p2.0, p2.1),
        );
    }

    path.close();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::PathSegment;

    #[test]
    fn test_smooth_empty_contour() {
        let contour = Contour {
            points: vec![],
            is_outer: true,
        };
        let path = smooth_contour(&contour, 1.0);
        assert!(path.is_empty());
    }

    #[test]
    fn test_smooth_two_points() {
        let contour = Contour {
            points: vec![(0, 0), (10, 10)],
            is_outer: true,
        };
        let path = smooth_contour(&contour, 1.0);
        assert!(!path.is_empty());
        // Should be a simple polyline (MoveTo + LineTo + Close)
        let elements = path.elements();
        assert!(matches!(elements[0], PathSegment::MoveTo(_)));
        assert!(matches!(elements[1], PathSegment::LineTo(_)));
    }

    #[test]
    fn test_smooth_contour_produces_valid_path() {
        // A square contour
        let contour = Contour {
            points: vec![
                (0, 0),
                (10, 0),
                (20, 0),
                (20, 10),
                (20, 20),
                (10, 20),
                (0, 20),
                (0, 10),
            ],
            is_outer: true,
        };
        let path = smooth_contour(&contour, 1.0);
        assert!(!path.is_empty());

        // Should start with MoveTo
        let elements = path.elements();
        assert!(
            matches!(elements[0], PathSegment::MoveTo(_)),
            "First element should be MoveTo"
        );

        // Should end with ClosePath
        assert!(
            matches!(elements.last(), Some(PathSegment::ClosePath)),
            "Last element should be ClosePath"
        );
    }

    #[test]
    fn test_smooth_contour_zero_smoothing() {
        // With smoothing = 0, should produce straight lines
        let contour = Contour {
            points: vec![(0, 0), (10, 0), (10, 10), (0, 10)],
            is_outer: true,
        };
        let path = smooth_contour(&contour, 0.0);
        let elements = path.elements();
        // Should only have MoveTo, LineTo, and ClosePath — no CurveTo
        for elem in &elements {
            assert!(
                !matches!(elem, PathSegment::CurveTo(..)),
                "Zero smoothing should not produce curves"
            );
        }
    }

    #[test]
    fn test_smooth_contour_full_smoothing() {
        let contour = Contour {
            points: vec![
                (0, 0),
                (5, 0),
                (10, 0),
                (10, 5),
                (10, 10),
                (5, 10),
                (0, 10),
                (0, 5),
            ],
            is_outer: true,
        };
        let path = smooth_contour(&contour, 1.0);
        let elements = path.elements();
        // Should contain CurveTo elements
        let has_curves = elements
            .iter()
            .any(|e| matches!(e, PathSegment::CurveTo(..)));
        assert!(has_curves, "Full smoothing should produce curves");
    }
}
