//! Lattice Live Path Effect.
//!
//! Warps the input path through a deformable grid. The grid subdivides
//! the bounding box into `grid_rows * grid_cols` cells, and each grid
//! vertex can be displaced to create smooth local distortions.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point};

/// Lattice LPE.
///
/// Divides the input path's bounding box into a grid and displaces grid
/// vertices. Points between vertices are interpolated using bilinear
/// interpolation within the enclosing cell.
#[derive(Debug, Clone)]
pub struct LatticeLpe {
    /// Number of grid rows (number of cells vertically).
    grid_rows: usize,
    /// Number of grid columns (number of cells horizontally).
    grid_cols: usize,
    /// Displacement vectors for each grid vertex.
    /// Stored row-major: `(grid_rows + 1) * (grid_cols + 1)` entries.
    /// Each `Point` represents the *destination* position of the vertex.
    deformations: Vec<Point>,
    /// Exposed param for the trait.
    strength: LpeParam,
}

impl LatticeLpe {
    /// Create a new Lattice effect.
    ///
    /// * `grid_rows` - Number of rows (minimum 1).
    /// * `grid_cols` - Number of columns (minimum 1).
    /// * `deformations` - Displacement offsets for each grid vertex. Must have
    ///   `(grid_rows + 1) * (grid_cols + 1)` entries; extra entries are ignored,
    ///   missing entries default to zero displacement.
    pub fn new(grid_rows: usize, grid_cols: usize, deformations: Vec<Point>) -> Self {
        let rows = grid_rows.max(1);
        let cols = grid_cols.max(1);
        Self {
            grid_rows: rows,
            grid_cols: cols,
            deformations,
            strength: LpeParam::new("strength", 1.0, 0.0, 10.0),
        }
    }

    /// Create a lattice with no deformation (identity).
    pub fn identity(grid_rows: usize, grid_cols: usize) -> Self {
        let rows = grid_rows.max(1);
        let cols = grid_cols.max(1);
        let n = (rows + 1) * (cols + 1);
        Self::new(rows, cols, vec![Point::ZERO; n])
    }

    /// Get the displacement for vertex at grid position `(row, col)`.
    fn get_deformation(&self, row: usize, col: usize) -> Point {
        let idx = row * (self.grid_cols + 1) + col;
        self.deformations.get(idx).copied().unwrap_or(Point::ZERO)
    }

    /// Map a point from input space through the lattice deformation.
    fn map_point(&self, p: Point, x0: f64, y0: f64, w: f64, h: f64) -> Point {
        let strength = self.strength.value;

        // Normalise to grid coordinates.
        let gx = if w.abs() > 1e-12 {
            ((p.x - x0) / w) * self.grid_cols as f64
        } else {
            0.0
        };
        let gy = if h.abs() > 1e-12 {
            ((p.y - y0) / h) * self.grid_rows as f64
        } else {
            0.0
        };

        // Find the enclosing cell.
        let col = (gx.floor() as usize).min(self.grid_cols.saturating_sub(1));
        let row = (gy.floor() as usize).min(self.grid_rows.saturating_sub(1));

        // Local coordinates within the cell [0, 1].
        let u = (gx - col as f64).clamp(0.0, 1.0);
        let v = (gy - row as f64).clamp(0.0, 1.0);

        // Get the four corner displacements.
        let d00 = self.get_deformation(row, col);
        let d01 = self.get_deformation(row, col + 1);
        let d10 = self.get_deformation(row + 1, col);
        let d11 = self.get_deformation(row + 1, col + 1);

        // Bilinear interpolation of displacements.
        let dx = (1.0 - u) * (1.0 - v) * d00.x
            + u * (1.0 - v) * d01.x
            + (1.0 - u) * v * d10.x
            + u * v * d11.x;
        let dy = (1.0 - u) * (1.0 - v) * d00.y
            + u * (1.0 - v) * d01.y
            + (1.0 - u) * v * d10.y
            + u * v * d11.y;

        Point::new(p.x + dx * strength, p.y + dy * strength)
    }
}

impl LivePathEffect for LatticeLpe {
    fn name(&self) -> &str {
        "lattice"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let bb = input.bounding_box();
        let w = bb.width();
        let h = bb.height();

        let elements = input.elements();
        let mut result = BezPath::new();

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    result.move_to(self.map_point(p, bb.x0, bb.y0, w, h));
                }
                PathSegment::LineTo(p) => {
                    result.line_to(self.map_point(p, bb.x0, bb.y0, w, h));
                }
                PathSegment::QuadTo(cp, end) => {
                    result.quad_to(
                        self.map_point(cp, bb.x0, bb.y0, w, h),
                        self.map_point(end, bb.x0, bb.y0, w, h),
                    );
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    result.curve_to(
                        self.map_point(cp1, bb.x0, bb.y0, w, h),
                        self.map_point(cp2, bb.x0, bb.y0, w, h),
                        self.map_point(end, bb.x0, bb.y0, w, h),
                    );
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.strength)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "strength" {
            self.strength.value = value.clamp(self.strength.min, self.strength.max);
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
    fn test_lattice_identity() {
        // Zero deformations should produce identity.
        let lpe = LatticeLpe::identity(2, 2);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_lattice_uniform_shift() {
        // Shift all grid points by (10, 20).
        let rows = 1;
        let cols = 1;
        let n = (rows + 1) * (cols + 1);
        let deformations = vec![Point::new(10.0, 20.0); n];
        let lpe = LatticeLpe::new(rows, cols, deformations);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!((bb.x0 - 10.0).abs() < 1e-6, "x0={}", bb.x0);
        assert!((bb.y0 - 20.0).abs() < 1e-6, "y0={}", bb.y0);
    }

    #[test]
    fn test_lattice_partial_deformation() {
        // 1x1 grid, only move top-left corner.
        let mut deformations = vec![Point::ZERO; 4];
        deformations[0] = Point::new(10.0, 10.0); // top-left shifted
        let lpe = LatticeLpe::new(1, 1, deformations);
        let input = square();
        let output = lpe.apply(&input);
        // Output should differ from input.
        assert_ne!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_lattice_strength_zero() {
        // Strength 0 should produce identity even with deformations.
        let deformations = vec![Point::new(50.0, 50.0); 4];
        let mut lpe = LatticeLpe::new(1, 1, deformations);
        lpe.set_param("strength", 0.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_lattice_empty_path() {
        let lpe = LatticeLpe::identity(2, 2);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_lattice_params() {
        let mut lpe = LatticeLpe::identity(2, 2);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "strength");
        assert!((params[0].value - 1.0).abs() < 1e-10);

        lpe.set_param("strength", 5.0);
        assert!((lpe.strength.value - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_lattice_name() {
        let lpe = LatticeLpe::identity(1, 1);
        assert_eq!(lpe.name(), "lattice");
    }

    #[test]
    fn test_lattice_2x2_grid() {
        // 2x2 grid with non-zero deformations at some vertices.
        let rows = 2;
        let cols = 2;
        let n = (rows + 1) * (cols + 1); // 9 vertices
        let mut deformations = vec![Point::ZERO; n];
        // Shift the center vertex.
        deformations[4] = Point::new(5.0, 5.0);
        let lpe = LatticeLpe::new(rows, cols, deformations);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }
}
