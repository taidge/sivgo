//! Linked offset -- an offset path that stays linked to its source path.
//!
//! A `LinkedOffset` is similar to [`DynamicOffset`](crate::dynamic_offset::DynamicOffset),
//! but the offset object maintains a reference to a *source* path rather than
//! owning a copy. When the source path changes, calling
//! [`compute`](LinkedOffset::compute) with the updated source produces a fresh
//! offset.
//!
//! To keep `sivgo-path-ops` independent from the DOM layer, `LinkedOffset` does
//! **not** resolve the source from an `SvgDocument` itself. Instead, the caller
//! (typically `sivgo-core`) supplies the current source `BezPath` when calling
//! [`compute`](LinkedOffset::compute). The struct stores a string-based
//! `source_id` that the caller uses to look up the source node.

use crate::offset::offset_path;
use crate::stroke::LineJoin;
use sivgo_math::BezPath;

/// Default curve-flattening tolerance used when computing the offset.
const DEFAULT_TOLERANCE: f64 = 0.25;

/// A path offset that is linked to an external source object.
///
/// The `source_id` field identifies the source element (e.g. an SVG `id`
/// attribute or a serialized `NodeId`). The actual source geometry is **not**
/// stored here -- it is provided by the caller at compute time. This keeps the
/// crate decoupled from the DOM.
///
/// # Examples
///
/// ```ignore
/// use sivgo_path_ops::linked_offset::LinkedOffset;
/// use sivgo_math::{BezPath, Point};
///
/// // The caller maintains the source path externally.
/// let mut source = BezPath::new();
/// source.move_to(Point::new(0.0, 0.0));
/// source.line_to(Point::new(100.0, 0.0));
/// source.line_to(Point::new(100.0, 50.0));
/// source.line_to(Point::new(0.0, 50.0));
/// source.close();
///
/// let linked = LinkedOffset::new("source-rect".into(), 5.0);
/// let result = linked.compute(&source);
///
/// // When the source path is modified elsewhere, just call compute again.
/// ```
#[derive(Debug, Clone)]
pub struct LinkedOffset {
    /// Identifier of the source element (e.g. SVG `id` or serialized `NodeId`).
    source_id: String,
    /// Current offset distance. Positive = outset, negative = inset.
    offset_amount: f64,
    /// Join style used when computing the offset.
    join_style: LineJoin,
    /// Curve-flattening tolerance.
    tolerance: f64,
}

impl LinkedOffset {
    /// Create a new linked offset referencing the given source element.
    ///
    /// Uses [`LineJoin::Miter`] and a default tolerance of 0.25.
    pub fn new(source_id: String, offset_amount: f64) -> Self {
        Self {
            source_id,
            offset_amount,
            join_style: LineJoin::Miter,
            tolerance: DEFAULT_TOLERANCE,
        }
    }

    /// Create a new linked offset with explicit join style and tolerance.
    pub fn with_params(
        source_id: String,
        offset_amount: f64,
        join_style: LineJoin,
        tolerance: f64,
    ) -> Self {
        Self {
            source_id,
            offset_amount,
            join_style,
            tolerance,
        }
    }

