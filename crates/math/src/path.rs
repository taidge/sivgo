use std::fmt;

use crate::error::MathError;
use crate::{Point, Rect, Transform2D};

/// A segment of a Bezier path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    /// Move to the given point (starts a new subpath).
    MoveTo(Point),
    /// Draw a line to the given point.
    LineTo(Point),
    /// Draw a quadratic Bezier curve (control point, end point).
    QuadTo(Point, Point),
    /// Draw a cubic Bezier curve (control point 1, control point 2, end point).
    CurveTo(Point, Point, Point),
    /// Close the current subpath.
    ClosePath,
}

/// A Bezier path, wrapping `kurbo::BezPath`.
///
/// Represents an SVG-style path consisting of moveto, lineto, quadratic and cubic
/// Bezier segments, and close operations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BezPath {
    inner: kurbo::BezPath,
}

impl BezPath {
    /// Create an empty path.
    #[inline]
    pub fn new() -> Self {
        BezPath {
            inner: kurbo::BezPath::new(),
        }
    }

    /// Parse a path from an SVG path `d` attribute string.
    ///
    /// Supports standard SVG path commands: M, L, H, V, C, S, Q, T, A, Z
    /// (both absolute and relative).
    pub fn from_svg_path(d: &str) -> Result<Self, MathError> {
        kurbo::BezPath::from_svg(d)
            .map(|inner| BezPath { inner })
            .map_err(|e| MathError::ParseError(format!("SVG path parse error: {}", e)))
    }

    /// Convert the path to an SVG path `d` attribute string.
    pub fn to_svg_string(&self) -> String {
        self.inner.to_svg()
    }

    /// Alias for [`to_svg_string`](Self::to_svg_string).
    pub fn to_svg_path(&self) -> String {
        self.to_svg_string()
    }

    /// Compute the bounding box of the path.
    ///
    /// Returns a zero rect if the path is empty.
    pub fn bounding_box(&self) -> Rect {
        use kurbo::Shape;
        Rect::from(self.inner.bounding_box())
    }

    /// Return an iterator over the path elements as `PathSegment`s.
    pub fn elements(&self) -> Vec<PathSegment> {
        self.inner
            .elements()
            .iter()
            .map(|el| match *el {
                kurbo::PathEl::MoveTo(p) => PathSegment::MoveTo(Point::from(p)),
                kurbo::PathEl::LineTo(p) => PathSegment::LineTo(Point::from(p)),
                kurbo::PathEl::QuadTo(p1, p2) => {
                    PathSegment::QuadTo(Point::from(p1), Point::from(p2))
                }
                kurbo::PathEl::CurveTo(p1, p2, p3) => {
                    PathSegment::CurveTo(Point::from(p1), Point::from(p2), Point::from(p3))
                }
                kurbo::PathEl::ClosePath => PathSegment::ClosePath,
            })
            .collect()
    }

    /// Return the number of path elements.
    pub fn element_count(&self) -> usize {
        self.inner.elements().len()
    }

