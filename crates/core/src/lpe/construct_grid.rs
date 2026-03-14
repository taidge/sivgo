//! Construct Grid Live Path Effect.
//!
//! Generates a rectangular grid of lines. The grid is positioned at the
//! origin of the input path's bounding box and sized according to the
//! `width` and `height` parameters, divided into `rows` and `cols` cells.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Point};

/// Construct Grid LPE.
///
/// Replaces the input path with a rectangular grid. The grid starts at the
/// top-left corner of the input's bounding box and extends by the configured
/// `width` and `height`.
#[derive(Debug, Clone)]
pub struct ConstructGridLpe {
    /// Number of rows in the grid.
    rows: LpeParam,
    /// Number of columns in the grid.
    cols: LpeParam,
    /// Total width of the grid (in user units).
    width: LpeParam,
    /// Total height of the grid (in user units).
    height: LpeParam,
}

impl ConstructGridLpe {
    /// Create a new Construct Grid effect.
    ///
    /// * `rows` - Number of rows (minimum 1).
    /// * `cols` - Number of columns (minimum 1).
    /// * `width` - Total width of the grid.
    /// * `height` - Total height of the grid.
    pub fn new(rows: u32, cols: u32, width: f64, height: f64) -> Self {
        Self {
            rows: LpeParam::new("rows", rows as f64, 1.0, 10_000.0),
            cols: LpeParam::new("cols", cols as f64, 1.0, 10_000.0),
            width: LpeParam::new("width", width, 0.1, 100_000.0),
            height: LpeParam::new("height", height, 0.1, 100_000.0),
        }
    }
}

impl LivePathEffect for ConstructGridLpe {
    fn name(&self) -> &str {
        "construct_grid"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        // Determine the top-left origin from the input bounding box.
        let (ox, oy) = if input.is_empty() {
            (0.0, 0.0)
        } else {
            let bb = input.bounding_box();
            (bb.x0, bb.y0)
        };

        let rows = self.rows.value as usize;
        let cols = self.cols.value as usize;
        let w = self.width.value;
        let h = self.height.value;

        let cell_w = w / cols as f64;
        let cell_h = h / rows as f64;

        let mut result = BezPath::new();

        // Horizontal lines (rows + 1 lines).
        for r in 0..=rows {
            let y = oy + r as f64 * cell_h;
            result.move_to(Point::new(ox, y));
            result.line_to(Point::new(ox + w, y));
        }

        // Vertical lines (cols + 1 lines).
        for c in 0..=cols {
            let x = ox + c as f64 * cell_w;
            result.move_to(Point::new(x, oy));
            result.line_to(Point::new(x, oy + h));
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.rows)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "rows" => {
                self.rows.value = value.clamp(self.rows.min, self.rows.max);
            }
            "cols" => {
                self.cols.value = value.clamp(self.cols.min, self.cols.max);
            }
            "width" => {
                self.width.value = value.clamp(self.width.min, self.width.max);
            }
            "height" => {
                self.height.value = value.clamp(self.height.min, self.height.max);
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
    use sivgo_math::PathSegment;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(10.0, 20.0));
        p.line_to(Point::new(110.0, 20.0));
        p.line_to(Point::new(110.0, 120.0));
        p.line_to(Point::new(10.0, 120.0));
        p.close();
        p
    }

    #[test]
    fn test_grid_basic() {
        let lpe = ConstructGridLpe::new(3, 4, 100.0, 75.0);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());

        // 3 rows => 4 horizontal lines, 4 cols => 5 vertical lines = 9 lines.
        // Each line is 1 MoveTo + 1 LineTo = 2 elements. Total = 18 elements.
        let elems = output.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(
            move_count, 9,
            "3x4 grid should have 9 lines, got {}",
            move_count
        );
    }

    #[test]
    fn test_grid_1x1() {
        let lpe = ConstructGridLpe::new(1, 1, 50.0, 50.0);
        let input = square();
        let output = lpe.apply(&input);

        // 1 row => 2 horizontal lines, 1 col => 2 vertical lines = 4 lines.
        let elems = output.elements();
        let move_count = elems
            .iter()
            .filter(|e| matches!(e, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(
            move_count, 4,
            "1x1 grid should have 4 lines, got {}",
            move_count
        );
    }

    #[test]
    fn test_grid_positioned_at_bbox_origin() {
        let lpe = ConstructGridLpe::new(2, 2, 100.0, 100.0);
        let input = square(); // bbox starts at (10, 20)
        let output = lpe.apply(&input);

        // The first element should be a MoveTo at the bbox origin.
        let elems = output.elements();
        if let PathSegment::MoveTo(p) = elems[0] {
            assert!(
                (p.x - 10.0).abs() < 1e-10 && (p.y - 20.0).abs() < 1e-10,
                "grid should start at bbox origin (10, 20), got ({}, {})",
                p.x,
                p.y
            );
        } else {
            panic!("expected MoveTo as first element");
        }
    }

    #[test]
    fn test_grid_empty_input() {
        let lpe = ConstructGridLpe::new(2, 2, 100.0, 100.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        // Should still produce a grid (at origin).
        assert!(!output.is_empty());
    }

    #[test]
    fn test_grid_params() {
        let mut lpe = ConstructGridLpe::new(3, 4, 200.0, 150.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "rows");
        assert!((params[0].value - 3.0).abs() < 1e-10);

        lpe.set_param("rows", 5.0);
        assert!((lpe.rows.value - 5.0).abs() < 1e-10);

        lpe.set_param("cols", 6.0);
        assert!((lpe.cols.value - 6.0).abs() < 1e-10);

        lpe.set_param("width", 300.0);
        assert!((lpe.width.value - 300.0).abs() < 1e-10);

        lpe.set_param("height", 250.0);
        assert!((lpe.height.value - 250.0).abs() < 1e-10);
    }

    #[test]
    fn test_grid_name() {
        let lpe = ConstructGridLpe::new(2, 2, 100.0, 100.0);
        assert_eq!(lpe.name(), "construct_grid");
    }

    #[test]
    fn test_grid_more_rows_more_elements() {
        let lpe2 = ConstructGridLpe::new(2, 2, 100.0, 100.0);
        let lpe5 = ConstructGridLpe::new(5, 2, 100.0, 100.0);
        let input = square();
        let out2 = lpe2.apply(&input);
        let out5 = lpe5.apply(&input);
        assert!(
            out5.element_count() > out2.element_count(),
            "5 rows ({}) should produce more elements than 2 ({})",
            out5.element_count(),
            out2.element_count()
        );
    }
}
