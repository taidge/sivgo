//! Conversion utilities: shape-to-path, combine, break apart, reverse.

use sivgo_dom::{
    CircleAttrs, EllipseAttrs, LineAttrs, PolygonAttrs, PolylineAttrs, RectAttrs, SvgElement,
};
use sivgo_math::{BezPath, PathSegment, Point};

// ---------------------------------------------------------------------------
// Element to path
// ---------------------------------------------------------------------------

/// Convert an SVG shape element to a `BezPath`.
///
/// Supported elements: `Rect`, `Circle`, `Ellipse`, `Line`, `Polyline`,
/// `Polygon`, and `Path`. Returns `None` for non-shape elements (e.g. `<g>`,
/// `<defs>`, filters, etc.).
pub fn element_to_path(element: &SvgElement) -> Option<BezPath> {
    match element {
        SvgElement::Rect(attrs) => Some(rect_to_path(attrs)),
        SvgElement::Circle(attrs) => Some(circle_to_path(attrs)),
        SvgElement::Ellipse(attrs) => Some(ellipse_to_path(attrs)),
        SvgElement::Line(attrs) => Some(line_to_path(attrs)),
        SvgElement::Polyline(attrs) => Some(polyline_to_path(attrs)),
        SvgElement::Polygon(attrs) => Some(polygon_to_path_element(attrs)),
        SvgElement::Path(attrs) => Some(attrs.d.clone()),
        _ => None,
    }
}

/// Convert a `<rect>` to a `BezPath`.
///
/// Handles `rx`/`ry` for rounded corners using cubic Bezier arcs.
pub fn rect_to_path(attrs: &RectAttrs) -> BezPath {
    let x = attrs.x.value;
    let y = attrs.y.value;
    let w = attrs.width.value;
    let h = attrs.height.value;
    let mut rx = attrs.rx.value;
    let mut ry = attrs.ry.value;

    // Clamp rx/ry per SVG spec.
    if rx > w / 2.0 {
        rx = w / 2.0;
    }
    if ry > h / 2.0 {
        ry = h / 2.0;
    }

    // If only one of rx/ry is set, the other takes the same value.
    if rx > 0.0 && ry == 0.0 {
        ry = rx;
    } else if ry > 0.0 && rx == 0.0 {
        rx = ry;
    }

    let mut path = BezPath::new();

    if rx <= 0.0 || ry <= 0.0 {
        // Simple rectangle without rounded corners.
        path.move_to(Point::new(x, y));
        path.line_to(Point::new(x + w, y));
        path.line_to(Point::new(x + w, y + h));
        path.line_to(Point::new(x, y + h));
        path.close();
    } else {
        // Rounded rectangle using cubic Bezier arcs.
        // The magic number for approximating a quarter circle with a cubic Bezier.
        let k = 0.5522847498; // 4/3 * (sqrt(2) - 1)
        let kx = rx * k;
        let ky = ry * k;

        path.move_to(Point::new(x + rx, y));
        // Top edge.
        path.line_to(Point::new(x + w - rx, y));
        // Top-right corner.
        path.curve_to(
            Point::new(x + w - rx + kx, y),
            Point::new(x + w, y + ry - ky),
            Point::new(x + w, y + ry),
        );
        // Right edge.
        path.line_to(Point::new(x + w, y + h - ry));
        // Bottom-right corner.
        path.curve_to(
            Point::new(x + w, y + h - ry + ky),
            Point::new(x + w - rx + kx, y + h),
            Point::new(x + w - rx, y + h),
        );
        // Bottom edge.
        path.line_to(Point::new(x + rx, y + h));
        // Bottom-left corner.
        path.curve_to(
            Point::new(x + rx - kx, y + h),
            Point::new(x, y + h - ry + ky),
            Point::new(x, y + h - ry),
        );
        // Left edge.
        path.line_to(Point::new(x, y + ry));
        // Top-left corner.
        path.curve_to(
            Point::new(x, y + ry - ky),
            Point::new(x + rx - kx, y),
            Point::new(x + rx, y),
        );
        path.close();
    }

    path
}

/// Convert a `<circle>` to a `BezPath` using four cubic Bezier arcs.
pub fn circle_to_path(attrs: &CircleAttrs) -> BezPath {
    let cx = attrs.cx.value;
    let cy = attrs.cy.value;
    let r = attrs.r.value;
    ellipse_to_path_impl(cx, cy, r, r)
}

