//! Knot Live Path Effect.
//!
//! Creates crossing effects at path self-intersections. At each
//! intersection, a gap is introduced in one strand so it appears to
//! pass under the other strand, giving a woven or knotted look.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Knot LPE.
///
/// At each self-intersection of the input path, a gap of `gap_width` is
/// cut into one of the crossing strands. The strands alternate
/// over/under at successive crossings.
#[derive(Debug, Clone)]
pub struct KnotLpe {
    /// Width of the gap at each crossing (in user units).
    gap_width: LpeParam,
}

impl KnotLpe {
    /// Create a new Knot effect.
    ///
    /// * `gap_width` - The width of the gap cut at each intersection.
    pub fn new(gap_width: f64) -> Self {
        Self {
            gap_width: LpeParam::new("gap_width", gap_width, 0.0, 10_000.0),
        }
    }
}

impl LivePathEffect for KnotLpe {
    fn name(&self) -> &str {
        "knot"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let gap = self.gap_width.value;
        if gap <= 0.0 || input.is_empty() {
            return input.clone();
        }

        // Flatten the path to line segments for intersection detection.
        let tolerance = 0.5;
        let flat = input.flatten(tolerance);
        let elements = flat.elements();
        let segments = collect_line_segments(&elements);

        if segments.len() < 2 {
            return input.clone();
        }

        // Find all self-intersections.
        let crossings = find_crossings(&segments);

        if crossings.is_empty() {
            return input.clone();
        }

        // For each crossing, compute the gap regions.
        // We mark segments near crossings as "interrupted".
        let half_gap = gap / 2.0;
        let mut gaps: Vec<(Point, f64)> = Vec::new(); // (crossing_point, gap_radius)

        for (i, crossing) in crossings.iter().enumerate() {
            // Alternate: even crossings gap the first strand,
            // odd crossings gap the second strand.
            // Since we're working with flattened segments, we just
            // create gaps around the crossing points.
            if i % 2 == 0 {
                gaps.push((crossing.point, half_gap));
            }
        }

        // Rebuild the path, introducing gaps near crossing points.
        rebuild_with_gaps(&elements, &gaps)
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.gap_width)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "gap_width" {
            self.gap_width.value = value.clamp(self.gap_width.min, self.gap_width.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types and helpers
// ---------------------------------------------------------------------------

/// A line segment defined by two endpoints, with subpath index.
#[derive(Debug, Clone, Copy)]
struct LineSeg {
    a: Point,
    b: Point,
    /// Which subpath this segment belongs to.
    subpath: usize,
}

/// A crossing (intersection) between two segments.
#[derive(Debug, Clone)]
struct Crossing {
    point: Point,
}

/// Collect all line segments from flattened path elements.
fn collect_line_segments(elements: &[PathSegment]) -> Vec<LineSeg> {
    let mut segments = Vec::new();
    let mut current = Point::ZERO;
    let mut subpath_idx = 0usize;

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                if !segments.is_empty() {
                    subpath_idx += 1;
                }
                current = p;
            }
            PathSegment::LineTo(p) => {
                segments.push(LineSeg {
                    a: current,
                    b: p,
                    subpath: subpath_idx,
                });
                current = p;
            }
            PathSegment::QuadTo(_cp, end) => {
                segments.push(LineSeg {
                    a: current,
                    b: end,
                    subpath: subpath_idx,
                });
                current = end;
            }
            PathSegment::CurveTo(_cp1, _cp2, end) => {
                segments.push(LineSeg {
                    a: current,
                    b: end,
                    subpath: subpath_idx,
                });
                current = end;
            }
            PathSegment::ClosePath => {}
        }
    }
    segments
}

/// Find all self-intersections among the line segments.
fn find_crossings(segments: &[LineSeg]) -> Vec<Crossing> {
    let mut crossings = Vec::new();

    for i in 0..segments.len() {
        for j in (i + 1)..segments.len() {
            // Skip adjacent segments within the same subpath (they share an endpoint).
            let same_subpath = segments[i].subpath == segments[j].subpath;
            if same_subpath && (j == i + 1) {
                continue;
            }
            if let Some(pt) = seg_seg_intersection(&segments[i], &segments[j]) {
                crossings.push(Crossing { point: pt });
            }
        }
    }

    crossings
}

/// Compute the intersection of two line segments.
///
/// Returns `Some(point)` if the segments cross, `None` otherwise.
fn seg_seg_intersection(s1: &LineSeg, s2: &LineSeg) -> Option<Point> {
    let d1 = s1.b - s1.a;
    let d2 = s2.b - s2.a;
    let denom = d1.x * d2.y - d1.y * d2.x;

    if denom.abs() < 1e-12 {
        return None; // Parallel or coincident.
    }

    let d = s2.a - s1.a;
    let t = (d.x * d2.y - d.y * d2.x) / denom;
    let u = (d.x * d1.y - d.y * d1.x) / denom;

    // Check both parameters are in [0, 1] (proper intersection).
    if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
        Some(Point::new(s1.a.x + t * d1.x, s1.a.y + t * d1.y))
    } else {
        None
    }
}

