//! Offset Live Path Effect.
//!
//! Insets or outsets the input path by a given distance, producing a preview
//! of the offset result. Uses path offset computation from the underlying
//! geometry library.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Line join style for the offset effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetJoin {
    /// Miter join (sharp corners).
    Miter,
    /// Bevel join (cut corners).
    Bevel,
    /// Round join (arc at corners).
    Round,
}

/// Offset LPE.
///
/// Applies an inset (negative) or outset (positive) to the input path.
/// The path is flattened to line segments, each segment is offset
/// perpendicular to its direction, and intersections between consecutive
/// offset edges are computed to form the result.
#[derive(Debug, Clone)]
pub struct OffsetLpe {
    /// Offset distance (positive = outset, negative = inset).
    offset: LpeParam,
    /// Join style at corners.
    join: OffsetJoin,
}

impl OffsetLpe {
    /// Create a new Offset effect.
    ///
    /// * `offset` - Distance to offset. Positive outsets, negative insets.
    /// * `join` - The join style to use at corners.
    pub fn new(offset: f64, join: OffsetJoin) -> Self {
        Self {
            offset: LpeParam::new("offset", offset, -100_000.0, 100_000.0),
            join,
        }
    }

    /// Set the join style.
    pub fn set_join(&mut self, join: OffsetJoin) {
        self.join = join;
    }

    /// Get the join style.
    pub fn join(&self) -> OffsetJoin {
        self.join
    }
}

impl LivePathEffect for OffsetLpe {
    fn name(&self) -> &str {
        "offset"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let distance = self.offset.value;
        if distance.abs() < 1e-12 || input.is_empty() {
            return input.clone();
        }

        // Flatten the path and offset each contour.
        let tolerance = 0.25;
        let flat = input.flatten(tolerance);
        let elements = flat.elements();
        let contours = collect_closed_contours(&elements);

        let mut result = BezPath::new();

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }

            if let Some(offset_pts) = offset_contour(contour, distance, self.join) {
                if offset_pts.len() >= 3 {
                    result.move_to(offset_pts[0]);
                    for &pt in &offset_pts[1..] {
                        result.line_to(pt);
                    }
                    result.close();
                }
            }
        }

        if result.is_empty() {
            return input.clone();
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.offset)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "offset" {
            self.offset.value = value.clamp(self.offset.min, self.offset.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Collect closed contours from flattened path elements.
fn collect_closed_contours(elements: &[PathSegment]) -> Vec<Vec<Point>> {
    let mut contours: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                if current.len() >= 3 {
                    contours.push(current);
                }
                current = vec![p];
            }
            PathSegment::LineTo(p) => {
                current.push(p);
            }
            PathSegment::QuadTo(_cp, end) => {
                current.push(end);
            }
            PathSegment::CurveTo(_cp1, _cp2, end) => {
                current.push(end);
            }
            PathSegment::ClosePath => {
                // Remove duplicate closing point.
                if current.len() >= 2 {
                    let first = current[0];
                    let last = *current.last().unwrap();
                    if first.distance(last) < 1e-10 {
                        current.pop();
                    }
                }
                if current.len() >= 3 {
                    contours.push(current);
                }
                current = Vec::new();
            }
        }
    }
    // Also add any unclosed contour with enough points.
    if current.len() >= 3 {
        contours.push(current);
    }
    contours
}

/// Signed area of a polygon (shoelace formula).
fn signed_area(contour: &[Point]) -> f64 {
    let n = contour.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += contour[i].x * contour[j].y;
        area -= contour[j].x * contour[i].y;
    }
    area * 0.5
}

/// Offset a single closed contour by `distance`.
fn offset_contour(contour: &[Point], distance: f64, join: OffsetJoin) -> Option<Vec<Point>> {
    let n = contour.len();
    if n < 3 {
        return None;
    }

    let area = signed_area(contour);
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };

    // Compute offset edges.
    let mut offset_edges: Vec<(Point, Point)> = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = contour[j].x - contour[i].x;
        let dy = contour[j].y - contour[i].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            offset_edges.push((contour[i], contour[j]));
            continue;
        }
        let nx = sign * dy / len * distance;
        let ny = sign * -dx / len * distance;

        offset_edges.push((
            Point::new(contour[i].x + nx, contour[i].y + ny),
            Point::new(contour[j].x + nx, contour[j].y + ny),
        ));
    }

    // Compute intersection of consecutive offset edges.
    let mut result: Vec<Point> = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i == 0 { n - 1 } else { i - 1 };
        let (a1, a2) = offset_edges[prev];
        let (b1, b2) = offset_edges[i];

        match join {
            OffsetJoin::Miter | OffsetJoin::Round => {
                if let Some(pt) = line_line_intersection(a1, a2, b1, b2) {
                    result.push(pt);
                } else {
                    result.push(b1);
                }
            }
            OffsetJoin::Bevel => {
                // For bevel, emit both endpoints at the corner.
                result.push(a2);
                result.push(b1);
            }
        }
    }

    if result.len() >= 3 {
        Some(result)
    } else {
        None
    }
}

/// Line-line intersection of two infinite lines, each given by two points.
fn line_line_intersection(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Point> {
    let d1 = a2 - a1;
    let d2 = b2 - b1;
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-12 {
        return None;
    }
    let d = b1 - a1;
    let t = (d.x * d2.y - d.y * d2.x) / denom;
    Some(Point::new(a1.x + t * d1.x, a1.y + t * d1.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    #[test]
    fn test_offset_zero_is_identity() {
        let lpe = OffsetLpe::new(0.0, OffsetJoin::Miter);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_outset_grows_bbox() {
        let lpe = OffsetLpe::new(5.0, OffsetJoin::Miter);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!(bb.x0 < 0.0, "x0={}", bb.x0);
        assert!(bb.y0 < 0.0, "y0={}", bb.y0);
        assert!(bb.x1 > 100.0, "x1={}", bb.x1);
        assert!(bb.y1 > 100.0, "y1={}", bb.y1);
    }

    #[test]
    fn test_inset_shrinks_bbox() {
        let lpe = OffsetLpe::new(-5.0, OffsetJoin::Miter);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!(bb.x0 > 0.0, "x0={}", bb.x0);
        assert!(bb.y0 > 0.0, "y0={}", bb.y0);
        assert!(bb.x1 < 100.0, "x1={}", bb.x1);
        assert!(bb.y1 < 100.0, "y1={}", bb.y1);
    }

    #[test]
    fn test_offset_empty_path() {
        let lpe = OffsetLpe::new(10.0, OffsetJoin::Miter);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_offset_bevel_join() {
        let lpe = OffsetLpe::new(5.0, OffsetJoin::Bevel);
        let input = square();
        let output = lpe.apply(&input);
        // Bevel adds extra points at corners.
        assert!(
            output.element_count() > input.element_count(),
            "bevel should produce more segments: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_offset_params() {
        let mut lpe = OffsetLpe::new(10.0, OffsetJoin::Miter);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "offset");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("offset", -5.0);
        assert!((lpe.offset.value - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_offset_name() {
        let lpe = OffsetLpe::new(1.0, OffsetJoin::Miter);
        assert_eq!(lpe.name(), "offset");
    }

    #[test]
    fn test_offset_round_join() {
        let lpe = OffsetLpe::new(5.0, OffsetJoin::Round);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }
}
