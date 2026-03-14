//! Fillet / Chamfer Live Path Effect.
//!
//! Rounds (fillet) or bevels (chamfer) corners at junctions of line segments
//! in the input path.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Vector};

/// Whether corners should be rounded (fillet) or bevelled (chamfer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilletMode {
    /// Replace sharp corners with circular arcs.
    Fillet,
    /// Replace sharp corners with straight chamfer cuts.
    Chamfer,
}

/// Fillet / Chamfer LPE.
///
/// Operates on the line-segment corners of the (flattened) path. Curves are
/// first flattened, then corners are treated. For each corner the effect
/// inserts either a circular arc approximation (fillet) or a straight cut
/// (chamfer) at a distance of `radius` from the corner point along each
/// adjacent edge.
#[derive(Debug, Clone)]
pub struct FilletChamferLpe {
    radius: LpeParam,
    mode: FilletMode,
}

impl FilletChamferLpe {
    /// Create a new Fillet/Chamfer effect.
    pub fn new(radius: f64, mode: FilletMode) -> Self {
        Self {
            radius: LpeParam::new("radius", radius, 0.0, 10_000.0),
            mode,
        }
    }

    /// Current mode.
    pub fn mode(&self) -> FilletMode {
        self.mode
    }

    /// Set the mode.
    pub fn set_mode(&mut self, mode: FilletMode) {
        self.mode = mode;
    }
}

impl LivePathEffect for FilletChamferLpe {
    fn name(&self) -> &str {
        "fillet_chamfer"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let radius = self.radius.value;
        if radius <= 0.0 {
            return input.clone();
        }

        // Collect subpaths -- each subpath is a list of points and whether it
        // is closed.
        let elements = input.elements();
        let subpaths = collect_subpaths(&elements);

        let mut result = BezPath::new();

        for sp in &subpaths {
            if sp.points.len() < 2 {
                // Degenerate subpath -- keep as-is.
                if let Some(&pt) = sp.points.first() {
                    result.move_to(pt);
                }
                continue;
            }

            apply_fillet_chamfer_subpath(&sp.points, sp.closed, radius, self.mode, &mut result);
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.radius)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "radius" {
            self.radius.value = value.clamp(self.radius.min, self.radius.max);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// A collected subpath: sequence of points and whether it was closed.
struct Subpath {
    points: Vec<Point>,
    closed: bool,
}

/// Walk through `PathSegment`s and split into subpaths of points.
/// Curves are flattened to line-like sequences by evaluating the control
/// polygon endpoints (this is intentionally simple -- a production
/// implementation would flatten curves properly).
fn collect_subpaths(elements: &[PathSegment]) -> Vec<Subpath> {
    let mut subpaths: Vec<Subpath> = Vec::new();
    let mut current: Option<Subpath> = None;

    for el in elements {
        match *el {
            PathSegment::MoveTo(p) => {
                if let Some(sp) = current.take() {
                    subpaths.push(sp);
                }
                current = Some(Subpath {
                    points: vec![p],
                    closed: false,
                });
            }
            PathSegment::LineTo(p) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(p);
                }
            }
            PathSegment::QuadTo(_cp, end) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(end);
                }
            }
            PathSegment::CurveTo(_cp1, _cp2, end) => {
                if let Some(ref mut sp) = current {
                    sp.points.push(end);
                }
            }
            PathSegment::ClosePath => {
                if let Some(ref mut sp) = current {
                    sp.closed = true;
                    // Remove duplicate closing point if present.
                    if sp.points.len() >= 2 {
                        let first = sp.points[0];
                        let last = *sp.points.last().unwrap();
                        if (first.x - last.x).abs() < 1e-10 && (first.y - last.y).abs() < 1e-10 {
                            sp.points.pop();
                        }
                    }
                }
                if let Some(sp) = current.take() {
                    subpaths.push(sp);
                }
            }
        }
    }
    if let Some(sp) = current.take() {
        subpaths.push(sp);
    }
    subpaths
}

