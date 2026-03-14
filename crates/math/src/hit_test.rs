use crate::{BezPath, Point, Rect};

/// Test whether a point is within `stroke_width / 2` distance of the path's stroke.
///
/// This works by checking the minimum distance from the point to each segment
/// of the path.
pub fn hit_test_stroke(path: &BezPath, point: Point, stroke_width: f64) -> bool {
    let half_width = stroke_width * 0.5;
    let kp = kurbo::Point::from(point);
    let inner = path.to_kurbo();

    // Quick bounding box rejection
    {
        use kurbo::Shape;
        let bb = inner.bounding_box();
        let expanded = bb.inflate(half_width, half_width);
        if !expanded.contains(kp) {
            return false;
        }
    }

    let tolerance = 0.5; // reasonable for hit testing

    for seg in inner.segments() {
        use kurbo::ParamCurveNearest;
        let nearest = seg.nearest(kp, tolerance);
        if nearest.distance_sq <= half_width * half_width {
            return true;
        }
    }

    false
}

/// Test whether a point is inside the fill of a closed path.
///
/// Uses the non-zero winding rule (the standard for SVG fill).
pub fn hit_test_fill(path: &BezPath, point: Point) -> bool {
    use kurbo::Shape;
    let inner = path.to_kurbo();
    let kp = kurbo::Point::from(point);
    inner.winding(kp) != 0
}

/// Compute the bounding box of a path.
///
/// Returns a zero-area rect if the path is empty.
pub fn bounding_box(path: &BezPath) -> Rect {
    path.bounding_box()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(50.0, 100.0));
        p.close();
        p
    }

    fn square_path() -> BezPath {
        BezPath::from_svg_path("M 0 0 L 100 0 L 100 100 L 0 100 Z").unwrap()
    }

    #[test]
    fn test_hit_test_fill_inside() {
        let path = square_path();
        assert!(hit_test_fill(&path, Point::new(50.0, 50.0)));
    }

    #[test]
    fn test_hit_test_fill_outside() {
        let path = square_path();
        assert!(!hit_test_fill(&path, Point::new(150.0, 50.0)));
        assert!(!hit_test_fill(&path, Point::new(-10.0, 50.0)));
    }

    #[test]
    fn test_hit_test_fill_triangle() {
        let path = triangle_path();
        // Center of triangle should be inside
        assert!(hit_test_fill(&path, Point::new(50.0, 30.0)));
        // Outside triangle
        assert!(!hit_test_fill(&path, Point::new(0.0, 100.0)));
    }

    #[test]
    fn test_hit_test_stroke_on_edge() {
        let path = square_path();
        // Point on the top edge
        assert!(hit_test_stroke(&path, Point::new(50.0, 0.0), 2.0));
    }

    #[test]
    fn test_hit_test_stroke_near_edge() {
        let path = square_path();
        // Point slightly outside the top edge, within stroke width
        assert!(hit_test_stroke(&path, Point::new(50.0, -0.5), 2.0));
    }

    #[test]
    fn test_hit_test_stroke_far_away() {
        let path = square_path();
        assert!(!hit_test_stroke(&path, Point::new(200.0, 200.0), 2.0));
    }

    #[test]
    fn test_hit_test_stroke_wide() {
        let path = square_path();
        // Point 5 units away from edge, with stroke width of 12
        assert!(hit_test_stroke(&path, Point::new(50.0, -5.0), 12.0));
        // But not with stroke width of 8
        assert!(!hit_test_stroke(&path, Point::new(50.0, -5.0), 8.0));
    }

    #[test]
    fn test_bounding_box_basic() {
        let path = square_path();
        let bb = bounding_box(&path);
        assert!((bb.x0 - 0.0).abs() < 1e-10);
        assert!((bb.y0 - 0.0).abs() < 1e-10);
        assert!((bb.x1 - 100.0).abs() < 1e-10);
        assert!((bb.y1 - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_bounding_box_empty() {
        let path = BezPath::new();
        let bb = bounding_box(&path);
        // An empty path bounding box should be zero
        assert!(bb.is_empty() || (bb.width() == 0.0 && bb.height() == 0.0));
    }

    #[test]
    fn test_hit_test_stroke_curve() {
        // A curved path
        let path = BezPath::from_svg_path("M 0 0 C 50 100 100 100 150 0").unwrap();
        // The curve passes roughly through (75, 75) area
        // Point near the middle of the curve should be hit with wide enough stroke
        assert!(hit_test_stroke(&path, Point::new(75.0, 74.0), 4.0));
        // A point far from the curve should not hit
        assert!(!hit_test_stroke(&path, Point::new(75.0, 0.0), 4.0));
    }
}
