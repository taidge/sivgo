//! Stroke to path conversion -- convert a stroked path to its filled outline.

use sivgo_math::BezPath;

/// Line cap style for stroke-to-path conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    /// Flat cap (no extension beyond the endpoint).
    Butt,
    /// Square cap extending half the stroke width beyond the endpoint.
    Square,
    /// Rounded cap with semicircle at the endpoint.
    Round,
}

/// Line join style for stroke-to-path conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// A straight line connecting the outer edges of the two segments.
    Bevel,
    /// Segments are extended to their natural intersection point, limited by
    /// the miter limit.
    Miter,
    /// An arc between the two segments.
    Round,
}

/// Parameters for stroke-to-path conversion.
#[derive(Debug, Clone)]
pub struct StrokeParams {
    /// The stroke width.
    pub width: f64,
    /// The line cap style.
    pub line_cap: LineCap,
    /// The line join style.
    pub line_join: LineJoin,
    /// The miter limit (only relevant for `LineJoin::Miter`). Defaults to 4.0.
    pub miter_limit: f64,
}

impl Default for StrokeParams {
    fn default() -> Self {
        StrokeParams {
            width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// Convert a stroked path to its filled outline.
///
/// Takes a path and stroke parameters (width, cap, join, miter limit) and
/// returns a new path representing the outline of the stroke. This uses
/// kurbo's stroke expansion functionality for accurate results.
///
/// # Arguments
/// * `path` - The center-line path to stroke.
/// * `params` - Stroke parameters (width, line cap, line join, miter limit).
/// * `tolerance` - Tolerance for curve approximation.
///
/// # Returns
/// A `BezPath` representing the outline of the stroked path.
pub fn stroke_to_path(path: &BezPath, params: &StrokeParams, tolerance: f64) -> BezPath {
    if path.is_empty() || params.width <= 0.0 {
        return BezPath::new();
    }

    let kurbo_cap = match params.line_cap {
        LineCap::Butt => kurbo::Cap::Butt,
        LineCap::Square => kurbo::Cap::Square,
        LineCap::Round => kurbo::Cap::Round,
    };

    let kurbo_join = match params.line_join {
        LineJoin::Bevel => kurbo::Join::Bevel,
        LineJoin::Miter => kurbo::Join::Miter,
        LineJoin::Round => kurbo::Join::Round,
    };

    let style = kurbo::Stroke {
        width: params.width,
        join: kurbo_join,
        miter_limit: params.miter_limit,
        start_cap: kurbo_cap,
        end_cap: kurbo_cap,
        dash_pattern: Default::default(),
        dash_offset: 0.0,
    };

    let opts = kurbo::StrokeOpts::default();
    let kurbo_path = path.to_kurbo();
    let stroked = kurbo::stroke(kurbo_path.iter(), &style, &opts, tolerance);

    BezPath::from_kurbo(stroked)
}

/// Convenience: convert a stroked path with simple parameters.
///
/// Uses default miter limit (4.0) and butt caps with miter joins.
pub fn stroke_to_path_simple(path: &BezPath, width: f64) -> BezPath {
    let params = StrokeParams {
        width,
        ..Default::default()
    };
    stroke_to_path(path, &params, 0.25)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    #[test]
    fn stroke_simple_line() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));

        let params = StrokeParams {
            width: 10.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        };
        let result = stroke_to_path(&path, &params, 0.25);
        assert!(!result.is_empty(), "stroked path should not be empty");

        // The bounding box should be roughly 100 wide and 10 tall.
        let bb = result.bounding_box();
        assert!(
            (bb.x1 - bb.x0 - 100.0).abs() < 1.0,
            "width: expected ~100, got {}",
            bb.x1 - bb.x0
        );
        assert!(
            (bb.y1 - bb.y0 - 10.0).abs() < 1.0,
            "height: expected ~10, got {}",
            bb.y1 - bb.y0
        );
    }

    #[test]
    fn stroke_closed_rect() {
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));
        path.line_to(Point::new(110.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();

        let params = StrokeParams {
            width: 4.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        };
        let result = stroke_to_path(&path, &params, 0.25);
        assert!(!result.is_empty());

        let bb = result.bounding_box();
        // The stroke should extend 2px (half width) beyond the rect on each side.
        assert!(bb.x0 < 10.0, "expected x0 < 10, got {}", bb.x0);
        assert!(bb.y0 < 10.0, "expected y0 < 10, got {}", bb.y0);
        assert!(bb.x1 > 110.0, "expected x1 > 110, got {}", bb.x1);
        assert!(bb.y1 > 60.0, "expected y1 > 60, got {}", bb.y1);
    }

    #[test]
    fn stroke_with_round_caps() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 50.0));
        path.line_to(Point::new(100.0, 50.0));

        let params = StrokeParams {
            width: 20.0,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            miter_limit: 4.0,
        };
        let result = stroke_to_path(&path, &params, 0.25);
        assert!(!result.is_empty());

        let bb = result.bounding_box();
        // Round caps should extend ~10px (half width) beyond each end.
        assert!(bb.x0 < 0.0, "round cap should extend before start");
        assert!(bb.x1 > 100.0, "round cap should extend past end");
    }

    #[test]
    fn stroke_with_square_caps() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 50.0));
        path.line_to(Point::new(100.0, 50.0));

        let params = StrokeParams {
            width: 20.0,
            line_cap: LineCap::Square,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        };
        let result = stroke_to_path(&path, &params, 0.25);
        assert!(!result.is_empty());

        let bb = result.bounding_box();
        // Square caps extend half-width beyond endpoints.
        assert!(
            bb.x0 < -9.0,
            "square cap should extend ~10px before start, got x0={}",
            bb.x0
        );
        assert!(
            bb.x1 > 109.0,
            "square cap should extend ~10px past end, got x1={}",
            bb.x1
        );
    }

    #[test]
    fn stroke_empty_path() {
        let path = BezPath::new();
        let result = stroke_to_path_simple(&path, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn stroke_zero_width() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        let result = stroke_to_path_simple(&path, 0.0);
        assert!(result.is_empty());
    }

    #[test]
    fn stroke_simple_convenience() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(50.0, 0.0));

        let result = stroke_to_path_simple(&path, 6.0);
        assert!(!result.is_empty());

        let bb = result.bounding_box();
        assert!(
            (bb.y1 - bb.y0 - 6.0).abs() < 1.0,
            "height should be ~6, got {}",
            bb.y1 - bb.y0
        );
    }

    #[test]
    fn stroke_curve_path() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 0.0),
        );

        let params = StrokeParams {
            width: 8.0,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            miter_limit: 4.0,
        };
        let result = stroke_to_path(&path, &params, 0.25);
        assert!(!result.is_empty());
        // Should produce a reasonably complex path with curves.
        assert!(
            result.element_count() > 4,
            "stroked curve should have many elements, got {}",
            result.element_count()
        );
    }
}
