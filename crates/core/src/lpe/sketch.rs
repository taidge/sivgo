//! Sketch Live Path Effect.
//!
//! Adds a hand-drawn look by generating multiple slightly displaced copies of
//! the input path. Each copy is offset by a random amount, and all copies are
//! combined into a single output path, giving the appearance of a rough pencil
//! sketch.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Sketch LPE.
///
/// Produces a sketchy, hand-drawn effect by drawing multiple copies of the
/// input path, each randomly displaced. The result looks like someone went
/// over the same stroke several times with slight wobble.
#[derive(Debug, Clone)]
pub struct SketchLpe {
    /// Number of displaced copies to draw.
    lines: LpeParam,
    /// Maximum displacement distance (in user units).
    max_displacement: LpeParam,
    /// Random seed for reproducible results.
    seed: u64,
}

impl SketchLpe {
    /// Create a new Sketch effect.
    ///
    /// * `lines` - Number of displaced copies (including the original).
    /// * `max_displacement` - Maximum random offset applied to each point.
    /// * `seed` - Seed for the pseudo-random number generator (deterministic).
    pub fn new(lines: u32, max_displacement: f64, seed: u64) -> Self {
        Self {
            lines: LpeParam::new("lines", lines as f64, 1.0, 100.0),
            max_displacement: LpeParam::new("max_displacement", max_displacement, 0.0, 10_000.0),
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

    /// Current number of displaced copies.
    pub fn lines(&self) -> u32 {
        self.lines.value as u32
    }
}

impl Default for SketchLpe {
    fn default() -> Self {
        Self::new(3, 2.0, 42)
    }
}

impl LivePathEffect for SketchLpe {
    fn name(&self) -> &str {
        "sketch"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let line_count = self.lines.value as usize;
        let max_disp = self.max_displacement.value;

        if line_count <= 1 && max_disp <= 0.0 {
            return input.clone();
        }

        // Flatten the input so we work with simple line segments.
        let flat = input.flatten(1.0);
        let elements = flat.elements();
        let subpaths = collect_subpaths(&elements);

        let mut result = BezPath::new();
        let mut rng = SimpleRng::new(self.seed);

        for copy_idx in 0..line_count {
            // The first copy (copy_idx == 0) can also be slightly displaced
            // to avoid one perfectly clean line.
            let disp_scale = if line_count == 1 {
                1.0
            } else {
                // Spread copies from 0 to max_displacement.
                (copy_idx as f64) / ((line_count - 1) as f64)
            };
            let effective_disp = max_disp * disp_scale;

            for sp in &subpaths {
                if sp.points.is_empty() {
                    continue;
                }

                let first = displace(sp.points[0], effective_disp, &mut rng);
                result.move_to(first);

                for i in 1..sp.points.len() {
                    result.line_to(displace(sp.points[i], effective_disp, &mut rng));
                }

                if sp.closed {
                    result.close();
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        // Return only the first param; callers can use set_param for any name.
        std::slice::from_ref(&self.lines)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "lines" => {
                self.lines.value = value.clamp(self.lines.min, self.lines.max);
            }
            "max_displacement" => {
                self.max_displacement.value =
                    value.clamp(self.max_displacement.min, self.max_displacement.max);
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

/// Displace a point by a random offset up to `max_disp`.
fn displace(p: Point, max_disp: f64, rng: &mut SimpleRng) -> Point {
    if max_disp <= 0.0 {
        return p;
    }
    let dx = (rng.next_f64() * 2.0 - 1.0) * max_disp;
    let dy = (rng.next_f64() * 2.0 - 1.0) * max_disp;
    Point::new(p.x + dx, p.y + dy)
}

// ---------------------------------------------------------------------------
// Simple deterministic PRNG (xorshift64)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    fn line() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p
    }

    #[test]
    fn test_sketch_name() {
        let lpe = SketchLpe::new(3, 2.0, 42);
        assert_eq!(lpe.name(), "sketch");
    }

    #[test]
    fn test_sketch_default() {
        let lpe = SketchLpe::default();
        assert_eq!(lpe.lines(), 3);
        assert_eq!(lpe.seed(), 42);
    }

    #[test]
    fn test_sketch_produces_multiple_copies() {
        let lpe = SketchLpe::new(3, 2.0, 42);
        let input = line();
        let output = lpe.apply(&input);

        // 3 copies of a line => 3 MoveTo elements.
        let elems = output.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(
            move_count, 3,
            "expected 3 copies (MoveTo), got {}",
            move_count
        );
    }

    #[test]
    fn test_sketch_on_square() {
        let lpe = SketchLpe::new(5, 3.0, 99);
        let input = square();
        let output = lpe.apply(&input);

        // 5 copies of the square.
        let elems = output.elements();
        let close_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::ClosePath))
            .count();
        assert_eq!(
            close_count, 5,
            "expected 5 closed copies, got {}",
            close_count
        );
    }

    #[test]
    fn test_sketch_deterministic() {
        let lpe = SketchLpe::new(3, 5.0, 42);
        let input = square();
        let output1 = lpe.apply(&input);
        let output2 = lpe.apply(&input);
        assert_eq!(output1.to_svg_string(), output2.to_svg_string());
    }

    #[test]
    fn test_sketch_different_seeds() {
        let lpe1 = SketchLpe::new(3, 5.0, 42);
        let lpe2 = SketchLpe::new(3, 5.0, 99);
        let input = square();
        let output1 = lpe1.apply(&input);
        let output2 = lpe2.apply(&input);
        assert_ne!(output1.to_svg_string(), output2.to_svg_string());
    }

    #[test]
    fn test_sketch_single_line_zero_displacement() {
        let lpe = SketchLpe::new(1, 0.0, 42);
        let input = line();
        let output = lpe.apply(&input);
        // With 1 line and 0 displacement, should be identical to input
        // (after flattening, which for a line is identity).
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_sketch_params() {
        let mut lpe = SketchLpe::new(3, 2.0, 42);
        let params = lpe.params();
        assert_eq!(params[0].name, "lines");
        assert!((params[0].value - 3.0).abs() < 1e-10);

        lpe.set_param("lines", 7.0);
        assert!((lpe.lines.value - 7.0).abs() < 1e-10);

        lpe.set_param("max_displacement", 15.0);
        assert!((lpe.max_displacement.value - 15.0).abs() < 1e-10);

        // Clamped to max
        lpe.set_param("lines", 200.0);
        assert!((lpe.lines.value - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_sketch_set_seed() {
        let mut lpe = SketchLpe::new(3, 2.0, 42);
        lpe.set_seed(123);
        assert_eq!(lpe.seed(), 123);
    }

    #[test]
    fn test_sketch_on_single_point() {
        let mut input = BezPath::new();
        input.move_to(Point::new(50.0, 50.0));

        let lpe = SketchLpe::new(3, 2.0, 42);
        let output = lpe.apply(&input);
        // Single point has no line segments, so each copy is just a MoveTo.
        assert!(!output.is_empty());
    }

    #[test]
    fn test_sketch_more_copies_more_elements() {
        let lpe3 = SketchLpe::new(3, 2.0, 42);
        let lpe5 = SketchLpe::new(5, 2.0, 42);
        let input = line();
        let out3 = lpe3.apply(&input);
        let out5 = lpe5.apply(&input);
        assert!(
            out5.element_count() > out3.element_count(),
            "5 copies should produce more elements than 3: {} vs {}",
            out5.element_count(),
            out3.element_count()
        );
    }
}
