//! Tiling Live Path Effect.
//!
//! Tiles the input path in a grid pattern, creating `rows * cols` copies
//! arranged in a regular grid with configurable spacing.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Transform2D};

/// Tiling LPE.
///
/// Creates a grid of copies of the input path. Each copy is translated by
/// `(col * spacing_x, row * spacing_y)` relative to the original.
#[derive(Debug, Clone)]
pub struct TilingLpe {
    /// Number of rows in the grid.
    rows: usize,
    /// Number of columns in the grid.
    cols: usize,
    /// Horizontal spacing between tile origins.
    spacing_x: LpeParam,
    /// Vertical spacing between tile origins.
    spacing_y: LpeParam,
}

impl TilingLpe {
    /// Create a new Tiling effect.
    ///
    /// * `rows` - Number of rows (minimum 1).
    /// * `cols` - Number of columns (minimum 1).
    /// * `spacing_x` - Horizontal distance between tile origins.
    /// * `spacing_y` - Vertical distance between tile origins.
    pub fn new(rows: usize, cols: usize, spacing_x: f64, spacing_y: f64) -> Self {
        Self {
            rows: rows.max(1),
            cols: cols.max(1),
            spacing_x: LpeParam::new("spacing_x", spacing_x, -100_000.0, 100_000.0),
            spacing_y: LpeParam::new("spacing_y", spacing_y, -100_000.0, 100_000.0),
        }
    }

    /// Set the number of rows.
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows.max(1);
    }

    /// Set the number of columns.
    pub fn set_cols(&mut self, cols: usize) {
        self.cols = cols.max(1);
    }
}

impl LivePathEffect for TilingLpe {
    fn name(&self) -> &str {
        "tiling"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let mut result = BezPath::new();

        for row in 0..self.rows {
            for col in 0..self.cols {
                let dx = col as f64 * self.spacing_x.value;
                let dy = row as f64 * self.spacing_y.value;

                if dx.abs() < 1e-12 && dy.abs() < 1e-12 {
                    // No translation needed -- append original.
                    result.extend(input);
                } else {
                    let transform = Transform2D::translate(dx, dy);
                    let translated = input.transformed(&transform);
                    result.extend(&translated);
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        // Return spacing_x as the primary parameter.
        std::slice::from_ref(&self.spacing_x)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "spacing_x" => {
                self.spacing_x.value = value.clamp(self.spacing_x.min, self.spacing_x.max);
            }
            "spacing_y" => {
                self.spacing_y.value = value.clamp(self.spacing_y.min, self.spacing_y.max);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        p.line_to(Point::new(0.0, 10.0));
        p.close();
        p
    }

    #[test]
    fn test_tiling_single_tile() {
        let lpe = TilingLpe::new(1, 1, 20.0, 20.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(output.element_count(), input.element_count());
    }

    #[test]
    fn test_tiling_2x2_grid() {
        let lpe = TilingLpe::new(2, 2, 20.0, 20.0);
        let input = square();
        let output = lpe.apply(&input);
        // 2 rows * 2 cols = 4 copies.
        assert_eq!(output.element_count(), input.element_count() * 4);
    }

    #[test]
    fn test_tiling_3x3_grid() {
        let lpe = TilingLpe::new(3, 3, 15.0, 15.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(output.element_count(), input.element_count() * 9);
    }

    #[test]
    fn test_tiling_bounding_box() {
        let lpe = TilingLpe::new(2, 3, 20.0, 20.0);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        // Rightmost tile at col=2: x offset = 40, plus 10 width = 50.
        assert!((bb.x1 - 50.0).abs() < 1e-6, "x1={}", bb.x1);
        // Bottom tile at row=1: y offset = 20, plus 10 height = 30.
        assert!((bb.y1 - 30.0).abs() < 1e-6, "y1={}", bb.y1);
    }

    #[test]
    fn test_tiling_empty_path() {
        let lpe = TilingLpe::new(2, 2, 10.0, 10.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_tiling_params() {
        let mut lpe = TilingLpe::new(2, 2, 20.0, 30.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "spacing_x");
        assert!((params[0].value - 20.0).abs() < 1e-10);

        lpe.set_param("spacing_x", 50.0);
        assert!((lpe.spacing_x.value - 50.0).abs() < 1e-10);

        lpe.set_param("spacing_y", 60.0);
        assert!((lpe.spacing_y.value - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_tiling_name() {
        let lpe = TilingLpe::new(1, 1, 10.0, 10.0);
        assert_eq!(lpe.name(), "tiling");
    }

    #[test]
    fn test_tiling_min_rows_cols() {
        // Zero rows/cols should be clamped to 1.
        let lpe = TilingLpe::new(0, 0, 10.0, 10.0);
        assert_eq!(lpe.rows, 1);
        assert_eq!(lpe.cols, 1);
    }
}