    /// Whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.inner.elements().is_empty()
    }

    /// Apply an affine transform to the path in-place.
    pub fn transform(&mut self, t: &Transform2D) {
        let affine: kurbo::Affine = t.to_kurbo();
        self.inner.apply_affine(affine);
    }

    /// Return a new path with the transform applied.
    pub fn transformed(&self, t: &Transform2D) -> BezPath {
        let mut clone = self.clone();
        clone.transform(t);
        clone
    }

    /// Reverse the direction of the path.
    ///
    /// This creates a new path with elements in reverse order. Subpath structure is preserved.
    pub fn reverse(&self) -> BezPath {
        // Kurbo doesn't have a built-in reverse for BezPath, so we implement it
        // by iterating segments in reverse and rebuilding.
        let elements = self.inner.elements();
        if elements.is_empty() {
            return BezPath::new();
        }

        // Collect subpaths: each subpath starts with MoveTo
        let mut subpaths: Vec<Vec<kurbo::PathEl>> = Vec::new();
        let mut current_subpath: Vec<kurbo::PathEl> = Vec::new();

        for el in elements {
            match el {
                kurbo::PathEl::MoveTo(_) => {
                    if !current_subpath.is_empty() {
                        subpaths.push(current_subpath);
                        current_subpath = Vec::new();
                    }
                    current_subpath.push(*el);
                }
                _ => {
                    current_subpath.push(*el);
                }
            }
        }
        if !current_subpath.is_empty() {
            subpaths.push(current_subpath);
        }

        let mut result = kurbo::BezPath::new();

        for subpath in subpaths.iter().rev() {
            if subpath.is_empty() {
                continue;
            }

            let is_closed = matches!(subpath.last(), Some(kurbo::PathEl::ClosePath));

            // Collect all the points that each element contributes to the path.
            // The "current point" after each element is the last endpoint.
            let mut endpoints: Vec<kurbo::Point> = Vec::new();
            // Track the start of the subpath for ClosePath
            let mut subpath_start = kurbo::Point::ORIGIN;

            // Also track the element types (excluding MoveTo and ClosePath)
            struct SegInfo {
                kind: SegKind,
                // control points
                cp1: Option<kurbo::Point>,
                cp2: Option<kurbo::Point>,
            }
            enum SegKind {
                Line,
                Quad,
                Cubic,
            }

            let mut segs: Vec<SegInfo> = Vec::new();

            for el in subpath {
                match *el {
                    kurbo::PathEl::MoveTo(p) => {
                        subpath_start = p;
                        endpoints.push(p);
                    }
                    kurbo::PathEl::LineTo(p) => {
                        segs.push(SegInfo {
                            kind: SegKind::Line,
                            cp1: None,
                            cp2: None,
                        });
                        endpoints.push(p);
                    }
                    kurbo::PathEl::QuadTo(cp, p) => {
                        segs.push(SegInfo {
                            kind: SegKind::Quad,
                            cp1: Some(cp),
                            cp2: None,
                        });
                        endpoints.push(p);
                    }
                    kurbo::PathEl::CurveTo(cp1, cp2, p) => {
                        segs.push(SegInfo {
                            kind: SegKind::Cubic,
                            cp1: Some(cp1),
                            cp2: Some(cp2),
                        });
                        endpoints.push(p);
                    }
                    kurbo::PathEl::ClosePath => {
                        // If close path adds a line segment, we represent it
                        if let Some(&last_ep) = endpoints.last() {
                            if last_ep != subpath_start {
                                segs.push(SegInfo {
                                    kind: SegKind::Line,
                                    cp1: None,
                                    cp2: None,
                                });
                                endpoints.push(subpath_start);
                            }
                        }
                    }
                }
            }

            // Now reverse: the last endpoint becomes the first MoveTo
            if let Some(&last_ep) = endpoints.last() {
                result.move_to(last_ep);
            }

            // Walk segments in reverse
            for (i, seg) in segs.iter().enumerate().rev() {
                // The "from" point for the original segment is endpoints[i]
                let from = endpoints[i];
                match seg.kind {
                    SegKind::Line => {
                        result.line_to(from);
                    }
                    SegKind::Quad => {
                        // Reversed quad: control point stays the same, endpoint is the original "from"
                        result.quad_to(seg.cp1.unwrap(), from);
                    }
                    SegKind::Cubic => {
                        // Reversed cubic: swap control points
                        result.curve_to(seg.cp2.unwrap(), seg.cp1.unwrap(), from);
                    }
                }
            }

            if is_closed {
                result.close_path();
            }
        }

        BezPath { inner: result }
    }

    /// Close the current subpath by appending a ClosePath element.
    pub fn close(&mut self) {
        self.inner.close_path();
    }

    /// Move to a point (start a new subpath).
    pub fn move_to(&mut self, p: Point) {
        self.inner.move_to(kurbo::Point::from(p));
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, p: Point) {
        self.inner.line_to(kurbo::Point::from(p));
    }

    /// Draw a quadratic Bezier to a point.
    pub fn quad_to(&mut self, cp: Point, end: Point) {
        self.inner
            .quad_to(kurbo::Point::from(cp), kurbo::Point::from(end));
    }

    /// Draw a cubic Bezier to a point.
    pub fn curve_to(&mut self, cp1: Point, cp2: Point, end: Point) {
        self.inner.curve_to(
            kurbo::Point::from(cp1),
            kurbo::Point::from(cp2),
            kurbo::Point::from(end),
        );
    }

    /// Append all segments from another `BezPath` to this one.
    pub fn extend(&mut self, other: &BezPath) {
        for el in other.inner.elements() {
            self.inner.push(*el);
        }
    }

    /// Flatten the path into a series of line segments within the given
    /// tolerance. Returns a new path consisting only of `MoveTo`, `LineTo`,
    /// and `ClosePath` elements.
    pub fn flatten(&self, tolerance: f64) -> BezPath {
        let mut result = kurbo::BezPath::new();
        kurbo::flatten(self.inner.clone(), tolerance, |el| result.push(el));
        BezPath { inner: result }
    }

    /// Return the total arc length of the path, approximated to the given
    /// accuracy.
    pub fn arc_length(&self, accuracy: f64) -> f64 {
        use kurbo::ParamCurveArclen;
        let mut total = 0.0;
        for seg in self.inner.segments() {
            total += match seg {
                kurbo::PathSeg::Line(l) => l.arclen(accuracy),
                kurbo::PathSeg::Quad(q) => q.arclen(accuracy),
                kurbo::PathSeg::Cubic(c) => c.arclen(accuracy),
            };
        }
        total
    }

    /// Evaluate the point at parameter `t` along the path, where `t` is
    /// in the range `[0, N]` with `N` being the number of segments.
    /// Returns `None` if the path has no segments.
    pub fn eval(&self, t: f64) -> Option<Point> {
        use kurbo::ParamCurve;
        let segs: Vec<_> = self.inner.segments().collect();
        if segs.is_empty() {
            return None;
        }
        let idx = (t.floor() as usize).min(segs.len() - 1);
        let local_t = (t - idx as f64).clamp(0.0, 1.0);
        let p = match segs[idx] {
            kurbo::PathSeg::Line(l) => l.eval(local_t),
            kurbo::PathSeg::Quad(q) => q.eval(local_t),
            kurbo::PathSeg::Cubic(c) => c.eval(local_t),
        };
        Some(Point::new(p.x, p.y))
    }

    /// Evaluate the tangent vector at parameter `t` along the path.
    ///
    /// The tangent is the derivative of the curve at that point, *not*
    /// normalised. Returns `None` if the path has no segments.
    pub fn tangent_at(&self, t: f64) -> Option<(f64, f64)> {
        use kurbo::{ParamCurve, ParamCurveDeriv};
        let segs: Vec<_> = self.inner.segments().collect();
        if segs.is_empty() {
            return None;
        }
        let idx = (t.floor() as usize).min(segs.len() - 1);
        let local_t = (t - idx as f64).clamp(0.0, 1.0);
        let d = match segs[idx] {
            kurbo::PathSeg::Line(l) => {
                let d = l.deriv();
                d.eval(local_t)
            }
            kurbo::PathSeg::Quad(q) => {
                let d = q.deriv();
                d.eval(local_t)
            }
            kurbo::PathSeg::Cubic(c) => {
                let d = c.deriv();
                d.eval(local_t)
            }
        };
        Some((d.x, d.y))
    }

    /// Find the nearest point on the path to the given point.
    ///
    /// Returns `(parameter, distance_squared)` where parameter is in `[0, N]`
    /// (N = segment count). Returns `None` if the path has no segments.
    pub fn nearest(&self, point: Point, accuracy: f64) -> Option<(f64, f64)> {
        use kurbo::ParamCurveNearest;
        let kp = kurbo::Point::new(point.x, point.y);
        let mut best_t = 0.0;
        let mut best_dist_sq = f64::INFINITY;
        let mut seg_offset = 0.0;
        for seg in self.inner.segments() {
            let n = match seg {
                kurbo::PathSeg::Line(l) => l.nearest(kp, accuracy),
                kurbo::PathSeg::Quad(q) => q.nearest(kp, accuracy),
                kurbo::PathSeg::Cubic(c) => c.nearest(kp, accuracy),
            };
            if n.distance_sq < best_dist_sq {
                best_dist_sq = n.distance_sq;
                best_t = seg_offset + n.t;
            }
            seg_offset += 1.0;
        }
        if best_dist_sq.is_finite() {
            Some((best_t, best_dist_sq))
        } else {
            None
        }
    }

    /// Compute the parameter value at a given arc-length distance from the
    /// start of the path. Returns the parameter in `[0, N]` range.
    /// Returns `None` if the path has no segments.
    pub fn param_at_arclen(&self, distance: f64, accuracy: f64) -> Option<f64> {
        use kurbo::ParamCurveArclen;
        let mut remaining = distance;
        let mut seg_offset = 0.0;
        for seg in self.inner.segments() {
            let seg_len = match &seg {
                kurbo::PathSeg::Line(l) => l.arclen(accuracy),
                kurbo::PathSeg::Quad(q) => q.arclen(accuracy),
                kurbo::PathSeg::Cubic(c) => c.arclen(accuracy),
            };
            if remaining <= seg_len {
                let local_t = match &seg {
                    kurbo::PathSeg::Line(l) => l.inv_arclen(remaining, accuracy),
                    kurbo::PathSeg::Quad(q) => q.inv_arclen(remaining, accuracy),
                    kurbo::PathSeg::Cubic(c) => c.inv_arclen(remaining, accuracy),
                };
                return Some(seg_offset + local_t);
            }
            remaining -= seg_len;
            seg_offset += 1.0;
        }
        // Beyond the end — return the last parameter
        if seg_offset > 0.0 {
            Some(seg_offset)
        } else {
            None
        }
    }

    /// Apply a dash array to the path, returning a new path with dashed
    /// segments. `dash_array` specifies alternating on/off lengths, and
    /// `dash_offset` shifts the pattern start.
    pub fn dash(&self, dash_array: &[f64], dash_offset: f64) -> BezPath {
        if dash_array.is_empty() {
            return self.clone();
        }
        use kurbo::ParamCurveArclen;
        let accuracy = 0.5;
        let total_pattern: f64 = dash_array.iter().sum();
        if total_pattern <= 0.0 {
            return self.clone();
        }

        let mut result = kurbo::BezPath::new();

        // Process each subpath independently.
        let mut subpath_start = kurbo::Point::ORIGIN;
        let mut segments: Vec<kurbo::PathSeg> = Vec::new();

        // Helper: dash a list of segments
        let dash_segments = |segs: &[kurbo::PathSeg], out: &mut kurbo::BezPath| {
            // Compute total length
            let mut offset = dash_offset % total_pattern;
            if offset < 0.0 {
                offset += total_pattern;
            }

            // Determine initial dash state
            let mut dash_idx = 0usize;
            let mut consumed = 0.0;
            while consumed + dash_array[dash_idx % dash_array.len()] <= offset {
                consumed += dash_array[dash_idx % dash_array.len()];
                dash_idx += 1;
            }
            let mut remaining_in_dash =
                dash_array[dash_idx % dash_array.len()] - (offset - consumed);
            let mut drawing = dash_idx.is_multiple_of(2); // even = on, odd = off
            let mut needs_moveto = true;

            for seg in segs {
                let seg_len = match seg {
                    kurbo::PathSeg::Line(l) => l.arclen(accuracy),
                    kurbo::PathSeg::Quad(q) => q.arclen(accuracy),
                    kurbo::PathSeg::Cubic(c) => c.arclen(accuracy),
                };
                let mut seg_consumed = 0.0;

                while seg_consumed < seg_len - 1e-10 {
                    let chunk = remaining_in_dash.min(seg_len - seg_consumed);
                    let t_start = seg_consumed / seg_len;
                    let t_end = ((seg_consumed + chunk) / seg_len).min(1.0);

                    if drawing {
                        let sub = subsegment(seg, t_start, t_end);
                        if needs_moveto {
                            out.move_to(seg_start(&sub));
                            needs_moveto = false;
                        }
                        push_seg(out, &sub);
                    }

                    seg_consumed += chunk;
                    remaining_in_dash -= chunk;

                    if remaining_in_dash <= 1e-10 {
                        dash_idx += 1;
                        remaining_in_dash = dash_array[dash_idx % dash_array.len()];
                        drawing = !drawing;
                        if drawing {
                            needs_moveto = true;
                        }
                    }
                }
            }
        };

        // Walk path elements, collecting per-subpath segments.
        for el in self.inner.elements() {
            match *el {
                kurbo::PathEl::MoveTo(p) => {
                    if !segments.is_empty() {
                        dash_segments(&segments, &mut result);
                        segments.clear();
                    }
                    subpath_start = p;
                }
                kurbo::PathEl::LineTo(p) => {
                    let start = last_point(&segments, subpath_start);
                    segments.push(kurbo::PathSeg::Line(kurbo::Line::new(start, p)));
                }
                kurbo::PathEl::QuadTo(p1, p2) => {
                    let start = last_point(&segments, subpath_start);
                    segments.push(kurbo::PathSeg::Quad(kurbo::QuadBez::new(start, p1, p2)));
                }
                kurbo::PathEl::CurveTo(p1, p2, p3) => {
                    let start = last_point(&segments, subpath_start);
                    segments.push(kurbo::PathSeg::Cubic(kurbo::CubicBez::new(
                        start, p1, p2, p3,
                    )));
                }
                kurbo::PathEl::ClosePath => {
                    let start = last_point(&segments, subpath_start);
                    if (start.x - subpath_start.x).abs() > 1e-10
                        || (start.y - subpath_start.y).abs() > 1e-10
                    {
                        segments.push(kurbo::PathSeg::Line(kurbo::Line::new(start, subpath_start)));
                    }
                    dash_segments(&segments, &mut result);
                    segments.clear();
                }
            }
        }
        if !segments.is_empty() {
            dash_segments(&segments, &mut result);
        }

        BezPath { inner: result }
    }

    /// Access the inner kurbo BezPath.
    #[inline]
    pub fn to_kurbo(&self) -> &kurbo::BezPath {
        &self.inner
    }

    /// Consume this and return the inner kurbo BezPath.
    #[inline]
    pub fn into_kurbo(self) -> kurbo::BezPath {
        self.inner
    }

    /// Create from a kurbo BezPath.
    #[inline]
    pub fn from_kurbo(inner: kurbo::BezPath) -> Self {
        BezPath { inner }
    }
}