    /// Return the source element identifier.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// Update the source element identifier.
    pub fn set_source_id(&mut self, id: String) {
        self.source_id = id;
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

    /// Compute the offset path from the given source geometry.
    ///
    /// The caller is responsible for resolving `source_id` to a `BezPath` and
    /// passing it here. Returns the offset path, or an empty path if the source
    /// is empty.
    pub fn compute(&self, source_path: &BezPath) -> BezPath {
        if source_path.is_empty() {
            return BezPath::new();
        }
        offset_path(source_path, self.offset_amount, self.tolerance)
    }

    /// Convenience: compute the offset only if a source path is available.
    ///
    /// Accepts an `Option<&BezPath>` and returns `None` when the source could
    /// not be resolved (e.g. the node was deleted).
    pub fn try_compute(&self, source_path: Option<&BezPath>) -> Option<BezPath> {
        source_path.map(|p| self.compute(p))
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
    fn compute_outset() {
        let source = make_rect();
        let linked = LinkedOffset::new("rect1".into(), 5.0);
        let result = linked.compute(&source);
        let bb = result.bounding_box();
        assert!(bb.x0 < 10.0, "expected x0 < 10, got {}", bb.x0);
        assert!(bb.y0 < 10.0, "expected y0 < 10, got {}", bb.y0);
        assert!(bb.x1 > 110.0, "expected x1 > 110, got {}", bb.x1);
        assert!(bb.y1 > 60.0, "expected y1 > 60, got {}", bb.y1);
    }

    #[test]
    fn compute_inset() {
        let source = make_rect();
        let linked = LinkedOffset::new("rect1".into(), -5.0);
        let result = linked.compute(&source);
        let bb = result.bounding_box();
        assert!(bb.x0 > 10.0, "expected x0 > 10, got {}", bb.x0);
        assert!(bb.y0 > 10.0, "expected y0 > 10, got {}", bb.y0);
        assert!(bb.x1 < 110.0, "expected x1 < 110, got {}", bb.x1);
        assert!(bb.y1 < 60.0, "expected y1 < 60, got {}", bb.y1);
    }

    #[test]
    fn zero_offset_preserves_bbox() {
        let source = make_rect();
        let linked = LinkedOffset::new("rect1".into(), 0.0);
        let result = linked.compute(&source);
        let orig_bb = source.bounding_box();
        let bb = result.bounding_box();
        assert!((bb.x0 - orig_bb.x0).abs() < 1e-6);
        assert!((bb.y0 - orig_bb.y0).abs() < 1e-6);
        assert!((bb.x1 - orig_bb.x1).abs() < 1e-6);
        assert!((bb.y1 - orig_bb.y1).abs() < 1e-6);
    }

    #[test]
    fn try_compute_none_source() {
        let linked = LinkedOffset::new("missing-node".into(), 5.0);
        let result = linked.try_compute(None);
        assert!(result.is_none());
    }

    #[test]
    fn try_compute_some_source() {
        let source = make_rect();
        let linked = LinkedOffset::new("rect1".into(), 5.0);
        let result = linked.try_compute(Some(&source));
        assert!(result.is_some());
        let bb = result.unwrap().bounding_box();
        assert!(bb.x0 < 10.0);
    }

    #[test]
    fn empty_source_returns_empty() {
        let empty = BezPath::new();
        let linked = LinkedOffset::new("empty".into(), 5.0);
        let result = linked.compute(&empty);
        assert!(result.is_empty());
    }

    #[test]
    fn set_offset_changes_result() {
        let source = make_rect();
        let mut linked = LinkedOffset::new("rect1".into(), 5.0);
        let bb1 = linked.compute(&source).bounding_box();

        linked.set_offset(10.0);
        let bb2 = linked.compute(&source).bounding_box();

        assert!(bb2.x0 < bb1.x0, "larger offset should produce smaller x0");
        assert!(bb2.x1 > bb1.x1, "larger offset should produce larger x1");
    }

    #[test]
    fn source_change_updates_output() {
        let linked = LinkedOffset::new("rect1".into(), 5.0);

        // Compute with original source.
        let source1 = make_rect();
        let bb1 = linked.compute(&source1).bounding_box();

        // Compute with a different (smaller) source.
        let mut source2 = BezPath::new();
        source2.move_to(Point::new(20.0, 20.0));
        source2.line_to(Point::new(40.0, 20.0));
        source2.line_to(Point::new(40.0, 40.0));
        source2.line_to(Point::new(20.0, 40.0));
        source2.close();
        let bb2 = linked.compute(&source2).bounding_box();

        assert!(
            bb2.x1 - bb2.x0 < bb1.x1 - bb1.x0,
            "smaller source should produce smaller output"
        );
    }

    #[test]
    fn accessors_work() {
        let mut linked = LinkedOffset::new("rect1".into(), 3.0);
        assert_eq!(linked.source_id(), "rect1");
        assert_eq!(linked.offset_amount(), 3.0);
        assert_eq!(linked.join_style(), LineJoin::Miter);
        assert_eq!(linked.tolerance(), DEFAULT_TOLERANCE);

        linked.set_source_id("circle2".into());
        assert_eq!(linked.source_id(), "circle2");

        linked.set_offset(7.5);
        assert_eq!(linked.offset_amount(), 7.5);

        linked.set_join_style(LineJoin::Round);
        assert_eq!(linked.join_style(), LineJoin::Round);

        linked.set_tolerance(0.1);
        assert_eq!(linked.tolerance(), 0.1);
    }

    #[test]
    fn with_params_constructor() {
        let linked = LinkedOffset::with_params("src".into(), 4.0, LineJoin::Bevel, 0.5);
        assert_eq!(linked.source_id(), "src");
        assert_eq!(linked.offset_amount(), 4.0);
        assert_eq!(linked.join_style(), LineJoin::Bevel);
        assert_eq!(linked.tolerance(), 0.5);
    }
}
