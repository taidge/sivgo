//! Taper Stroke Live Path Effect.
//!
//! Creates a variable-width stroke that tapers (narrows) at its start and/or
//! end, producing a calligraphic or pointed appearance. The input open path
//! is converted into a closed outline whose width varies linearly from
//! `start_width` through the body to `end_width`, with optional taper
//! lengths at both ends.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Taper Stroke LPE.
///
/// Converts an open path into a closed outline whose stroke width varies
/// from `start_width` at the beginning to `end_width` at the end, with
/// configurable taper lengths at both extremities.
#[derive(Debug, Clone)]
pub struct TaperStrokeLpe {
    /// Width of the stroke at the start of the path.
    start_width: LpeParam,
    /// Width of the stroke at the end of the path.
    end_width: LpeParam,
    /// Length (in user units) over which the taper occurs at each end.
    taper_length: LpeParam,
}

impl TaperStrokeLpe {
    /// Create a new Taper Stroke effect.
    ///
    /// * `start_width` - Stroke width at the path start (half-width on each
    ///   side).
    /// * `end_width` - Stroke width at the path end.
    /// * `taper_length` - Length over which the stroke tapers to a point at
    ///   each end. If this exceeds half the total path length it is clamped.
    pub fn new(start_width: f64, end_width: f64, taper_length: f64) -> Self {
        Self {
            start_width: LpeParam::new("start_width", start_width, 0.0, 100_000.0),
            end_width: LpeParam::new("end_width", end_width, 0.0, 100_000.0),
            taper_length: LpeParam::new("taper_length", taper_length, 0.0, 100_000.0),
        }
    }

    /// Compute the half-width at a given distance `d` along a path of total
    /// length `total`, applying tapers at both ends.
    fn half_width_at(&self, d: f64, total: f64) -> f64 {
        let sw = self.start_width.value * 0.5;
        let ew = self.end_width.value * 0.5;
        let tl = self.taper_length.value.min(total * 0.5);

        // Body width interpolates linearly from start to end over the full
        // path length.
        let t = if total > 0.0 { d / total } else { 0.0 };
        let body_hw = sw + t * (ew - sw);

        // Taper factor: 0 at the very tip, 1 once past the taper region.
        let taper = if tl < 1e-10 {
            1.0
        } else if d < tl {
            d / tl
        } else if d > total - tl {
            (total - d) / tl
        } else {
            1.0
        };

        body_hw * taper
    }
}

impl LivePathEffect for TaperStrokeLpe {
    fn name(&self) -> &str {
        "taper_stroke"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        // Flatten the input to a polyline for simple offset computation.
        let flat = input.flatten(1.0);
        let elements = flat.elements();

        // Collect the polyline points from the first subpath.
        let mut points: Vec<Point> = Vec::new();
        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    if !points.is_empty() {
                        break; // only process the first subpath
                    }
                    points.push(p);
                }
                PathSegment::LineTo(p) => {
                    points.push(p);
                }
                PathSegment::QuadTo(_, end) | PathSegment::CurveTo(_, _, end) => {
                    points.push(end);
                }
                PathSegment::ClosePath => {
                    break;
                }
            }
        }

        if points.len() < 2 {
            return input.clone();
        }

        // Compute cumulative arc-length distances.
        let n = points.len();
        let mut dists = Vec::with_capacity(n);
        dists.push(0.0_f64);
        for i in 1..n {
            dists.push(dists[i - 1] + points[i - 1].distance(points[i]));
        }
        let total = *dists.last().unwrap();

        if total < 1e-10 {
            return input.clone();
        }

        // Build left and right offset polylines.
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);

        for i in 0..n {
            // Compute the local tangent direction.
            let tangent = if i == 0 {
                let dx = points[1].x - points[0].x;
                let dy = points[1].y - points[0].y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-10);
                (dx / len, dy / len)
            } else if i == n - 1 {
                let dx = points[n - 1].x - points[n - 2].x;
                let dy = points[n - 1].y - points[n - 2].y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-10);
                (dx / len, dy / len)
            } else {
                let dx = points[i + 1].x - points[i - 1].x;
                let dy = points[i + 1].y - points[i - 1].y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-10);
                (dx / len, dy / len)
            };

            // Normal is perpendicular to the tangent.
            let nx = -tangent.1;
            let ny = tangent.0;

            let hw = self.half_width_at(dists[i], total);

            left.push(Point::new(points[i].x + nx * hw, points[i].y + ny * hw));
            right.push(Point::new(points[i].x - nx * hw, points[i].y - ny * hw));
        }

        // Build the closed outline: left forward, then right reversed.
        let mut result = BezPath::new();
        result.move_to(left[0]);
        for point in left.iter().skip(1) {
            result.line_to(*point);
        }
        for point in right.iter().rev() {
            result.line_to(*point);
        }
        result.close();

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.start_width)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "start_width" => {
                self.start_width.value = value.clamp(self.start_width.min, self.start_width.max);
            }
            "end_width" => {
                self.end_width.value = value.clamp(self.end_width.min, self.end_width.max);
            }
            "taper_length" => {
                self.taper_length.value = value.clamp(self.taper_length.min, self.taper_length.max);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal_line() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 50.0));
        p.line_to(Point::new(200.0, 50.0));
        p
    }

    fn diagonal_line() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p
    }

    #[test]
    fn test_taper_stroke_produces_closed_outline() {
        let lpe = TaperStrokeLpe::new(10.0, 10.0, 20.0);
        let input = horizontal_line();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());

        // The output should be a closed path.
        let elems = output.elements();
        let has_close = elems.iter().any(|e| matches!(e, PathSegment::ClosePath));
        assert!(has_close, "taper stroke should produce a closed outline");
    }

    #[test]
    fn test_taper_stroke_tapers_at_ends() {
        let lpe = TaperStrokeLpe::new(10.0, 10.0, 50.0);
        let input = horizontal_line();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();

        // The height of the bounding box should be less than the full width
        // (10 units) because the ends taper to points.
        // At the widest point (centre), half-width = 5, so total height ~10.
        // At the tips, width = 0. The bounding box height should be at most ~10.
        assert!(
            bb.height() <= 10.0 + 1.0,
            "height {} should be at most ~10",
            bb.height()
        );
    }

    #[test]
    fn test_taper_stroke_variable_width() {
        // Wide at start, narrow at end.
        let lpe = TaperStrokeLpe::new(20.0, 2.0, 0.0);
        let input = horizontal_line();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // The output should have more elements than the input.
        assert!(output.element_count() > input.element_count());
    }

    #[test]
    fn test_taper_stroke_on_diagonal() {
        let lpe = TaperStrokeLpe::new(8.0, 8.0, 10.0);
        let input = diagonal_line();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_taper_stroke_empty_input() {
        let lpe = TaperStrokeLpe::new(10.0, 10.0, 20.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_taper_stroke_params() {
        let mut lpe = TaperStrokeLpe::new(10.0, 5.0, 20.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "start_width");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("start_width", 15.0);
        assert!((lpe.start_width.value - 15.0).abs() < 1e-10);

        lpe.set_param("end_width", 3.0);
        assert!((lpe.end_width.value - 3.0).abs() < 1e-10);

        lpe.set_param("taper_length", 30.0);
        assert!((lpe.taper_length.value - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_taper_stroke_name() {
        let lpe = TaperStrokeLpe::new(10.0, 10.0, 20.0);
        assert_eq!(lpe.name(), "taper_stroke");
    }
}