/// Subdivide a cubic Bezier at parameter `t` (0..1) using de Casteljau.
///
/// Returns `(left, right)` where each is `(p0, p1, p2, p3)`.
pub fn subdivide_cubic(
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
    t: f64,
) -> (CubicPoints, CubicPoints) {
    let cb = kurbo::CubicBez::new(
        kurbo::Point::new(p0.x, p0.y),
        kurbo::Point::new(p1.x, p1.y),
        kurbo::Point::new(p2.x, p2.y),
        kurbo::Point::new(p3.x, p3.y),
    );
    use kurbo::ParamCurve;
    let (l, r) = cb.subdivide();
    // kurbo's subdivide is at t=0.5. For arbitrary t we use subsegment.
    if (t - 0.5).abs() < 1e-12 {
        (
            (
                Point::new(l.p0.x, l.p0.y),
                Point::new(l.p1.x, l.p1.y),
                Point::new(l.p2.x, l.p2.y),
                Point::new(l.p3.x, l.p3.y),
            ),
            (
                Point::new(r.p0.x, r.p0.y),
                Point::new(r.p1.x, r.p1.y),
                Point::new(r.p2.x, r.p2.y),
                Point::new(r.p3.x, r.p3.y),
            ),
        )
    } else {
        let left = cb.subsegment(0.0..t);
        let right = cb.subsegment(t..1.0);
        (
            (
                Point::new(left.p0.x, left.p0.y),
                Point::new(left.p1.x, left.p1.y),
                Point::new(left.p2.x, left.p2.y),
                Point::new(left.p3.x, left.p3.y),
            ),
            (
                Point::new(right.p0.x, right.p0.y),
                Point::new(right.p1.x, right.p1.y),
                Point::new(right.p2.x, right.p2.y),
                Point::new(right.p3.x, right.p3.y),
            ),
        )
    }
}

