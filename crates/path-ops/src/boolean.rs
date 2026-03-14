//! Boolean operations on BezPaths using the `i_overlay` crate.
//!
//! Supports union, intersection, difference, and xor of two paths.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;

use sivgo_math::BezPath;

use crate::flatten::flatten_path;
use crate::refit::shapes_to_path;

/// The type of boolean operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// Combine both shapes into one.
    Union,
    /// Keep only the overlapping area.
    Intersection,
    /// Subtract the clip shape from the subject shape.
    Difference,
    /// Keep areas that are in either shape but not both.
    Xor,
}

/// Perform a boolean operation on two paths.
///
/// The paths are flattened to polygons (using the given `tolerance` for curve
/// subdivision), the boolean operation is executed via `i_overlay`, and the
/// result is converted back to a `BezPath` consisting of line segments.
///
/// # Arguments
/// * `subject` - The subject path (base shape).
/// * `clip` - The clip path (modifier shape).
/// * `op` - The boolean operation to perform.
/// * `tolerance` - Curve flattening tolerance (smaller = more precise, more points).
pub fn boolean_op(subject: &BezPath, clip: &BezPath, op: BooleanOp, tolerance: f64) -> BezPath {
    let subj_contours = flatten_path(subject, tolerance);
    let clip_contours = flatten_path(clip, tolerance);

    if subj_contours.is_empty() && clip_contours.is_empty() {
        return BezPath::new();
    }

    let rule = match op {
        BooleanOp::Union => OverlayRule::Union,
        BooleanOp::Intersection => OverlayRule::Intersect,
        BooleanOp::Difference => OverlayRule::Difference,
        BooleanOp::Xor => OverlayRule::Xor,
    };

    // i_overlay works with contours (Vec<Vec<[f64; 2]>>) as OverlayResource.
    // Use FloatOverlay::with_subj_and_clip for the multi-contour case.
    let mut overlay = FloatOverlay::with_subj_and_clip(&subj_contours, &clip_contours);
    let result = overlay.overlay(rule, FillRule::NonZero);

    shapes_to_path(&result)
}

/// Compute the union of two paths (combine both shapes).
pub fn union(a: &BezPath, b: &BezPath) -> BezPath {
    boolean_op(a, b, BooleanOp::Union, 0.25)
}

/// Compute the intersection of two paths (overlapping area only).
pub fn intersection(a: &BezPath, b: &BezPath) -> BezPath {
    boolean_op(a, b, BooleanOp::Intersection, 0.25)
}

/// Compute the difference of two paths (subtract `b` from `a`).
pub fn difference(a: &BezPath, b: &BezPath) -> BezPath {
    boolean_op(a, b, BooleanOp::Difference, 0.25)
}

/// Compute the symmetric difference (xor) of two paths.
pub fn xor(a: &BezPath, b: &BezPath) -> BezPath {
    boolean_op(a, b, BooleanOp::Xor, 0.25)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    /// Helper: create a rectangle path.
    fn make_rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(x, y));
        path.line_to(Point::new(x + w, y));
        path.line_to(Point::new(x + w, y + h));
        path.line_to(Point::new(x, y + h));
        path.close();
        path
    }

    #[test]
    fn union_of_overlapping_rects() {
        let a = make_rect(0.0, 0.0, 10.0, 10.0);
        let b = make_rect(5.0, 0.0, 10.0, 10.0);
        let result = union(&a, &b);
        assert!(!result.is_empty());

        // The union should have a bounding box from (0,0) to (15,10).
        let bb = result.bounding_box();
        assert!((bb.x0 - 0.0).abs() < 0.5);
        assert!((bb.y0 - 0.0).abs() < 0.5);
        assert!((bb.x1 - 15.0).abs() < 0.5);
        assert!((bb.y1 - 10.0).abs() < 0.5);
    }

    #[test]
    fn intersection_of_overlapping_rects() {
        let a = make_rect(0.0, 0.0, 10.0, 10.0);
        let b = make_rect(5.0, 0.0, 10.0, 10.0);
        let result = intersection(&a, &b);
        assert!(!result.is_empty());

        // The intersection should have a bounding box from (5,0) to (10,10).
        let bb = result.bounding_box();
        assert!((bb.x0 - 5.0).abs() < 0.5);
        assert!((bb.y0 - 0.0).abs() < 0.5);
        assert!((bb.x1 - 10.0).abs() < 0.5);
        assert!((bb.y1 - 10.0).abs() < 0.5);
    }

    #[test]
    fn difference_of_overlapping_rects() {
        let a = make_rect(0.0, 0.0, 10.0, 10.0);
        let b = make_rect(5.0, 0.0, 10.0, 10.0);
        let result = difference(&a, &b);
        assert!(!result.is_empty());

        // The difference should have a bounding box from (0,0) to (5,10).
        let bb = result.bounding_box();
        assert!((bb.x0 - 0.0).abs() < 0.5);
        assert!((bb.y0 - 0.0).abs() < 0.5);
        assert!((bb.x1 - 5.0).abs() < 0.5);
        assert!((bb.y1 - 10.0).abs() < 0.5);
    }

    #[test]
    fn xor_of_overlapping_rects() {
        let a = make_rect(0.0, 0.0, 10.0, 10.0);
        let b = make_rect(5.0, 0.0, 10.0, 10.0);
        let result = xor(&a, &b);
        assert!(!result.is_empty());

        // The xor should have a bounding box from (0,0) to (15,10).
        let bb = result.bounding_box();
        assert!((bb.x0 - 0.0).abs() < 0.5);
        assert!((bb.y0 - 0.0).abs() < 0.5);
        assert!((bb.x1 - 15.0).abs() < 0.5);
        assert!((bb.y1 - 10.0).abs() < 0.5);
    }

    #[test]
    fn union_of_non_overlapping_rects() {
        let a = make_rect(0.0, 0.0, 5.0, 5.0);
        let b = make_rect(10.0, 10.0, 5.0, 5.0);
        let result = union(&a, &b);
        assert!(!result.is_empty());
    }

    #[test]
    fn intersection_of_non_overlapping_rects_is_empty() {
        let a = make_rect(0.0, 0.0, 5.0, 5.0);
        let b = make_rect(10.0, 10.0, 5.0, 5.0);
        let result = intersection(&a, &b);
        // No overlap => empty result.
        assert!(result.is_empty());
    }

    #[test]
    fn boolean_op_with_empty_paths() {
        let a = BezPath::new();
        let b = BezPath::new();
        let result = union(&a, &b);
        assert!(result.is_empty());
    }
}
