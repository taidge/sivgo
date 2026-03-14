//! BezPath flattening -- convert curves to line segments for polygon operations.

use sivgo_math::{BezPath, PathSegment};

/// Flatten a BezPath into a list of polygon contours.
///
/// Each contour is a `Vec<[f64; 2]>` of points. Curves (quadratic and cubic
/// Bezier segments) are recursively subdivided using de Casteljau until they
/// deviate from a straight line by less than `tolerance`.
pub fn flatten_path(path: &BezPath, tolerance: f64) -> Vec<Vec<[f64; 2]>> {
    let elements = path.elements();
    if elements.is_empty() {
        return Vec::new();
    }

    let mut contours: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    let mut current_pt = [0.0f64; 2];
    let mut subpath_start = [0.0f64; 2];

    for seg in &elements {
        match seg {
            PathSegment::MoveTo(p) => {
                // Finish previous contour if non-empty.
                if current.len() >= 2 {
                    contours.push(current);
                }
                current = Vec::new();
                let pt = [p.x, p.y];
                current.push(pt);
                current_pt = pt;
                subpath_start = pt;
            }
            PathSegment::LineTo(p) => {
                let pt = [p.x, p.y];
                current.push(pt);
                current_pt = pt;
            }
            PathSegment::QuadTo(cp, end) => {
                flatten_quad(
                    current_pt,
                    [cp.x, cp.y],
                    [end.x, end.y],
                    tolerance,
                    &mut current,
                );
                current_pt = [end.x, end.y];
            }
            PathSegment::CurveTo(cp1, cp2, end) => {
                flatten_cubic(
                    current_pt,
                    [cp1.x, cp1.y],
                    [cp2.x, cp2.y],
                    [end.x, end.y],
                    tolerance,
                    &mut current,
                );
                current_pt = [end.x, end.y];
            }
            PathSegment::ClosePath => {
                // Close the contour: if the last point isn't the same as the start, the
                // contour is already implicitly closed by i_overlay, but we keep it consistent.
                if current.len() >= 2 {
                    // Only add closing point if it differs from the first.
                    if let Some(first) = current.first() {
                        let dx = current_pt[0] - first[0];
                        let dy = current_pt[1] - first[1];
                        if dx * dx + dy * dy > 1e-12 {
                            // i_overlay auto-closes, so we don't need to add the start point.
                        }
                    }
                    contours.push(current);
                }
                current = Vec::new();
                current_pt = subpath_start;
            }
        }
    }

    // Push any remaining open contour.
    if current.len() >= 2 {
        contours.push(current);
    }

    contours
}

/// Recursively flatten a quadratic Bezier using de Casteljau subdivision.
fn flatten_quad(p0: [f64; 2], cp: [f64; 2], p2: [f64; 2], tolerance: f64, out: &mut Vec<[f64; 2]>) {
    // Check if the curve is flat enough: compute the distance from the control
    // point to the line p0-p2.
    let dist = point_to_line_distance(cp, p0, p2);
    if dist <= tolerance {
        out.push(p2);
    } else {
        // Subdivide at t=0.5
        let m01 = midpoint(p0, cp);
        let m12 = midpoint(cp, p2);
        let mid = midpoint(m01, m12);

        flatten_quad(p0, m01, mid, tolerance, out);
        flatten_quad(mid, m12, p2, tolerance, out);
    }
}

/// Recursively flatten a cubic Bezier using de Casteljau subdivision.
fn flatten_cubic(
    p0: [f64; 2],
    cp1: [f64; 2],
    cp2: [f64; 2],
    p3: [f64; 2],
    tolerance: f64,
    out: &mut Vec<[f64; 2]>,
) {
    // Check flatness: both control points should be close to the line p0-p3.
    let d1 = point_to_line_distance(cp1, p0, p3);
    let d2 = point_to_line_distance(cp2, p0, p3);
    if d1 <= tolerance && d2 <= tolerance {
        out.push(p3);
    } else {
        // Subdivide at t=0.5
        let m01 = midpoint(p0, cp1);
        let m12 = midpoint(cp1, cp2);
        let m23 = midpoint(cp2, p3);
        let m012 = midpoint(m01, m12);
        let m123 = midpoint(m12, m23);
        let mid = midpoint(m012, m123);

        flatten_cubic(p0, m01, m012, mid, tolerance, out);
        flatten_cubic(mid, m123, m23, p3, tolerance, out);
    }
}

/// Distance from point `p` to the line defined by `a` and `b`.
fn point_to_line_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-20 {
        // a and b are essentially the same point
        let ex = p[0] - a[0];
        let ey = p[1] - a[1];
        return (ex * ex + ey * ey).sqrt();
    }
    let cross = (p[0] - a[0]) * dy - (p[1] - a[1]) * dx;
    cross.abs() / len_sq.sqrt()
}

#[inline]
fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    #[test]
    fn flatten_simple_line_path() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.close();

        let contours = flatten_path(&path, 0.25);
        assert_eq!(contours.len(), 1);
        // 3 points: (0,0), (100,0), (100,100). The close doesn't add start again
        // since i_overlay auto-closes.
        assert_eq!(contours[0].len(), 3);
    }

    #[test]
    fn flatten_cubic_curve() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 0.0),
        );
        path.close();

        let contours = flatten_path(&path, 0.25);
        assert_eq!(contours.len(), 1);
        // Should have more than 2 points due to subdivision.
        assert!(
            contours[0].len() > 3,
            "expected subdivision, got {} points",
            contours[0].len()
        );
        // First point should be origin.
        assert_eq!(contours[0][0], [0.0, 0.0]);
        // Last point should be (100, 0).
        let last = contours[0].last().unwrap();
        assert!((last[0] - 100.0).abs() < 1e-10);
        assert!((last[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn flatten_quad_curve() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.quad_to(Point::new(50.0, 100.0), Point::new(100.0, 0.0));
        path.close();

        let contours = flatten_path(&path, 0.25);
        assert_eq!(contours.len(), 1);
        assert!(
            contours[0].len() > 3,
            "expected subdivision, got {} points",
            contours[0].len()
        );
    }

    #[test]
    fn flatten_empty_path() {
        let path = BezPath::new();
        let contours = flatten_path(&path, 0.25);
        assert!(contours.is_empty());
    }

    #[test]
    fn flatten_multiple_subpaths() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(10.0, 10.0));
        path.close();
        path.move_to(Point::new(20.0, 20.0));
        path.line_to(Point::new(30.0, 20.0));
        path.line_to(Point::new(30.0, 30.0));
        path.close();

        let contours = flatten_path(&path, 0.25);
        assert_eq!(contours.len(), 2);
    }
}