type CubicPoints = (Point, Point, Point, Point);

/// Subdivide a quadratic Bezier at parameter `t` (0..1).
///
/// Returns `(left, right)` where each is `(p0, p1, p2)`.
pub fn subdivide_quad(
    p0: Point,
    p1: Point,
    p2: Point,
    t: f64,
) -> ((Point, Point, Point), (Point, Point, Point)) {
    let qb = kurbo::QuadBez::new(
        kurbo::Point::new(p0.x, p0.y),
        kurbo::Point::new(p1.x, p1.y),
        kurbo::Point::new(p2.x, p2.y),
    );
    use kurbo::ParamCurve;
    let left = qb.subsegment(0.0..t);
    let right = qb.subsegment(t..1.0);
    (
        (
            Point::new(left.p0.x, left.p0.y),
            Point::new(left.p1.x, left.p1.y),
            Point::new(left.p2.x, left.p2.y),
        ),
        (
            Point::new(right.p0.x, right.p0.y),
            Point::new(right.p1.x, right.p1.y),
            Point::new(right.p2.x, right.p2.y),
        ),
    )
}

// Dash helper: get subsegment of a PathSeg between t0..t1.
fn subsegment(seg: &kurbo::PathSeg, t0: f64, t1: f64) -> kurbo::PathSeg {
    use kurbo::ParamCurve;
    match seg {
        kurbo::PathSeg::Line(l) => kurbo::PathSeg::Line(l.subsegment(t0..t1)),
        kurbo::PathSeg::Quad(q) => kurbo::PathSeg::Quad(q.subsegment(t0..t1)),
        kurbo::PathSeg::Cubic(c) => kurbo::PathSeg::Cubic(c.subsegment(t0..t1)),
    }
}

