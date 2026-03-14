//! Roughen Live Path Effect.
//!
//! Adds random noise to path segments, giving an organic, hand-drawn
//! appearance. The path is first subdivided into segments of a target length,
//! then each node is displaced by a random offset.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Roughen LPE.
///
/// Subdivides the input path into small segments and displaces each node by
/// a random amount, producing a jittery, hand-drawn aesthetic.
#[derive(Debug, Clone)]
pub struct RoughenLpe {
    /// Maximum displacement distance (in user units).
    max_displacement: LpeParam,
    /// Target length for each subdivided segment (in user units).
    segment_length: LpeParam,
    /// Random seed for reproducible results.
    seed: u64,
}

impl RoughenLpe {
    /// Create a new Roughen effect.
    ///
    /// * `max_displacement` - Maximum random offset applied to each node.
    /// * `segment_length` - The path is subdivided so each segment is
    ///   approximately this long before displacement is applied.
    /// * `seed` - Seed for the pseudo-random number generator (deterministic).
    pub fn new(max_displacement: f64, segment_length: f64, seed: u64) -> Self {
        Self {
            max_displacement: LpeParam::new("max_displacement", max_displacement, 0.0, 100_000.0),
            segment_length: LpeParam::new("segment_length", segment_length, 0.1, 100_000.0),
            seed,
        }
    }

    /// Set the random seed.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    /// Get the current seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl LivePathEffect for RoughenLpe {
    fn name(&self) -> &str {
        "roughen"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let max_disp = self.max_displacement.value;
        if max_disp <= 0.0 {
            return input.clone();
        }

        let seg_len = self.segment_length.value;

        // Flatten the input to line segments.
        let flat = input.flatten(seg_len * 0.25);
        let elements = flat.elements();
        let subpaths = collect_subpaths(&elements);

        let mut rng = SimpleRng::new(self.seed);
        let mut result = BezPath::new();

        for sp in &subpaths {
            if sp.points.len() < 2 {
                if let Some(&pt) = sp.points.first() {
                    result.move_to(pt);
                }
                continue;
            }

            // Subdivide into segments of approximately `seg_len`.
            let subdivided = subdivide_polyline(&sp.points, seg_len);

            // Displace each interior point randomly.
            let n = subdivided.len();
            result.move_to(displace(subdivided[0], max_disp, &mut rng));

            for i in 1..n {
                result.line_to(displace(subdivided[i], max_disp, &mut rng));
            }

            if sp.closed {
                result.close();
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.max_displacement)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "max_displacement" => {
                self.max_displacement.value =
                    value.clamp(self.max_displacement.min, self.max_displacement.max)
            }
            "segment_length" => {
                self.segment_length.value =
                    value.clamp(self.segment_length.min, self.segment_length.max)
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct Subpath {
    points: Vec<Point>,
    closed: bool,
}

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
                    if sp.points.len() >= 2 {
                        let first = sp.points[0];
                        let last = *sp.points.last().unwrap();
                        if first.distance(last) < 1e-10 {
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

/// Subdivide a polyline so that each resulting segment is at most `max_len`
/// long. Extra points are inserted via linear interpolation.
fn subdivide_polyline(points: &[Point], max_len: f64) -> Vec<Point> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut result = vec![points[0]];

    for i in 1..points.len() {
        let a = points[i - 1];
        let b = points[i];
        let dist = a.distance(b);

        if dist <= max_len {
            result.push(b);
        } else {
            let n = (dist / max_len).ceil() as usize;
            for j in 1..=n {
                let t = j as f64 / n as f64;
                result.push(a.lerp(b, t));
            }
        }
    }

    result
}

/// Displace a point by a random offset up to `max_disp`.
fn displace(p: Point, max_disp: f64, rng: &mut SimpleRng) -> Point {
    let dx = (rng.next_f64() * 2.0 - 1.0) * max_disp;
    let dy = (rng.next_f64() * 2.0 - 1.0) * max_disp;
    Point::new(p.x + dx, p.y + dy)
}

// ---------------------------------------------------------------------------
// Simple deterministic PRNG (xorshift64)
// ---------------------------------------------------------------------------

/// A minimal xorshift64 PRNG for deterministic roughening.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Returns a f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
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
    fn test_roughen_adds_segments() {
        let lpe = RoughenLpe::new(5.0, 10.0, 42);
        let input = square();
        let output = lpe.apply(&input);

        // Roughening with small segment_length should create more segments.
        assert!(
            output.element_count() > input.element_count(),
            "roughen should add segments: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_roughen_deterministic() {
        let lpe = RoughenLpe::new(5.0, 10.0, 42);
        let input = square();
        let output1 = lpe.apply(&input);
        let output2 = lpe.apply(&input);

        // Same seed should produce same result.
        assert_eq!(output1.to_svg_string(), output2.to_svg_string());
    }

    #[test]
    fn test_roughen_different_seeds() {
        let lpe1 = RoughenLpe::new(5.0, 10.0, 42);
        let lpe2 = RoughenLpe::new(5.0, 10.0, 99);
        let input = square();
        let output1 = lpe1.apply(&input);
        let output2 = lpe2.apply(&input);

        // Different seeds should (almost certainly) produce different results.
        assert_ne!(output1.to_svg_string(), output2.to_svg_string());
    }

    #[test]
    fn test_roughen_zero_displacement_is_identity() {
        let lpe = RoughenLpe::new(0.0, 10.0, 42);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_roughen_params() {
        let mut lpe = RoughenLpe::new(5.0, 10.0, 42);
        let params = lpe.params();
        assert_eq!(params[0].name, "max_displacement");

        lpe.set_param("max_displacement", 15.0);
        assert!((lpe.max_displacement.value - 15.0).abs() < 1e-10);

        lpe.set_param("segment_length", 20.0);
        assert!((lpe.segment_length.value - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_roughen_on_line() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = RoughenLpe::new(3.0, 5.0, 123);
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        assert!(output.element_count() > input.element_count());
    }

    #[test]
    fn test_roughen_name() {
        let lpe = RoughenLpe::new(5.0, 10.0, 1);
        assert_eq!(lpe.name(), "roughen");
    }

    #[test]
    fn test_subdivide_polyline() {
        let points = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        let result = subdivide_polyline(&points, 25.0);
        // 100 / 25 = 4 segments => 5 points
        assert_eq!(result.len(), 5);
        assert!((result[0].x - 0.0).abs() < 1e-10);
        assert!((result[4].x - 100.0).abs() < 1e-10);
    }
}
