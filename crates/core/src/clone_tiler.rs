//! Clone tiler — generate tiled patterns of cloned objects.
//!
//! Implements the "Create Tiled Clones" dialog functionality from Inkscape.
//! Given a source object, produces a grid of `<use>` clones with optional
//! per-clone transformations (shift, scale, rotation, opacity).

use sivgo_math::{Point, Transform2D};

/// Symmetry group for tiling.
///
/// These correspond to the 17 wallpaper groups (2D crystallographic groups)
/// used by Inkscape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymmetryGroup {
    /// P1 — Simple translation (no symmetry)
    P1,
    /// P2 — 180° rotation
    P2,
    /// PM — Reflection
    PM,
    /// PG — Glide reflection
    PG,
    /// CM — Reflection + glide
    CM,
    /// PMM — Two perpendicular reflections
    PMM,
    /// PMG — Reflection + glide perpendicular
    PMG,
    /// PGG — Two perpendicular glide reflections
    PGG,
    /// CMM — Reflection + rotation 180°
    CMM,
    /// P4 — 90° rotation
    P4,
    /// P4M — 90° rotation + reflection
    P4M,
    /// P4G — 90° rotation + glide
    P4G,
    /// P3 — 120° rotation
    P3,
    /// P3M1 — 120° rotation + reflection (mirror axis through edge)
    P3M1,
    /// P31M — 120° rotation + reflection (mirror axis through vertex)
    P31M,
    /// P6 — 60° rotation
    P6,
    /// P6M — 60° rotation + reflection
    P6M,
}

/// Parameters for creating tiled clones.
#[derive(Debug, Clone)]
pub struct TileParams {
    /// Symmetry group to use.
    pub symmetry: SymmetryGroup,
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub columns: usize,

    // Shift
    /// Horizontal shift per row (fraction of tile width). Default: 0.0.
    pub shift_x_per_row: f64,
    /// Vertical shift per column (fraction of tile height). Default: 0.0.
    pub shift_y_per_col: f64,

    // Scale
    /// Scale factor change per row. Default: 0.0 (no scaling).
    pub scale_x_per_row: f64,
    /// Scale factor change per column. Default: 0.0.
    pub scale_y_per_col: f64,

    // Rotation
    /// Rotation in degrees per row. Default: 0.0.
    pub rotation_per_row: f64,
    /// Rotation in degrees per column. Default: 0.0.
    pub rotation_per_col: f64,

    // Opacity
    /// Opacity change per row (additive). Default: 0.0.
    pub opacity_per_row: f64,
    /// Opacity change per column. Default: 0.0.
    pub opacity_per_col: f64,

    /// Tile width (in document units). Defaults to source bbox width.
    pub tile_width: f64,
    /// Tile height (in document units). Defaults to source bbox height.
    pub tile_height: f64,
}

impl Default for TileParams {
    fn default() -> Self {
        Self {
            symmetry: SymmetryGroup::P1,
            rows: 3,
            columns: 3,
            shift_x_per_row: 0.0,
            shift_y_per_col: 0.0,
            scale_x_per_row: 0.0,
            scale_y_per_col: 0.0,
            rotation_per_row: 0.0,
            rotation_per_col: 0.0,
            opacity_per_row: 0.0,
            opacity_per_col: 0.0,
            tile_width: 100.0,
            tile_height: 100.0,
        }
    }
}

/// A single tile clone placement.
#[derive(Debug, Clone)]
pub struct TilePlacement {
    /// Transform to apply to the clone.
    pub transform: Transform2D,
    /// Opacity for this clone (0.0–1.0).
    pub opacity: f64,
}

/// Generate tile placements for the given parameters.
///
/// Returns a list of `TilePlacement` values, one per clone to create.
/// The caller creates `<use>` elements referencing the source, with the
/// given transforms and opacities.
pub fn generate_tile_placements(params: &TileParams) -> Vec<TilePlacement> {
    let mut placements = Vec::new();

    let base_transforms = symmetry_base_transforms(params.symmetry);

    for row in 0..params.rows {
        for col in 0..params.columns {
            let r = row as f64;
            let c = col as f64;

            // Base translation for this grid cell
            let tx = c * params.tile_width + r * params.shift_x_per_row * params.tile_width;
            let ty = r * params.tile_height + c * params.shift_y_per_col * params.tile_height;

            // Per-cell scale
            let sx = 1.0 + r * params.scale_x_per_row + c * params.scale_x_per_row * 0.0;
            let sy = 1.0 + r * params.scale_y_per_col * 0.0 + c * params.scale_y_per_col;

            // Per-cell rotation
            let rot_deg = r * params.rotation_per_row + c * params.rotation_per_col;

            // Opacity
            let opacity =
                (1.0 + r * params.opacity_per_row + c * params.opacity_per_col).clamp(0.0, 1.0);

            // Apply symmetry transforms
            for base in &base_transforms {
                // Combine: translate to position, then apply symmetry, then scale/rotate
                let cell_center =
                    Point::new(tx + params.tile_width / 2.0, ty + params.tile_height / 2.0);

                let mut t = Transform2D::translate(tx, ty);
                t = t.then(base);

                if (sx - 1.0).abs() > 1e-10 || (sy - 1.0).abs() > 1e-10 {
                    let scale = Transform2D::translate(-cell_center.x, -cell_center.y)
                        .then(&Transform2D::scale_xy(sx, sy))
                        .then(&Transform2D::translate(cell_center.x, cell_center.y));
                    t = t.then(&scale);
                }

                if rot_deg.abs() > 1e-10 {
                    let rot = Transform2D::translate(-cell_center.x, -cell_center.y)
                        .then(&Transform2D::rotate(rot_deg.to_radians()))
                        .then(&Transform2D::translate(cell_center.x, cell_center.y));
                    t = t.then(&rot);
                }

                placements.push(TilePlacement {
                    transform: t,
                    opacity,
                });
            }
        }
    }

    placements
}