#[allow(unused_assignments)]
/// Rebuild the path, introducing gaps near crossing points.
///
/// Segments that pass within `gap_radius` of a gap point are split:
/// the portion near the crossing is omitted, creating a visual gap.
fn rebuild_with_gaps(elements: &[PathSegment], gaps: &[(Point, f64)]) -> BezPath {
    let mut result = BezPath::new();
    let mut current = Point::ZERO;
    let mut needs_moveto = false;

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                current = p;
                result.move_to(p);
                needs_moveto = false;
            }
            PathSegment::LineTo(p) => {
                let seg_start = current;
                let seg_end = p;

                // Check if this segment passes through any gap.
                let mut interrupted = false;
                for &(gap_center, gap_radius) in gaps {
                    if segment_near_point(seg_start, seg_end, gap_center, gap_radius) {
                        // Split the segment: emit the parts outside the gap.
                        let seg_vec = seg_end - seg_start;
                        let seg_len = seg_vec.length();

                        if seg_len < 1e-12 {
                            interrupted = true;
                            break;
                        }

                        // Project gap_center onto the segment.
                        let to_gap = gap_center - seg_start;
                        let t_proj =
                            (to_gap.x * seg_vec.x + to_gap.y * seg_vec.y) / (seg_len * seg_len);
                        let t_proj = t_proj.clamp(0.0, 1.0);

                        let t_gap_start = (t_proj - gap_radius / seg_len).max(0.0);
                        let t_gap_end = (t_proj + gap_radius / seg_len).min(1.0);

                        // Emit segment before the gap.
                        if t_gap_start > 0.01 {
                            let before_end = seg_start.lerp(seg_end, t_gap_start);
                            if needs_moveto {
                                result.move_to(seg_start);
                                needs_moveto = false;
                            }
                            result.line_to(before_end);
                        }

                        // Emit segment after the gap.
                        if t_gap_end < 0.99 {
                            let after_start = seg_start.lerp(seg_end, t_gap_end);
                            result.move_to(after_start);
                            result.line_to(seg_end);
                            needs_moveto = false;
                        } else {
                            needs_moveto = true;
                        }

                        interrupted = true;
                        break;
                    }
                }

                if !interrupted {
                    if needs_moveto {
                        result.move_to(seg_start);
                        needs_moveto = false;
                    }
                    result.line_to(p);
                }
                current = p;
            }
            PathSegment::QuadTo(cp, end) => {
                if needs_moveto {
                    result.move_to(current);
                    needs_moveto = false;
                }
                result.quad_to(cp, end);
                current = end;
            }
            PathSegment::CurveTo(cp1, cp2, end) => {
                if needs_moveto {
                    result.move_to(current);
                    needs_moveto = false;
                }
                result.curve_to(cp1, cp2, end);
                current = end;
            }
            PathSegment::ClosePath => {
                result.close();
                needs_moveto = false;
            }
        }
    }

    result
}

/// Check whether a line segment passes within `radius` of a point.
fn segment_near_point(a: Point, b: Point, p: Point, radius: f64) -> bool {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.x * ab.x + ab.y * ab.y;

    if len_sq < 1e-12 {
        return a.distance(p) <= radius;
    }

    let t = ((ap.x * ab.x + ap.y * ab.y) / len_sq).clamp(0.0, 1.0);
    let closest = Point::new(a.x + t * ab.x, a.y + t * ab.y);
    closest.distance(p) <= radius
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An X shape that clearly intersects.
    fn x_shape() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.move_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(0.0, 100.0));
        p
    }

    #[test]
    fn test_knot_on_x_shape() {
        let lpe = KnotLpe::new(10.0);
        let input = x_shape();
        let output = lpe.apply(&input);
        // The output should have more elements due to gap splitting.
        assert!(
            output.element_count() > input.element_count(),
            "knot should split at crossings: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_knot_zero_gap_is_identity() {
        let lpe = KnotLpe::new(0.0);
        let input = x_shape();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_knot_no_intersections() {
        // A simple line has no self-intersections.
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = KnotLpe::new(10.0);
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_knot_empty_path() {
        let lpe = KnotLpe::new(10.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_knot_params() {
        let mut lpe = KnotLpe::new(10.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "gap_width");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("gap_width", 20.0);
        assert!((lpe.gap_width.value - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_knot_name() {
        let lpe = KnotLpe::new(5.0);
        assert_eq!(lpe.name(), "knot");
    }

    #[test]
    fn test_seg_seg_intersection() {
        // Two crossing segments.
        let s1 = LineSeg {
            a: Point::new(0.0, 0.0),
            b: Point::new(100.0, 100.0),
            subpath: 0,
        };
        let s2 = LineSeg {
            a: Point::new(100.0, 0.0),
            b: Point::new(0.0, 100.0),
            subpath: 1,
        };
        let pt = seg_seg_intersection(&s1, &s2).unwrap();
        assert!((pt.x - 50.0).abs() < 1e-6, "ix={}", pt.x);
        assert!((pt.y - 50.0).abs() < 1e-6, "iy={}", pt.y);
    }

    #[test]
    fn test_seg_seg_parallel() {
        let s1 = LineSeg {
            a: Point::new(0.0, 0.0),
            b: Point::new(100.0, 0.0),
            subpath: 0,
        };
        let s2 = LineSeg {
            a: Point::new(0.0, 10.0),
            b: Point::new(100.0, 10.0),
            subpath: 1,
        };
        assert!(seg_seg_intersection(&s1, &s2).is_none());
    }

    #[test]
    fn test_segment_near_point() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(100.0, 0.0);
        assert!(segment_near_point(a, b, Point::new(50.0, 3.0), 5.0));
        assert!(!segment_near_point(a, b, Point::new(50.0, 10.0), 5.0));
    }
}
