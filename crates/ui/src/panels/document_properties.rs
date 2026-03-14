//! Document Properties panel data model (11.4.8).
//!
//! Page dimensions, units, orientation, background, metadata, grid, and snap
//! settings for the current document.

/// Measurement unit for page dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageUnit {
    Px,
    Mm,
    In,
    Pt,
}

impl PageUnit {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PageUnit::Px => "px",
            PageUnit::Mm => "mm",
            PageUnit::In => "in",
            PageUnit::Pt => "pt",
        }
    }

    /// All units in display order.
    pub fn all() -> &'static [PageUnit] {
        &[PageUnit::Px, PageUnit::Mm, PageUnit::In, PageUnit::Pt]
    }
}

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

impl PageOrientation {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PageOrientation::Portrait => "Portrait",
            PageOrientation::Landscape => "Landscape",
        }
    }
}

/// Named preset page sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagePreset {
    A4,
    A3,
    Letter,
    Legal,
    Custom,
}

impl PagePreset {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PagePreset::A4 => "A4 (210 x 297 mm)",
            PagePreset::A3 => "A3 (297 x 420 mm)",
            PagePreset::Letter => "Letter (8.5 x 11 in)",
            PagePreset::Legal => "Legal (8.5 x 14 in)",
            PagePreset::Custom => "Custom",
        }
    }

    /// All presets in display order.
    pub fn all() -> &'static [PagePreset] {
        &[
            PagePreset::A4,
            PagePreset::A3,
            PagePreset::Letter,
            PagePreset::Legal,
            PagePreset::Custom,
        ]
    }

    /// Return the (width, height) in pixels at 96 DPI for portrait orientation.
    /// Returns `None` for `Custom`.
    pub fn dimensions_px(&self) -> Option<(f64, f64)> {
        match self {
            PagePreset::A4 => Some((793.7, 1122.5)),
            PagePreset::A3 => Some((1122.5, 1587.4)),
            PagePreset::Letter => Some((816.0, 1056.0)),
            PagePreset::Legal => Some((816.0, 1344.0)),
            PagePreset::Custom => None,
        }
    }
}

/// Document metadata fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentMetadata {
    /// Document title.
    pub title: String,
    /// Author name.
    pub author: String,
    /// Short description.
    pub description: String,
    /// Comma-separated keywords.
    pub keywords: String,
    /// License identifier or URL.
    pub license: String,
}

/// Grid settings for the document.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSettings {
    /// Whether the grid is enabled / visible.
    pub enabled: bool,
    /// Horizontal grid spacing in user units.
    pub spacing_x: f64,
    /// Vertical grid spacing in user units.
    pub spacing_y: f64,
    /// Grid origin X.
    pub origin_x: f64,
    /// Grid origin Y.
    pub origin_y: f64,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing_x: 10.0,
            spacing_y: 10.0,
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }
}

/// Snap settings for the document.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapSettings {
    /// Global snap enable.
    pub enabled: bool,
    /// Snap to grid.
    pub snap_to_grid: bool,
    /// Snap to guides.
    pub snap_to_guides: bool,
    /// Snap to object nodes.
    pub snap_to_nodes: bool,
    /// Snap to object edges / bounding boxes.
    pub snap_to_edges: bool,
    /// Snap distance in pixels.
    pub snap_distance: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            snap_to_grid: true,
            snap_to_guides: true,
            snap_to_nodes: true,
            snap_to_edges: true,
            snap_distance: 10.0,
        }
    }
}

/// Complete state for the Document Properties panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentPropertiesState {
    /// Page width in current units.
    pub page_width: f64,
    /// Page height in current units.
    pub page_height: f64,
    /// Measurement unit.
    pub page_unit: PageUnit,
    /// Background colour as RGBA `[0.0, 1.0]`.
    pub background_color: [f64; 4],
    /// Current page orientation.
    pub page_orientation: PageOrientation,
    /// Document metadata.
    pub metadata: DocumentMetadata,
    /// Grid settings.
    pub grid: GridSettings,
    /// Snap settings.
    pub snap: SnapSettings,
}

