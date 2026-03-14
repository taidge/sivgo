//! Dashed Stroke Live Path Effect.
//!
//! Applies a dash pattern to the input path, converting it into
//! alternating visible and invisible segments. Uses the dash generation
//! facility from `BezPath::dash` in `sivgo-math`.

use super::{LivePathEffect, LpeParam};
use sivgo_math::BezPath;

/// Dashed Stroke LPE.
///
/// Converts the input path into a dashed version by applying a regular
/// dash-gap pattern. The resulting path contains only the "on" segments.
#[derive(Debug, Clone)]
pub struct DashedStrokeLpe {
    /// Length of each visible dash.
    dash_length: LpeParam,
    /// Length of each invisible gap.
    gap_length: LpeParam,
}

impl DashedStrokeLpe {
    /// Create a new Dashed Stroke effect.
    ///
    /// * `dash_length` - Length of visible dashes.
    /// * `gap_length` - Length of invisible gaps.
    pub fn new(dash_length: f64, gap_length: f64) -> Self {
        Self {
            dash_length: LpeParam::new("dash_length", dash_length, 0.1, 100_000.0),
            gap_length: LpeParam::new("gap_length", gap_length, 0.1, 100_000.0),
        }
    }
}

impl LivePathEffect for DashedStrokeLpe {
    fn name(&self) -> &str {
        "dashed_stroke"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let dash = self.dash_length.value;
        let gap = self.gap_length.value;

        input.dash(&[dash, gap], 0.0)
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.dash_length)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "dash_length" => {
                self.dash_length.value = value.clamp(self.dash_length.min, self.dash_length.max);
            }
            "gap_length" => {
                self.gap_length.value = value.clamp(self.gap_length.min, self.gap_length.max);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    fn line_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p
    }

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
    fn test_dashed_creates_segments() {
        let lpe = DashedStrokeLpe::new(10.0, 5.0);
        let input = line_path();
        let output = lpe.apply(&input);
        // A 100px line with 10px dash + 5px gap = ~7 dashes.
        assert!(
            output.element_count() > input.element_count(),
            "dashed should create segments: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_dashed_on_square() {
        let lpe = DashedStrokeLpe::new(20.0, 10.0);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        assert!(output.element_count() > input.element_count());
    }

    #[test]
    fn test_dashed_long_dash() {
        // Dash longer than the path -- should return approximately the full path.
        let lpe = DashedStrokeLpe::new(200.0, 10.0);
        let input = line_path();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_dashed_empty_path() {
        let lpe = DashedStrokeLpe::new(10.0, 5.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_dashed_params() {
        let mut lpe = DashedStrokeLpe::new(10.0, 5.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "dash_length");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("dash_length", 20.0);
        assert!((lpe.dash_length.value - 20.0).abs() < 1e-10);

        lpe.set_param("gap_length", 15.0);
        assert!((lpe.gap_length.value - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_dashed_name() {
        let lpe = DashedStrokeLpe::new(10.0, 5.0);
        assert_eq!(lpe.name(), "dashed_stroke");
    }

    #[test]
    fn test_dashed_equal_dash_gap() {
        let lpe = DashedStrokeLpe::new(10.0, 10.0);
        let input = line_path();
        let output = lpe.apply(&input);
        // 100px line / (10+10) = 5 dashes.
        // Each dash is a MoveTo + LineTo = 2 elements, so ~10 elements.
        assert!(
            output.element_count() >= 8,
            "got {}",
            output.element_count()
        );
    }
}