// Get the start point of a PathSeg.
fn seg_start(seg: &kurbo::PathSeg) -> kurbo::Point {
    match seg {
        kurbo::PathSeg::Line(l) => l.p0,
        kurbo::PathSeg::Quad(q) => q.p0,
        kurbo::PathSeg::Cubic(c) => c.p0,
    }
}

// Push a PathSeg to a BezPath (without MoveTo).
fn push_seg(path: &mut kurbo::BezPath, seg: &kurbo::PathSeg) {
    match seg {
        kurbo::PathSeg::Line(l) => path.line_to(l.p1),
        kurbo::PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        kurbo::PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
    }
}

// Get the last endpoint from a list of segments, or fall back to subpath start.
fn last_point(segments: &[kurbo::PathSeg], subpath_start: kurbo::Point) -> kurbo::Point {
    segments.last().map_or(subpath_start, |seg| match seg {
        kurbo::PathSeg::Line(l) => l.p1,
        kurbo::PathSeg::Quad(q) => q.p2,
        kurbo::PathSeg::Cubic(c) => c.p3,
    })
}

impl fmt::Display for BezPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_svg_string())
    }
}

impl From<kurbo::BezPath> for BezPath {
    fn from(inner: kurbo::BezPath) -> Self {
        BezPath { inner }
    }
}