/// Process a single subpath, inserting fillet arcs or chamfer cuts.
fn apply_fillet_chamfer_subpath(
    points: &[Point],
    closed: bool,
    radius: f64,
    mode: FilletMode,
    out: &mut BezPath,
) {
    let n = points.len();
    if n < 2 {
        if let Some(&p) = points.first() {
            out.move_to(p);
        }
        return;
    }

    // For an open path with 2 points (single segment), there is nothing to
    // fillet -- just emit the segment.
    if !closed && n == 2 {
        out.move_to(points[0]);
        out.line_to(points[1]);
        return;
    }

    // For each corner i, compute the maximum allowed radius as
    // min(half-edge-before, half-edge-after) so arcs never overlap.
    let corner_count = if closed { n } else { n - 2 };

    // Pre-compute clamped radii per corner.
    let mut radii = vec![0.0f64; n];
    for ci in 0..corner_count {
        let i = if closed { ci } else { ci + 1 };
        let prev = if i == 0 { n - 1 } else { i - 1 };
        let next = if i == n - 1 { 0 } else { i + 1 };

        let len_before = points[i].distance(points[prev]);
        let len_after = points[i].distance(points[next]);
        let max_r = (len_before * 0.5).min(len_after * 0.5);
        radii[i] = radius.min(max_r);
    }

    // Walk points and emit segments.
    let start_idx: usize = if closed { 0 } else { 0 };
    let end_idx: usize = if closed { n } else { n };

    let mut first_point: Option<Point> = None;
    let mut moved = false;

    for ci in start_idx..end_idx {
        let is_corner = if closed { true } else { ci > 0 && ci < n - 1 };

        if is_corner && radii[ci] > 1e-10 {
            let prev = if ci == 0 { n - 1 } else { ci - 1 };
            let next = if ci == n - 1 { 0 } else { ci + 1 };

            let r = radii[ci];

            // Direction vectors from corner toward adjacent points.
            let dir_prev = unit_vec(points[ci], points[prev]);
            let dir_next = unit_vec(points[ci], points[next]);

            // Points on the edges at distance `r` from the corner.
            let p_in = offset_point(points[ci], dir_prev, r);
            let p_out = offset_point(points[ci], dir_next, r);

            // Emit line from previous position to p_in.
            if !moved {
                out.move_to(p_in);
                first_point = Some(p_in);
                moved = true;
            } else {
                out.line_to(p_in);
            }

            match mode {
                FilletMode::Chamfer => {
                    out.line_to(p_out);
                }
                FilletMode::Fillet => {
                    // Approximate the arc with a cubic Bezier.
                    // Use the standard circular-arc approximation via the
                    // tangent half-angle formula.
                    emit_arc_cubic(out, p_in, p_out, points[ci], dir_prev, dir_next);
                }
            }
        } else {
            // Not a corner (or radius is zero) -- emit point directly.
            if !moved {
                out.move_to(points[ci]);
                first_point = Some(points[ci]);
                moved = true;
            } else {
                out.line_to(points[ci]);
            }
        }
    }

    if closed {
        out.close();
    }
    let _ = first_point; // suppress unused warning
}

/// Unit vector from `from` toward `to`.
fn unit_vec(from: Point, to: Point) -> Vector {
    let v = to - from;
    let len = v.length();
    if len < 1e-12 {
        Vector::new(1.0, 0.0)
    } else {
        v * (1.0 / len)
    }
}

/// Move `origin` along `dir` by `dist`.
fn offset_point(origin: Point, dir: Vector, dist: f64) -> Point {
    Point::new(origin.x + dir.x * dist, origin.y + dir.y * dist)
}

/// Emit a cubic Bezier approximating a circular arc from `p0` to `p1` around
/// `corner`. `dir0` is the unit vector from `corner` toward p0's edge side and
/// `dir1` toward p1's edge side.
fn emit_arc_cubic(
    out: &mut BezPath,
    p0: Point,
    p1: Point,
    corner: Point,
    dir0: Vector,
    dir1: Vector,
) {
    // Compute the half-angle between the two edge directions.
    let dot = dir0.dot(dir1).clamp(-1.0, 1.0);
    let half_angle = ((1.0 - dot) / 2.0).sqrt().asin(); // == acos(dot)/2

    if half_angle.abs() < 1e-10 {
        // Nearly straight -- just draw a line.
        out.line_to(p1);
        return;
    }

    // The "kappa" factor for a circular arc approximated by a single cubic:
    //   k = (4/3) * tan(theta/2)
    // where theta is the arc angle. The arc angle is PI - angle_between_edges.
    let angle = (1.0 - dot).acos().abs(); // angle between edges (at corner)
    let arc_angle = std::f64::consts::PI - angle; // the supplementary angle is the arc sweep
    let k = (4.0 / 3.0) * (arc_angle / 4.0).tan();

    // Tangent directions at p0 and p1 (perpendicular to the radius,
    // oriented in the direction of travel around the arc).
    // The tangent at p0 points away from the incoming edge direction, rotated
    // toward p1.
    let tan0 = perp_toward(dir0, dir1);
    let tan1 = perp_toward(dir1, dir0);

    let r0 = p0.distance(corner);
    let cp0 = Point::new(p0.x + tan0.x * k * r0, p0.y + tan0.y * k * r0);
    let cp1 = Point::new(p1.x - tan1.x * k * r0, p1.y - tan1.y * k * r0);

    out.curve_to(cp0, cp1, p1);
}

/// Return the perpendicular of `v` that points toward the half-plane of `toward`.
fn perp_toward(v: Vector, toward: Vector) -> Vector {
    let p = v.perp(); // (-y, x)
    if p.dot(toward) > 0.0 {
        p
    } else {
        Vector::new(-p.x, -p.y)
    }
}
