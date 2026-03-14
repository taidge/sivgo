//! Dynamic offset -- an offset path that preserves the link to its original
//! source path so the offset amount can be adjusted interactively.
//!
//! Unlike a static offset (which produces a standalone path), a `DynamicOffset`
//! retains the original geometry and recomputes the offset on demand whenever
//! [`set_offset`](DynamicOffset::set_offset) or
//! [`compute`](DynamicOffset::compute) is called.

use crate::offset::offset_path;
use crate::stroke::LineJoin;
use sivgo_math::BezPath;

/// Default curve-flattening tolerance used when computing the offset.
const DEFAULT_TOLERANCE: f64 = 0.25;

/// A path offset whose distance can be changed interactively.
///
/// The struct stores the *original* (un-offset) path together with the current
/// offset amount and join style. Calling [`compute`](Self::compute) produces a
/// new `BezPath` that is offset from the original by the stored distance.
///
/// # Examples
///
/// ```ignore
/// use sivgo_path_ops::dynamic_offset::DynamicOffset;
/// use sivgo_math::{BezPath, Point};
///
/// let mut path = BezPath::new();
/// path.move_to(Point::new(0.0, 0.0));
/// path.line_to(Point::new(100.0, 0.0));
/// path.line_to(Point::new(100.0, 50.0));
/// path.line_to(Point::new(0.0, 50.0));
/// path.close();
///
/// let mut dyn_off = DynamicOffset::new(path, 5.0);
/// let offset_result = dyn_off.compute();
/// // Later, adjust interactively:
/// dyn_off.set_offset(10.0);
/// let offset_result_2 = dyn_off.compute();
/// ```
#[derive(Debug, Clone)]
pub struct DynamicOffset {
    /// The original (un-offset) path.
    original_path: BezPath,
    /// Current offset distance. Positive = outset, negative = inset.
    offset_amount: f64,
    /// Join style used when computing the offset.
    join_style: LineJoin,
    /// Curve-flattening tolerance.
    tolerance: f64,
}

impl DynamicOffset {
    /// Create a new dynamic offset from the given source path and initial
    /// offset distance.
    ///
    /// Uses [`LineJoin::Miter`] and a default tolerance of 0.25.
    pub fn new(original_path: BezPath, offset_amount: f64) -> Self {
        Self {
            original_path,
            offset_amount,
            join_style: LineJoin::Miter,
            tolerance: DEFAULT_TOLERANCE,
        }
    }

    /// Create a new dynamic offset with explicit join style and tolerance.
    pub fn with_params(
        original_path: BezPath,
        offset_amount: f64,
        join_style: LineJoin,
        tolerance: f64,
    ) -> Self {
        Self {
            original_path,
            offset_amount,
            join_style,
            tolerance,
        }
    }

    /// Update the offset distance.
    pub fn set_offset(&mut self, amount: f64) {
        self.offset_amount = amount;
    }

    /// Return the current offset distance.
    pub fn offset_amount(&self) -> f64 {
        self.offset_amount
    }

    /// Update the join style.
    pub fn set_join_style(&mut self, join: LineJoin) {
        self.join_style = join;
    }

    /// Return the current join style.
    pub fn join_style(&self) -> LineJoin {
        self.join_style
    }