impl From<BezPath> for kurbo::BezPath {
    fn from(p: BezPath) -> Self {
        p.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_path() {
        let p = BezPath::new();
        assert!(p.is_empty());
        assert_eq!(p.element_count(), 0);
    }

    #[test]
    fn test_build_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.close();

        assert_eq!(p.element_count(), 4);
        let elems = p.elements();
        assert!(matches!(elems[0], PathSegment::MoveTo(_)));
        assert!(matches!(elems[1], PathSegment::LineTo(_)));
        assert!(matches!(elems[2], PathSegment::LineTo(_)));
        assert!(matches!(elems[3], PathSegment::ClosePath));
    }

    #[test]
    fn test_parse_svg_path() {
        let p = BezPath::from_svg_path("M 10 10 L 100 10 L 100 100 Z").unwrap();
        assert_eq!(p.element_count(), 4);
    }

    #[test]
    fn test_parse_svg_path_relative() {
        let p = BezPath::from_svg_path("m 10 10 l 90 0 l 0 90 z").unwrap();
        assert_eq!(p.element_count(), 4);
    }

    #[test]
    fn test_parse_svg_path_cubic() {
        let p = BezPath::from_svg_path("M 0 0 C 10 20 30 40 50 60").unwrap();
        assert_eq!(p.element_count(), 2);
        let elems = p.elements();
        assert!(matches!(elems[1], PathSegment::CurveTo(..)));
    }