/// Return the base transforms for a symmetry group within one tile cell.
fn symmetry_base_transforms(sym: SymmetryGroup) -> Vec<Transform2D> {
    match sym {
        SymmetryGroup::P1 => {
            vec![Transform2D::IDENTITY]
        }
        SymmetryGroup::P2 => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(std::f64::consts::PI),
            ]
        }
        SymmetryGroup::PM => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::scale_xy(-1.0, 1.0), // horizontal reflection
            ]
        }
        SymmetryGroup::PG => {
            vec![
                Transform2D::IDENTITY,
                // Glide reflection: reflect + translate half-cell
                Transform2D::scale_xy(-1.0, 1.0),
            ]
        }
        SymmetryGroup::CM => {
            vec![Transform2D::IDENTITY, Transform2D::scale_xy(-1.0, 1.0)]
        }
        SymmetryGroup::PMM => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::scale_xy(-1.0, 1.0),
                Transform2D::scale_xy(1.0, -1.0),
                Transform2D::scale_xy(-1.0, -1.0),
            ]
        }
        SymmetryGroup::PMG | SymmetryGroup::PGG | SymmetryGroup::CMM => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(std::f64::consts::PI),
                Transform2D::scale_xy(-1.0, 1.0),
                Transform2D::scale_xy(1.0, -1.0),
            ]
        }
        SymmetryGroup::P4 => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(std::f64::consts::FRAC_PI_2),
                Transform2D::rotate(std::f64::consts::PI),
                Transform2D::rotate(3.0 * std::f64::consts::FRAC_PI_2),
            ]
        }
        SymmetryGroup::P4M | SymmetryGroup::P4G => {
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(std::f64::consts::FRAC_PI_2),
                Transform2D::rotate(std::f64::consts::PI),
                Transform2D::rotate(3.0 * std::f64::consts::FRAC_PI_2),
                Transform2D::scale_xy(-1.0, 1.0),
                Transform2D::scale_xy(-1.0, 1.0)
                    .then(&Transform2D::rotate(std::f64::consts::FRAC_PI_2)),
                Transform2D::scale_xy(-1.0, 1.0).then(&Transform2D::rotate(std::f64::consts::PI)),
                Transform2D::scale_xy(-1.0, 1.0)
                    .then(&Transform2D::rotate(3.0 * std::f64::consts::FRAC_PI_2)),
            ]
        }
        SymmetryGroup::P3 => {
            let a = 2.0 * std::f64::consts::PI / 3.0;
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(a),
                Transform2D::rotate(2.0 * a),
            ]
        }
        SymmetryGroup::P3M1 | SymmetryGroup::P31M => {
            let a = 2.0 * std::f64::consts::PI / 3.0;
            vec![
                Transform2D::IDENTITY,
                Transform2D::rotate(a),
                Transform2D::rotate(2.0 * a),
                Transform2D::scale_xy(-1.0, 1.0),
                Transform2D::scale_xy(-1.0, 1.0).then(&Transform2D::rotate(a)),
                Transform2D::scale_xy(-1.0, 1.0).then(&Transform2D::rotate(2.0 * a)),
            ]
        }
        SymmetryGroup::P6 => {
            let a = std::f64::consts::PI / 3.0;
            (0..6).map(|i| Transform2D::rotate(i as f64 * a)).collect()
        }
        SymmetryGroup::P6M => {
            let a = std::f64::consts::PI / 3.0;
            let mut v: Vec<Transform2D> =
                (0..6).map(|i| Transform2D::rotate(i as f64 * a)).collect();
            for i in 0..6 {
                v.push(Transform2D::scale_xy(-1.0, 1.0).then(&Transform2D::rotate(i as f64 * a)));
            }
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_generates_grid() {
        let params = TileParams {
            symmetry: SymmetryGroup::P1,
            rows: 3,
            columns: 4,
            tile_width: 50.0,
            tile_height: 50.0,
            ..Default::default()
        };
        let placements = generate_tile_placements(&params);
        assert_eq!(placements.len(), 12); // 3*4*1 (P1 has 1 base)
    }

    #[test]
    fn p2_doubles_count() {
        let params = TileParams {
            symmetry: SymmetryGroup::P2,
            rows: 2,
            columns: 2,
            tile_width: 50.0,
            tile_height: 50.0,
            ..Default::default()
        };
        let placements = generate_tile_placements(&params);
        assert_eq!(placements.len(), 8); // 2*2*2 (P2 has 2 bases)
    }

    #[test]
    fn opacity_decreases_per_row() {
        let params = TileParams {
            symmetry: SymmetryGroup::P1,
            rows: 5,
            columns: 1,
            tile_width: 50.0,
            tile_height: 50.0,
            opacity_per_row: -0.2,
            ..Default::default()
        };
        let placements = generate_tile_placements(&params);
        assert_eq!(placements.len(), 5);
        assert!((placements[0].opacity - 1.0).abs() < 1e-10);
        assert!((placements[1].opacity - 0.8).abs() < 1e-10);
        assert!((placements[4].opacity - 0.2).abs() < 1e-10);
    }

    #[test]
    fn p6m_generates_12_per_cell() {
        let params = TileParams {
            symmetry: SymmetryGroup::P6M,
            rows: 1,
            columns: 1,
            tile_width: 50.0,
            tile_height: 50.0,
            ..Default::default()
        };
        let placements = generate_tile_placements(&params);
        assert_eq!(placements.len(), 12); // P6M: 6 rotations + 6 reflections
    }
}
