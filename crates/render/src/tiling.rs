//! Tile-based rendering for large documents.
//!
//! [`TileGrid`] partitions the canvas into a regular grid of fixed-size tiles.
//! Each [`Tile`] holds its own pixel buffer and a *dirty* flag so the renderer
//! can skip tiles that have not changed.  When a region of the document is
//! modified, call [`TileGrid::invalidate_region`] to mark the overlapping
//! tiles, then only re-render those tiles on the next frame.

use sivgo_math::Rect;

/// Default tile dimension in pixels.
pub const DEFAULT_TILE_SIZE: u32 = 256;

/// A single tile in the grid.
#[derive(Debug, Clone)]
pub struct Tile {
    /// Column index of this tile in the grid (0-based).
    pub col: usize,
    /// Row index of this tile in the grid (0-based).
    pub row: usize,
    /// Whether this tile needs to be re-rendered.
    pub dirty: bool,
    /// Cached premultiplied RGBA pixel data for this tile.
    ///
    /// Empty until the tile has been rendered at least once.
    pub pixmap: Vec<u8>,
    /// Actual width of this tile in pixels (may be smaller than `tile_size`
    /// for the rightmost column).
    pub width: u32,
    /// Actual height of this tile in pixels (may be smaller than `tile_size`
    /// for the bottom row).
    pub height: u32,
}

impl Tile {
    /// Create a new dirty tile with an empty pixel buffer.
    fn new(col: usize, row: usize, width: u32, height: u32) -> Self {
        Self {
            col,
            row,
            dirty: true,
            pixmap: Vec::new(),
            width,
            height,
        }
    }

    /// Clear the cached pixmap and mark the tile dirty.
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.pixmap.clear();
    }
}

/// A grid of tiles covering the full canvas area.
///
/// Created via [`TileGrid::new`], which computes the number of columns and
/// rows needed to cover `total_width x total_height` using tiles of
/// `tile_size x tile_size` pixels.
#[derive(Debug, Clone)]
pub struct TileGrid {
    /// Nominal tile dimension (tiles on the right/bottom edge may be smaller).
    tile_size: u32,
    /// Total canvas width in pixels.
    total_width: u32,
    /// Total canvas height in pixels.
    total_height: u32,
    /// Number of tile columns.
    cols: usize,
    /// Number of tile rows.
    rows: usize,
    /// Flat storage of tiles in row-major order: `tiles[row * cols + col]`.
    tiles: Vec<Tile>,
}