    #[test]
    fn test_to_svg_roundtrip() {
        let original = "M0,0 L100,0 L100,100 Z";
        let p = BezPath::from_svg_path(original).unwrap();
        let svg = p.to_svg_string();
        // Roundtrip should parse back to same path
        let p2 = BezPath::from_svg_path(&svg).unwrap();
        assert_eq!(p.element_count(), p2.element_count());
    }

    #[test]
    fn test_bounding_box() {
        let p = BezPath::from_svg_path("M 10 20 L 100 20 L 100 80 L 10 80 Z").unwrap();
        let bb = p.bounding_box();
        assert!((bb.x0 - 10.0).abs() < 1e-10);
        assert!((bb.y0 - 20.0).abs() < 1e-10);
        assert!((bb.x1 - 100.0).abs() < 1e-10);
        assert!((bb.y1 - 80.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        p.close();

        let t = Transform2D::translate(100.0, 200.0);
        let transformed = p.transformed(&t);
        let bb = transformed.bounding_box();
        assert!((bb.x0 - 100.0).abs() < 1e-10);
        assert!((bb.y0 - 200.0).abs() < 1e-10);
        assert!((bb.x1 - 110.0).abs() < 1e-10);
        assert!((bb.y1 - 210.0).abs() < 1e-10);
    }

    #[test]
    fn test_reverse_line_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));