/// Convert an `<ellipse>` to a `BezPath` using four cubic Bezier arcs.
pub fn ellipse_to_path(attrs: &EllipseAttrs) -> BezPath {
    let cx = attrs.cx.value;
    let cy = attrs.cy.value;
    let rx = attrs.rx.value;
    let ry = attrs.ry.value;
    ellipse_to_path_impl(cx, cy, rx, ry)
}

/// Internal: create an ellipse path from center and radii.
fn ellipse_to_path_impl(cx: f64, cy: f64, rx: f64, ry: f64) -> BezPath {
    let mut path = BezPath::new();

    if rx <= 0.0 || ry <= 0.0 {
        return path;
    }

    // The magic number for approximating a quarter circle with a cubic Bezier.
    let k = 0.5522847498;
    let kx = rx * k;
    let ky = ry * k;

    // Start at the rightmost point and go clockwise.
    path.move_to(Point::new(cx + rx, cy));

    // Right to bottom.
    path.curve_to(
        Point::new(cx + rx, cy + ky),
        Point::new(cx + kx, cy + ry),
        Point::new(cx, cy + ry),
    );
    // Bottom to left.
    path.curve_to(
        Point::new(cx - kx, cy + ry),
        Point::new(cx - rx, cy + ky),
        Point::new(cx - rx, cy),
    );
    // Left to top.
    path.curve_to(
        Point::new(cx - rx, cy - ky),
        Point::new(cx - kx, cy - ry),
        Point::new(cx, cy - ry),
    );
    // Top to right.
    path.curve_to(
        Point::new(cx + kx, cy - ry),
        Point::new(cx + rx, cy - ky),
        Point::new(cx + rx, cy),
    );
    path.close();

    path
}

/// Convert a `<line>` to a `BezPath`.
pub fn line_to_path(attrs: &LineAttrs) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(Point::new(attrs.x1.value, attrs.y1.value));
    path.line_to(Point::new(attrs.x2.value, attrs.y2.value));
    path
}

/// Convert a `<polyline>` to a `BezPath`.
pub fn polyline_to_path(attrs: &PolylineAttrs) -> BezPath {
    let mut path = BezPath::new();
    if attrs.points.is_empty() {
        return path;
    }
    path.move_to(Point::new(attrs.points[0].0, attrs.points[0].1));
    for &(x, y) in &attrs.points[1..] {
        path.line_to(Point::new(x, y));
    }
    path
}

/// Convert a `<polygon>` to a `BezPath` (same as polyline but closed).
pub fn polygon_to_path_element(attrs: &PolygonAttrs) -> BezPath {
    let mut path = BezPath::new();
    if attrs.points.is_empty() {
        return path;
    }
    path.move_to(Point::new(attrs.points[0].0, attrs.points[0].1));
    for &(x, y) in &attrs.points[1..] {
        path.line_to(Point::new(x, y));
    }
    path.close();
    path
}

// ---------------------------------------------------------------------------
// Combine / Break apart
// ---------------------------------------------------------------------------

/// Combine multiple paths into one by concatenating their subpaths.
///
/// The resulting path contains all subpaths from all input paths.
pub fn combine_paths(paths: &[BezPath]) -> BezPath {
    let mut result = BezPath::new();
    for path in paths {
        result.extend(path);
    }
    result
}

/// Break a path into separate subpaths (one per `MoveTo`).
///
/// Each returned `BezPath` contains exactly one subpath starting with a `MoveTo`.
pub fn break_apart(path: &BezPath) -> Vec<BezPath> {
    let elements = path.elements();
    if elements.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<BezPath> = Vec::new();
    let mut current = BezPath::new();

    for seg in &elements {
        match seg {
            PathSegment::MoveTo(p) => {
                if current.element_count() > 0 {
                    result.push(current);
                    current = BezPath::new();
                }
                current.move_to(*p);
            }
            PathSegment::LineTo(p) => {
                current.line_to(*p);
            }
            PathSegment::QuadTo(cp, end) => {
                current.quad_to(*cp, *end);
            }
            PathSegment::CurveTo(cp1, cp2, end) => {
                current.curve_to(*cp1, *cp2, *end);
            }
            PathSegment::ClosePath => {
                current.close();
            }
        }
    }

    if current.element_count() > 0 {
        result.push(current);
    }

    result
}