impl Default for DocumentPropertiesState {
    fn default() -> Self {
        // Default to A4 portrait at 96 DPI.
        let (w, h) = PagePreset::A4.dimensions_px().unwrap();
        Self {
            page_width: w,
            page_height: h,
            page_unit: PageUnit::Px,
            background_color: [1.0, 1.0, 1.0, 1.0],
            page_orientation: PageOrientation::Portrait,
            metadata: DocumentMetadata::default(),
            grid: GridSettings::default(),
            snap: SnapSettings::default(),
        }
    }
}

impl DocumentPropertiesState {
    /// Apply a named preset page size, keeping the current orientation.
    pub fn set_page_size(&mut self, preset: PagePreset) {
        if let Some((w, h)) = preset.dimensions_px() {
            match self.page_orientation {
                PageOrientation::Portrait => {
                    self.page_width = w;
                    self.page_height = h;
                }
                PageOrientation::Landscape => {
                    self.page_width = h;
                    self.page_height = w;
                }
            }
        }
    }

    /// Set an explicit custom page size.
    pub fn set_custom_size(&mut self, width: f64, height: f64) {
        self.page_width = width;
        self.page_height = height;
    }

    /// Toggle between portrait and landscape, swapping width and height.
    pub fn toggle_orientation(&mut self) {
        std::mem::swap(&mut self.page_width, &mut self.page_height);
        self.page_orientation = match self.page_orientation {
            PageOrientation::Portrait => PageOrientation::Landscape,
            PageOrientation::Landscape => PageOrientation::Portrait,
        };
    }

    /// Set the background colour from RGBA components in `[0.0, 1.0]`.
    pub fn set_background(&mut self, r: f64, g: f64, b: f64, a: f64) {
        self.background_color = [r, g, b, a];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_a4_portrait() {
        let state = DocumentPropertiesState::default();
        assert_eq!(state.page_orientation, PageOrientation::Portrait);
        assert!((state.page_width - 793.7).abs() < 0.1);
        assert!((state.page_height - 1122.5).abs() < 0.1);
    }

    #[test]
    fn toggle_orientation_swaps_dimensions() {
        let mut state = DocumentPropertiesState::default();
        let w = state.page_width;
        let h = state.page_height;
        state.toggle_orientation();
        assert_eq!(state.page_orientation, PageOrientation::Landscape);
        assert!((state.page_width - h).abs() < f64::EPSILON);
        assert!((state.page_height - w).abs() < f64::EPSILON);
    }

    #[test]
    fn set_page_size_letter() {
        let mut state = DocumentPropertiesState::default();
        state.set_page_size(PagePreset::Letter);
        assert!((state.page_width - 816.0).abs() < 0.1);
        assert!((state.page_height - 1056.0).abs() < 0.1);
    }

    #[test]
    fn set_page_size_landscape() {
        let mut state = DocumentPropertiesState::default();
        state.toggle_orientation();
        state.set_page_size(PagePreset::Letter);
        // In landscape, width > height.
        assert!(state.page_width > state.page_height);
    }

    #[test]
    fn set_custom_size() {
        let mut state = DocumentPropertiesState::default();
        state.set_custom_size(500.0, 700.0);
        assert!((state.page_width - 500.0).abs() < f64::EPSILON);
        assert!((state.page_height - 700.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_background_color() {
        let mut state = DocumentPropertiesState::default();
        state.set_background(0.1, 0.2, 0.3, 0.4);
        assert!((state.background_color[0] - 0.1).abs() < f64::EPSILON);
        assert!((state.background_color[3] - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn preset_labels() {
        assert_eq!(PagePreset::A4.label(), "A4 (210 x 297 mm)");
        assert_eq!(PagePreset::all().len(), 5);
    }

    #[test]
    fn grid_defaults() {
        let grid = GridSettings::default();
        assert!(!grid.enabled);
        assert!((grid.spacing_x - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn snap_defaults() {
        let snap = SnapSettings::default();
        assert!(snap.enabled);
        assert!(snap.snap_to_grid);
    }
}