        let rev = p.reverse();
        let elems = rev.elements();
        assert_eq!(elems.len(), 3);
        assert_eq!(elems[0], PathSegment::MoveTo(Point::new(10.0, 10.0)));
        assert_eq!(elems[1], PathSegment::LineTo(Point::new(10.0, 0.0)));
        assert_eq!(elems[2], PathSegment::LineTo(Point::new(0.0, 0.0)));
    }

    #[test]
    fn test_reverse_cubic_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.curve_to(
            Point::new(10.0, 20.0),
            Point::new(30.0, 40.0),
            Point::new(50.0, 60.0),
        );

        let rev = p.reverse();
        let elems = rev.elements();
        assert_eq!(elems.len(), 2);
        assert_eq!(elems[0], PathSegment::MoveTo(Point::new(50.0, 60.0)));
        // Control points should be swapped
        assert_eq!(
            elems[1],
            PathSegment::CurveTo(
                Point::new(30.0, 40.0),
                Point::new(10.0, 20.0),
                Point::new(0.0, 0.0)
            )
        );
    }

    #[test]
    fn test_display() {
        let p = BezPath::from_svg_path("M 0 0 L 10 10").unwrap();
        let s = format!("{}", p);
        assert!(!s.is_empty());
    }

    #[test]
    fn test_parse_error() {
        let result = BezPath::from_svg_path("L 10 10"); // missing initial moveto
        assert!(result.is_err());
    }

    #[test]
    fn test_flatten_only_lines() {
        let p = BezPath::from_svg_path("M 0 0 C 50 0 50 100 100 100").unwrap();
        let flat = p.flatten(1.0);
        let elems = flat.elements();
        // Should only contain MoveTo, LineTo (no CurveTo)
        for e in &elems {
            assert!(
                matches!(
                    e,
                    PathSegment::MoveTo(_) | PathSegment::LineTo(_) | PathSegment::ClosePath
                ),
                "expected only lines, got {:?}",
                e
            );
        }
        // Should have more elements than the original (subdivision)
        assert!(flat.element_count() > p.element_count());
    }

    #[test]
    fn test_arc_length_line() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let len = p.arc_length(1e-6);
        assert!((len - 100.0).abs() < 0.01, "len={}", len);
    }

    #[test]
    fn test_eval_midpoint() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let mid = p.eval(0.5).unwrap();
        assert!((mid.x - 50.0).abs() < 1e-6);
        assert!((mid.y).abs() < 1e-6);
    }

    #[test]
    fn test_dash_basic() {
        // 100px line with 10-10 dash → 5 visible dashes
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let dashed = p.dash(&[10.0, 10.0], 0.0);
        // Should produce multiple MoveTo segments (5 dashes)
        let elems = dashed.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(
            move_count, 5,
            "expected 5 dash segments, got {}",
            move_count
        );
    }

    #[test]
    fn test_dash_offset() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let dashed = p.dash(&[10.0, 10.0], 5.0);
        // Offset by 5 means first dash is only 5px, then follows pattern
        assert!(!dashed.is_empty());
    }

    #[test]
    fn test_subdivide_cubic_midpoint() {
        let p0 = Point::new(0.0, 0.0);
        let p1 = Point::new(0.0, 100.0);
        let p2 = Point::new(100.0, 100.0);
        let p3 = Point::new(100.0, 0.0);
        let (left, right) = subdivide_cubic(p0, p1, p2, p3, 0.5);
        // Left curve starts at p0
        assert!((left.0.x).abs() < 1e-6);
        assert!((left.0.y).abs() < 1e-6);
        // Left and right share the midpoint
        assert!((left.3.x - right.0.x).abs() < 1e-6);
        assert!((left.3.y - right.0.y).abs() < 1e-6);
        // Right curve ends at p3
        assert!((right.3.x - 100.0).abs() < 1e-6);
        assert!((right.3.y).abs() < 1e-6);
    }

    #[test]
    fn test_nearest_point() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let (t, dist_sq) = p.nearest(Point::new(50.0, 10.0), 1e-6).unwrap();
        assert!((t - 0.5).abs() < 0.01, "t={}", t);
        assert!((dist_sq - 100.0).abs() < 0.1, "dist_sq={}", dist_sq); // 10^2 = 100
    }

    #[test]
    fn test_param_at_arclen() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        let t = p.param_at_arclen(50.0, 1e-6).unwrap();
        assert!((t - 0.5).abs() < 0.01, "t={}", t);
    }

    #[test]
    fn test_subdivide_quad_midpoint() {
        let p0 = Point::new(0.0, 0.0);
        let p1 = Point::new(50.0, 100.0);
        let p2 = Point::new(100.0, 0.0);
        let (left, right) = subdivide_quad(p0, p1, p2, 0.5);
        assert!((left.0.x).abs() < 1e-6);
        assert!((right.2.x - 100.0).abs() < 1e-6);
        // Shared midpoint
        assert!((left.2.x - right.0.x).abs() < 1e-6);
        assert!((left.2.y - right.0.y).abs() < 1e-6);
    }
}
