use crate::actions::PluginAction;
use crate::api::{Plugin, PluginContext};
use crate::manifest::{Permission, PluginManifest};
use sivgo_dom::types::{LengthUnit, SvgLength};
use sivgo_dom::{RectAttrs, SvgElement};

/// Plugin that generates a grid of rectangles.
///
/// Useful for creating tiled backgrounds, graph-paper overlays, or batch
/// geometry for testing.
#[derive(Debug)]
pub struct GridGeneratorPlugin {
    manifest: PluginManifest,
    rows: u32,
    cols: u32,
    cell_size: f64,
    gap: f64,
}

impl GridGeneratorPlugin {
    /// Create a grid generator with the given dimensions.
    ///
    /// * `rows` / `cols` -- number of cells in each direction.
    /// * `cell_size` -- width and height of each cell (square).
    /// * `gap` -- spacing between cells.
    pub fn new(rows: u32, cols: u32, cell_size: f64, gap: f64) -> Self {
        Self {
            manifest: PluginManifest {
                id: "builtin.grid_gen".to_string(),
                name: "Grid Generator".to_string(),
                version: "1.0.0".to_string(),
                author: "Sivgo Team".to_string(),
                description: format!(
                    "Generates a {}x{} grid of rectangles ({} px cells, {} px gap)",
                    rows, cols, cell_size, gap,
                ),
                permissions: vec![Permission::ModifyDocument],
            },
            rows,
            cols,
            cell_size,
            gap,
        }
    }
}

impl Plugin for GridGeneratorPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn on_activate(&mut self, ctx: &PluginContext) -> Vec<PluginAction> {
        let root = match ctx.document.root() {
            Some(id) => id,
            None => {
                return vec![PluginAction::Log("GridGenerator: no root node".to_string())];
            }
        };

        let mut actions: Vec<PluginAction> =
            Vec::with_capacity((self.rows * self.cols) as usize + 1);

        for row in 0..self.rows {
            for col in 0..self.cols {
                let x = col as f64 * (self.cell_size + self.gap);
                let y = row as f64 * (self.cell_size + self.gap);
                actions.push(PluginAction::AddNode {
                    parent_id: root,
                    element: SvgElement::Rect(RectAttrs {
                        x: SvgLength::new(x, LengthUnit::None),
                        y: SvgLength::new(y, LengthUnit::None),
                        width: SvgLength::new(self.cell_size, LengthUnit::None),
                        height: SvgLength::new(self.cell_size, LengthUnit::None),
                        rx: SvgLength::zero(),
                        ry: SvgLength::zero(),
                    }),
                });
            }
        }

        actions.push(PluginAction::ShowMessage(format!(
            "Generated {}x{} grid ({} rects)",
            self.rows,
            self.cols,
            self.rows * self.cols,
        )));

        actions
    }
}