impl TileGrid {
    /// Create a new tile grid covering `total_width x total_height` pixels
    /// with tiles of `tile_size x tile_size`.
    ///
    /// # Panics
    ///
    /// Panics if `tile_size` is zero.
    pub fn new(total_width: u32, total_height: u32, tile_size: u32) -> Self {
        assert!(tile_size > 0, "tile_size must be > 0");

        let cols = Self::div_ceil(total_width, tile_size);
        let rows = Self::div_ceil(total_height, tile_size);

        let mut tiles = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let tw = tile_size.min(total_width.saturating_sub(col as u32 * tile_size));
                let th = tile_size.min(total_height.saturating_sub(row as u32 * tile_size));
                tiles.push(Tile::new(col, row, tw, th));
            }
        }

        Self {
            tile_size,
            total_width,
            total_height,
            cols,
            rows,
            tiles,
        }
    }

    /// Integer ceiling division.
    fn div_ceil(a: u32, b: u32) -> usize {
        (a as usize).div_ceil(b as usize)
    }

    /// Number of tile columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of tile rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Tile dimension used by this grid.
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    /// Total canvas width.
    pub fn total_width(&self) -> u32 {
        self.total_width
    }

    /// Total canvas height.
    pub fn total_height(&self) -> u32 {
        self.total_height
    }

    /// Get an immutable reference to the tile at `(col, row)`.
    ///
    /// Returns `None` if the indices are out of range.
    pub fn get_tile(&self, col: usize, row: usize) -> Option<&Tile> {
        if col < self.cols && row < self.rows {
            Some(&self.tiles[row * self.cols + col])
        } else {
            None
        }
    }

    /// Get a mutable reference to the tile at `(col, row)`.
    ///
    /// Returns `None` if the indices are out of range.
    pub fn get_tile_mut(&mut self, col: usize, row: usize) -> Option<&mut Tile> {
        if col < self.cols && row < self.rows {
            Some(&mut self.tiles[row * self.cols + col])
        } else {
            None
        }
    }

    /// Return the pixel-space bounding rectangle of the tile at `(col, row)`.
    pub fn tile_rect(&self, col: usize, row: usize) -> Rect {
        let x0 = col as f64 * self.tile_size as f64;
        let y0 = row as f64 * self.tile_size as f64;
        let tw = self.get_tile(col, row).map(|t| t.width).unwrap_or(0);
        let th = self.get_tile(col, row).map(|t| t.height).unwrap_or(0);
        Rect::new(x0, y0, x0 + tw as f64, y0 + th as f64)
    }

    /// Mark all tiles that overlap the given rectangle as dirty.
    pub fn invalidate_region(&mut self, rect: Rect) {
        for (col, row) in self.tile_indices_in_region(rect) {
            if let Some(tile) = self.get_tile_mut(col, row) {
                tile.invalidate();
            }
        }
    }

    /// Mark every tile in the grid as dirty.
    pub fn invalidate_all(&mut self) {
        for tile in &mut self.tiles {
            tile.invalidate();
        }
    }

    /// Return the `(col, row)` indices of every tile that overlaps `rect`.
    pub fn tiles_in_region(&self, rect: Rect) -> Vec<(usize, usize)> {
        self.tile_indices_in_region(rect)
    }

    /// Return all tiles that are currently dirty.
    pub fn dirty_tiles(&self) -> Vec<(usize, usize)> {
        self.tiles
            .iter()
            .filter(|t| t.dirty)
            .map(|t| (t.col, t.row))
            .collect()
    }

    /// Internal helper: compute tile index ranges that overlap `rect`.
    fn tile_indices_in_region(&self, rect: Rect) -> Vec<(usize, usize)> {
        if rect.is_empty() {
            return Vec::new();
        }

        let col_start = (rect.x0.max(0.0) as u32 / self.tile_size) as usize;
        let row_start = (rect.y0.max(0.0) as u32 / self.tile_size) as usize;

        // Exclusive upper bounds (ceil), clamped to grid dimensions.
        let col_end = Self::div_ceil(
            (rect.x1.max(0.0).min(self.total_width as f64)) as u32,
            self.tile_size,
        )
        .min(self.cols);
        let row_end = Self::div_ceil(
            (rect.y1.max(0.0).min(self.total_height as f64)) as u32,
            self.tile_size,
        )
        .min(self.rows);

        let mut result = Vec::with_capacity((col_end - col_start) * (row_end - row_start));
        for row in row_start..row_end {
            for col in col_start..col_end {
                result.push((col, row));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_dimensions() {
        let grid = TileGrid::new(512, 512, 256);
        assert_eq!(grid.cols(), 2);
        assert_eq!(grid.rows(), 2);
    }

    #[test]
    fn grid_dimensions_non_exact() {
        let grid = TileGrid::new(500, 300, 256);
        assert_eq!(grid.cols(), 2);
        assert_eq!(grid.rows(), 2);
    }

    #[test]
    fn grid_dimensions_smaller_than_tile() {
        let grid = TileGrid::new(100, 50, 256);
        assert_eq!(grid.cols(), 1);
        assert_eq!(grid.rows(), 1);
        let tile = grid.get_tile(0, 0).unwrap();
        assert_eq!(tile.width, 100);
        assert_eq!(tile.height, 50);
    }

    #[test]
    fn edge_tile_sizes() {
        let grid = TileGrid::new(500, 300, 256);
        // Top-left tile: full size
        let tl = grid.get_tile(0, 0).unwrap();
        assert_eq!(tl.width, 256);
        assert_eq!(tl.height, 256);
        // Top-right tile: partial width
        let tr = grid.get_tile(1, 0).unwrap();
        assert_eq!(tr.width, 244); // 500 - 256
        assert_eq!(tr.height, 256);
        // Bottom-left tile: partial height
        let bl = grid.get_tile(0, 1).unwrap();
        assert_eq!(bl.width, 256);
        assert_eq!(bl.height, 44); // 300 - 256
                                   // Bottom-right tile: partial both
        let br = grid.get_tile(1, 1).unwrap();
        assert_eq!(br.width, 244);
        assert_eq!(br.height, 44);
    }

    #[test]
    fn all_tiles_start_dirty() {
        let grid = TileGrid::new(256, 256, 256);
        let tile = grid.get_tile(0, 0).unwrap();
        assert!(tile.dirty);
    }

    #[test]
    fn get_tile_out_of_bounds() {
        let grid = TileGrid::new(256, 256, 256);
        assert!(grid.get_tile(1, 0).is_none());
        assert!(grid.get_tile(0, 1).is_none());
    }

    #[test]
    fn invalidate_region_marks_correct_tiles() {
        let mut grid = TileGrid::new(1024, 1024, 256);
        // Clear all dirty flags first
        for tile in &mut grid.tiles {
            tile.dirty = false;
        }
        // Invalidate a region that spans tiles (0,0) and (1,0) and (0,1) and (1,1)
        grid.invalidate_region(Rect::new(200.0, 200.0, 300.0, 300.0));
        assert!(grid.get_tile(0, 0).unwrap().dirty);
        assert!(grid.get_tile(1, 0).unwrap().dirty);
        assert!(grid.get_tile(0, 1).unwrap().dirty);
        assert!(grid.get_tile(1, 1).unwrap().dirty);
        // Tile (2,0) should be untouched
        assert!(!grid.get_tile(2, 0).unwrap().dirty);
    }

    #[test]
    fn invalidate_region_single_tile() {
        let mut grid = TileGrid::new(1024, 1024, 256);
        for tile in &mut grid.tiles {
            tile.dirty = false;
        }
        grid.invalidate_region(Rect::new(10.0, 10.0, 50.0, 50.0));
        assert!(grid.get_tile(0, 0).unwrap().dirty);
        assert!(!grid.get_tile(1, 0).unwrap().dirty);
        assert!(!grid.get_tile(0, 1).unwrap().dirty);
    }

    #[test]
    fn tiles_in_region_returns_correct_indices() {
        let grid = TileGrid::new(1024, 1024, 256);
        let indices = grid.tiles_in_region(Rect::new(200.0, 200.0, 300.0, 300.0));
        assert_eq!(indices.len(), 4);
        assert!(indices.contains(&(0, 0)));
        assert!(indices.contains(&(1, 0)));
        assert!(indices.contains(&(0, 1)));
        assert!(indices.contains(&(1, 1)));
    }

    #[test]
    fn tiles_in_region_empty_rect() {
        let grid = TileGrid::new(1024, 1024, 256);
        let indices = grid.tiles_in_region(Rect::new(10.0, 10.0, 10.0, 10.0));
        assert!(indices.is_empty());
    }

    #[test]
    fn tiles_in_region_outside_canvas() {
        let grid = TileGrid::new(512, 512, 256);
        let indices = grid.tiles_in_region(Rect::new(600.0, 600.0, 700.0, 700.0));
        assert!(indices.is_empty());
    }

    #[test]
    fn tile_rect_values() {
        let grid = TileGrid::new(512, 512, 256);
        let r = grid.tile_rect(1, 0);
        assert_eq!(r, Rect::new(256.0, 0.0, 512.0, 256.0));
    }

    #[test]
    fn dirty_tiles_all_initially() {
        let grid = TileGrid::new(512, 512, 256);
        assert_eq!(grid.dirty_tiles().len(), 4);
    }

    #[test]
    fn invalidate_all() {
        let mut grid = TileGrid::new(512, 512, 256);
        for tile in &mut grid.tiles {
            tile.dirty = false;
        }
        assert!(grid.dirty_tiles().is_empty());
        grid.invalidate_all();
        assert_eq!(grid.dirty_tiles().len(), 4);
    }

    #[test]
    #[should_panic(expected = "tile_size must be > 0")]
    fn zero_tile_size_panics() {
        TileGrid::new(100, 100, 0);
    }
}