    /// Update the curve-flattening tolerance.
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance;
    }

    /// Return the current tolerance.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Return a reference to the original (un-offset) path.
    pub fn original_path(&self) -> &BezPath {
        &self.original_path
    }

    /// Replace the original path with a new one.
    pub fn set_original_path(&mut self, path: BezPath) {
        self.original_path = path;
    }

    /// Compute the offset path using the current parameters.
    ///
    /// This delegates to [`offset_path`] internally.
    pub fn compute(&self) -> BezPath {
        offset_path(&self.original_path, self.offset_amount, self.tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    fn make_rect() -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));
        path.line_to(Point::new(110.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();
        path
    }

    #[test]
    fn zero_offset_preserves_bbox() {
        let rect = make_rect();
        let dyn_off = DynamicOffset::new(rect.clone(), 0.0);
        let result = dyn_off.compute();
        let orig_bb = rect.bounding_box();
        let bb = result.bounding_box();
        assert!((bb.x0 - orig_bb.x0).abs() < 1e-6);
        assert!((bb.y0 - orig_bb.y0).abs() < 1e-6);
        assert!((bb.x1 - orig_bb.x1).abs() < 1e-6);
        assert!((bb.y1 - orig_bb.y1).abs() < 1e-6);
    }

    #[test]
    fn positive_offset_outsets() {
        let rect = make_rect();
        let dyn_off = DynamicOffset::new(rect, 5.0);
        let result = dyn_off.compute();
        let bb = result.bounding_box();
        assert!(bb.x0 < 10.0, "expected x0 < 10, got {}", bb.x0);
        assert!(bb.y0 < 10.0, "expected y0 < 10, got {}", bb.y0);
        assert!(bb.x1 > 110.0, "expected x1 > 110, got {}", bb.x1);
        assert!(bb.y1 > 60.0, "expected y1 > 60, got {}", bb.y1);
    }

    #[test]
    fn negative_offset_insets() {
        let rect = make_rect();
        let dyn_off = DynamicOffset::new(rect, -5.0);
        let result = dyn_off.compute();
        let bb = result.bounding_box();
        assert!(bb.x0 > 10.0, "expected x0 > 10, got {}", bb.x0);
        assert!(bb.y0 > 10.0, "expected y0 > 10, got {}", bb.y0);
        assert!(bb.x1 < 110.0, "expected x1 < 110, got {}", bb.x1);
        assert!(bb.y1 < 60.0, "expected y1 < 60, got {}", bb.y1);
    }

    #[test]
    fn set_offset_changes_result() {
        let rect = make_rect();
        let mut dyn_off = DynamicOffset::new(rect, 5.0);
        let bb1 = dyn_off.compute().bounding_box();

        dyn_off.set_offset(10.0);
        let bb2 = dyn_off.compute().bounding_box();

        // Larger offset should produce a larger bounding box.
        assert!(
            bb2.x0 < bb1.x0,
            "10px outset x0 ({}) should be less than 5px outset x0 ({})",
            bb2.x0,
            bb1.x0
        );
        assert!(
            bb2.x1 > bb1.x1,
            "10px outset x1 ({}) should be greater than 5px outset x1 ({})",
            bb2.x1,
            bb1.x1
        );
    }

    #[test]
    fn original_path_preserved() {
        let rect = make_rect();
        let orig_bb = rect.bounding_box();
        let dyn_off = DynamicOffset::new(rect, 20.0);

        // The original path should be untouched.
        let stored_bb = dyn_off.original_path().bounding_box();
        assert!((stored_bb.x0 - orig_bb.x0).abs() < 1e-6);
        assert!((stored_bb.y0 - orig_bb.y0).abs() < 1e-6);
        assert!((stored_bb.x1 - orig_bb.x1).abs() < 1e-6);
        assert!((stored_bb.y1 - orig_bb.y1).abs() < 1e-6);
    }

    #[test]
    fn accessors_work() {
        let rect = make_rect();
        let mut dyn_off = DynamicOffset::new(rect.clone(), 3.0);
        assert_eq!(dyn_off.offset_amount(), 3.0);
        assert_eq!(dyn_off.join_style(), LineJoin::Miter);
        assert_eq!(dyn_off.tolerance(), DEFAULT_TOLERANCE);

        dyn_off.set_offset(7.5);
        assert_eq!(dyn_off.offset_amount(), 7.5);

        dyn_off.set_join_style(LineJoin::Round);
        assert_eq!(dyn_off.join_style(), LineJoin::Round);

        dyn_off.set_tolerance(0.1);
        assert_eq!(dyn_off.tolerance(), 0.1);
    }

    #[test]
    fn with_params_constructor() {
        let rect = make_rect();
        let dyn_off = DynamicOffset::with_params(rect, 4.0, LineJoin::Bevel, 0.5);
        assert_eq!(dyn_off.offset_amount(), 4.0);
        assert_eq!(dyn_off.join_style(), LineJoin::Bevel);
        assert_eq!(dyn_off.tolerance(), 0.5);
    }

    #[test]
    fn set_original_path_updates_source() {
        let rect = make_rect();
        let mut dyn_off = DynamicOffset::new(rect, 5.0);
        let bb1 = dyn_off.compute().bounding_box();

        // Replace with a smaller rectangle.
        let mut small_rect = BezPath::new();
        small_rect.move_to(Point::new(20.0, 20.0));
        small_rect.line_to(Point::new(40.0, 20.0));
        small_rect.line_to(Point::new(40.0, 40.0));
        small_rect.line_to(Point::new(20.0, 40.0));
        small_rect.close();

        dyn_off.set_original_path(small_rect);
        let bb2 = dyn_off.compute().bounding_box();

        // The new result should be much smaller than the old one.
        assert!(
            bb2.x1 - bb2.x0 < bb1.x1 - bb1.x0,
            "new source should produce smaller output"
        );
    }
}