/// Reverse a path's direction.
///
/// Delegates to `BezPath::reverse()`.
pub fn reverse_path(path: &BezPath) -> BezPath {
    path.reverse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::SvgLength;

    fn len(v: f64) -> SvgLength {
        SvgLength {
            value: v,
            unit: sivgo_dom::LengthUnit::None,
        }
    }

    #[test]
    fn rect_to_path_simple() {
        let attrs = RectAttrs {
            x: len(10.0),
            y: len(20.0),
            width: len(100.0),
            height: len(50.0),
            rx: len(0.0),
            ry: len(0.0),
        };
        let path = rect_to_path(&attrs);
        assert_eq!(path.element_count(), 5); // M L L L Z
        let bb = path.bounding_box();
        assert!((bb.x0 - 10.0).abs() < 1e-10);
        assert!((bb.y0 - 20.0).abs() < 1e-10);
        assert!((bb.x1 - 110.0).abs() < 1e-10);
        assert!((bb.y1 - 70.0).abs() < 1e-10);
    }

    #[test]
    fn rect_to_path_rounded() {
        let attrs = RectAttrs {
            x: len(0.0),
            y: len(0.0),
            width: len(100.0),
            height: len(50.0),
            rx: len(10.0),
            ry: len(10.0),
        };
        let path = rect_to_path(&attrs);
        // Should have curves for corners.
        let elements = path.elements();
        let has_curves = elements
            .iter()
            .any(|e| matches!(e, PathSegment::CurveTo(..)));
        assert!(has_curves, "rounded rect should have cubic segments");
    }

    #[test]
    fn circle_to_path_bounding_box() {
        let attrs = CircleAttrs {
            cx: len(50.0),
            cy: len(50.0),
            r: len(30.0),
        };
        let path = circle_to_path(&attrs);
        let bb = path.bounding_box();
        assert!((bb.x0 - 20.0).abs() < 0.5);
        assert!((bb.y0 - 20.0).abs() < 0.5);
        assert!((bb.x1 - 80.0).abs() < 0.5);
        assert!((bb.y1 - 80.0).abs() < 0.5);
    }

    #[test]
    fn ellipse_to_path_bounding_box() {
        let attrs = EllipseAttrs {
            cx: len(100.0),
            cy: len(50.0),
            rx: len(40.0),
            ry: len(20.0),
        };
        let path = ellipse_to_path(&attrs);
        let bb = path.bounding_box();
        assert!((bb.x0 - 60.0).abs() < 0.5);
        assert!((bb.y0 - 30.0).abs() < 0.5);
        assert!((bb.x1 - 140.0).abs() < 0.5);
        assert!((bb.y1 - 70.0).abs() < 0.5);
    }

    #[test]
    fn line_to_path_works() {
        let attrs = LineAttrs {
            x1: len(0.0),
            y1: len(0.0),
            x2: len(100.0),
            y2: len(50.0),
        };
        let path = line_to_path(&attrs);
        assert_eq!(path.element_count(), 2); // M L
    }

    #[test]
    fn polyline_to_path_works() {
        let attrs = PolylineAttrs {
            points: vec![(0.0, 0.0), (10.0, 20.0), (30.0, 40.0)],
        };
        let path = polyline_to_path(&attrs);
        assert_eq!(path.element_count(), 3); // M L L
    }

    #[test]
    fn polygon_to_path_works() {
        let attrs = PolygonAttrs {
            points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        };
        let path = polygon_to_path_element(&attrs);
        assert_eq!(path.element_count(), 4); // M L L Z
    }

    #[test]
    fn combine_paths_works() {
        let mut a = BezPath::new();
        a.move_to(Point::new(0.0, 0.0));
        a.line_to(Point::new(10.0, 0.0));
        a.close();

        let mut b = BezPath::new();
        b.move_to(Point::new(20.0, 20.0));
        b.line_to(Point::new(30.0, 20.0));
        b.close();

        let combined = combine_paths(&[a, b]);
        assert_eq!(combined.element_count(), 6); // 2 * (M L Z)
    }

    #[test]
    fn break_apart_splits_subpaths() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(10.0, 0.0));
        path.line_to(Point::new(10.0, 10.0));
        path.close();
        path.move_to(Point::new(20.0, 20.0));
        path.line_to(Point::new(30.0, 20.0));
        path.line_to(Point::new(30.0, 30.0));
        path.close();

        let parts = break_apart(&path);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].element_count(), 4); // M L L Z
        assert_eq!(parts[1].element_count(), 4); // M L L Z
    }

    #[test]
    fn break_apart_empty() {
        let path = BezPath::new();
        let parts = break_apart(&path);
        assert!(parts.is_empty());
    }

    #[test]
    fn element_to_path_returns_none_for_group() {
        let element = SvgElement::Group(sivgo_dom::GroupAttrs::default());
        assert!(element_to_path(&element).is_none());
    }
}
