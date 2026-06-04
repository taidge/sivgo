use sivgo_math::{BezPath, PathSegment, Point, Rect};

pub(super) fn drag_rect(start: Point, end: Point, square: bool, from_center: bool) -> Rect {
    let mut dx = end.x - start.x;
    let mut dy = end.y - start.y;

    if square {
        let side = dx.abs().max(dy.abs());
        dx = side * dx.signum();
        dy = side * dy.signum();
    }

    if from_center {
        Rect::from_points(
            Point::new(start.x - dx, start.y - dy),
            Point::new(start.x + dx, start.y + dy),
        )
    } else {
        Rect::from_points(start, Point::new(start.x + dx, start.y + dy))
    }
}

pub(super) fn fit_path_to_rect(path: &BezPath, target: Rect) -> BezPath {
    let source = path.bounding_box();
    if source.width().abs() < 1e-12 || source.height().abs() < 1e-12 {
        return path.clone();
    }

    let scale_x = target.width() / source.width();
    let scale_y = target.height() / source.height();
    let map = |p: Point| {
        Point::new(
            target.x0 + (p.x - source.x0) * scale_x,
            target.y0 + (p.y - source.y0) * scale_y,
        )
    };

    let mut fitted = BezPath::new();
    for segment in path.elements() {
        match segment {
            PathSegment::MoveTo(p) => fitted.move_to(map(p)),
            PathSegment::LineTo(p) => fitted.line_to(map(p)),
            PathSegment::QuadTo(p1, p2) => fitted.quad_to(map(p1), map(p2)),
            PathSegment::CurveTo(p1, p2, p3) => fitted.curve_to(map(p1), map(p2), map(p3)),
            PathSegment::ClosePath => fitted.close(),
        }
    }
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn drag_rect_supports_square_and_center_modes() {
        let start = Point::new(10.0, 20.0);
        let end = Point::new(30.0, 35.0);

        assert_eq!(
            drag_rect(start, end, false, false),
            Rect::new(10.0, 20.0, 30.0, 35.0)
        );
        assert_eq!(
            drag_rect(start, end, true, false),
            Rect::new(10.0, 20.0, 30.0, 40.0)
        );
        assert_eq!(
            drag_rect(start, end, false, true),
            Rect::new(-10.0, 5.0, 30.0, 35.0)
        );
    }

    #[test]
    fn fit_path_to_rect_maps_path_bounds_to_target_bounds() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(2.0, 0.0));
        path.line_to(Point::new(2.0, 1.0));
        path.close();

        let fitted = fit_path_to_rect(&path, Rect::new(10.0, 20.0, 30.0, 50.0));
        let bounds = fitted.bounding_box();
        assert_close(bounds.x0, 10.0);
        assert_close(bounds.y0, 20.0);
        assert_close(bounds.x1, 30.0);
        assert_close(bounds.y1, 50.0);
    }
}
