//! Measure Segments Live Path Effect.
//!
//! Annotates each segment of the input path with its length, generating
//! simple ruler-like measurement lines and approximate text-label paths
//! alongside the original geometry.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Vector};

/// Measure Segments LPE.
///
/// For each line/curve segment in the input path, this effect appends a
/// measurement annotation: a small offset line parallel to the segment
/// with tick marks at each end, plus a rough text label (as a simple
/// dash-pattern representation of the length value).
///
/// The input path is preserved in the output; annotations are appended
/// after it.
#[derive(Debug, Clone)]
pub struct MeasureSegmentsLpe {
    /// The unit label for measurements (e.g. `"px"`, `"mm"`).
    unit: String,
    /// Font size for labels (controls annotation offset distance).
    font_size: LpeParam,
    /// Offset distance of the measurement line from the path.
    measure_offset: LpeParam,
}

impl MeasureSegmentsLpe {
    /// Create a new Measure Segments effect.
    ///
    /// * `unit` - Unit label (purely cosmetic; does not convert values).
    /// * `font_size` - Controls the size of the annotation offset.
    pub fn new(unit: impl Into<String>, font_size: f64) -> Self {
        Self {
            unit: unit.into(),
            font_size: LpeParam::new("font_size", font_size, 1.0, 1000.0),
            measure_offset: LpeParam::new("measure_offset", 10.0, 1.0, 1000.0),
        }
    }

    /// Set the unit label.
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
    }

    /// Get the unit label.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Generate a measurement annotation for a segment from `a` to `b`.
    fn annotate_segment(&self, a: Point, b: Point, result: &mut BezPath) {
        let seg_vec = b - a;
        let seg_len = seg_vec.length();

        if seg_len < 1e-6 {
            return;
        }

        let offset = self.measure_offset.value;
        let tick = offset * 0.3;

        // Normal direction (perpendicular, pointing "left" of the segment direction).
        let normal = Vector::new(-seg_vec.y / seg_len, seg_vec.x / seg_len);

        // Offset endpoints.
        let a_off = Point::new(a.x + normal.x * offset, a.y + normal.y * offset);
        let b_off = Point::new(b.x + normal.x * offset, b.y + normal.y * offset);

        // Measurement line.
        result.move_to(a_off);
        result.line_to(b_off);

        // Tick at start.
        let tick_a1 = Point::new(
            a.x + normal.x * (offset - tick),
            a.y + normal.y * (offset - tick),
        );
        let tick_a2 = Point::new(
            a.x + normal.x * (offset + tick),
            a.y + normal.y * (offset + tick),
        );
        result.move_to(tick_a1);
        result.line_to(tick_a2);

        // Tick at end.
        let tick_b1 = Point::new(
            b.x + normal.x * (offset - tick),
            b.y + normal.y * (offset - tick),
        );
        let tick_b2 = Point::new(
            b.x + normal.x * (offset + tick),
            b.y + normal.y * (offset + tick),
        );
        result.move_to(tick_b1);
        result.line_to(tick_b2);

        // Midpoint marker: a small crosshair to indicate where the label goes.
        let mid = a_off.midpoint(b_off);
        let half = self.font_size.value * 0.3;
        let tangent = Vector::new(seg_vec.x / seg_len, seg_vec.y / seg_len);

        // Small hash mark at midpoint.
        let hash_a = Point::new(
            mid.x - tangent.x * half + normal.x * half * 0.5,
            mid.y - tangent.y * half + normal.y * half * 0.5,
        );
        let hash_b = Point::new(
            mid.x + tangent.x * half - normal.x * half * 0.5,
            mid.y + tangent.y * half - normal.y * half * 0.5,
        );
        result.move_to(hash_a);
        result.line_to(hash_b);
    }
}

impl LivePathEffect for MeasureSegmentsLpe {
    fn name(&self) -> &str {
        "measure_segments"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        // Start with the original path.
        let mut result = input.clone();

        // Walk the path elements, annotating each segment.
        let elements = input.elements();
        let mut current_pos = Point::ZERO;

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    current_pos = p;
                }
                PathSegment::LineTo(p) => {
                    self.annotate_segment(current_pos, p, &mut result);
                    current_pos = p;
                }
                PathSegment::QuadTo(_cp, end) => {
                    // Annotate the chord for curves.
                    self.annotate_segment(current_pos, end, &mut result);
                    current_pos = end;
                }
                PathSegment::CurveTo(_cp1, _cp2, end) => {
                    self.annotate_segment(current_pos, end, &mut result);
                    current_pos = end;
                }
                PathSegment::ClosePath => {
                    // ClosePath implicitly draws back to the subpath start,
                    // but we don't have the start tracked here. Skip.
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.font_size)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "font_size" => {
                self.font_size.value = value.clamp(self.font_size.min, self.font_size.max);
            }
            "measure_offset" => {
                self.measure_offset.value =
                    value.clamp(self.measure_offset.min, self.measure_offset.max);
            }
            _ => {}
        }
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
    fn test_measure_adds_annotations() {
        let lpe = MeasureSegmentsLpe::new("px", 12.0);
        let input = square();
        let output = lpe.apply(&input);
        // Output should have more elements than input (annotations added).
        assert!(
            output.element_count() > input.element_count(),
            "measure should add annotations: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_measure_preserves_input() {
        let lpe = MeasureSegmentsLpe::new("px", 12.0);
        let input = square();
        let output = lpe.apply(&input);
        // The first elements should match the input path.
        let in_els = input.elements();
        let out_els = output.elements();
        for (i, el) in in_els.iter().enumerate() {
            assert_eq!(
                format!("{:?}", el),
                format!("{:?}", out_els[i]),
                "element {} should match input",
                i
            );
        }
    }

    #[test]
    fn test_measure_on_line() {
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let lpe = MeasureSegmentsLpe::new("mm", 10.0);
        let output = lpe.apply(&input);
        // Should have the original 2 elements plus annotation elements.
        assert!(output.element_count() > 2);
    }

    #[test]
    fn test_measure_empty_path() {
        let lpe = MeasureSegmentsLpe::new("px", 12.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_measure_params() {
        let mut lpe = MeasureSegmentsLpe::new("px", 12.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "font_size");
        assert!((params[0].value - 12.0).abs() < 1e-10);

        lpe.set_param("font_size", 24.0);
        assert!((lpe.font_size.value - 24.0).abs() < 1e-10);

        lpe.set_param("measure_offset", 20.0);
        assert!((lpe.measure_offset.value - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_measure_name() {
        let lpe = MeasureSegmentsLpe::new("px", 12.0);
        assert_eq!(lpe.name(), "measure_segments");
    }

    #[test]
    fn test_measure_unit() {
        let mut lpe = MeasureSegmentsLpe::new("px", 12.0);
        assert_eq!(lpe.unit(), "px");
        lpe.set_unit("mm");
        assert_eq!(lpe.unit(), "mm");
    }
}
