//! Export panel data model (11.4.18).
//!
//! Settings for exporting the document or a portion of it to various
//! raster and vector formats.

/// Export file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Svg,
    Pdf,
    WebP,
}

impl ExportFormat {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ExportFormat::Png => "PNG",
            ExportFormat::Jpeg => "JPEG",
            ExportFormat::Svg => "SVG",
            ExportFormat::Pdf => "PDF",
            ExportFormat::WebP => "WebP",
        }
    }

    /// File extension (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Jpeg => "jpg",
            ExportFormat::Svg => "svg",
            ExportFormat::Pdf => "pdf",
            ExportFormat::WebP => "webp",
        }
    }

    /// All formats in display order.
    pub fn all() -> &'static [ExportFormat] {
        &[
            ExportFormat::Png,
            ExportFormat::Jpeg,
            ExportFormat::Svg,
            ExportFormat::Pdf,
            ExportFormat::WebP,
        ]
    }

    /// Return `true` for raster formats.
    pub fn is_raster(&self) -> bool {
        matches!(
            self,
            ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::WebP
        )
    }
}

/// Which area of the document to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportArea {
    /// The full page.
    Page,
    /// Bounding box of all drawing content.
    Drawing,
    /// Bounding box of the current selection.
    Selection,
    /// A user-defined rectangle.
    Custom,
}

impl ExportArea {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ExportArea::Page => "Page",
            ExportArea::Drawing => "Drawing",
            ExportArea::Selection => "Selection",
            ExportArea::Custom => "Custom",
        }
    }

    /// All areas in display order.
    pub fn all() -> &'static [ExportArea] {
        &[
            ExportArea::Page,
            ExportArea::Drawing,
            ExportArea::Selection,
            ExportArea::Custom,
        ]
    }
}

/// A rectangle defined by position and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Default for ExportRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        }
    }
}

/// Complete state for the Export panel.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPanelState {
    /// Target file format.
    pub format: ExportFormat,
    /// Area to export.
    pub area: ExportArea,
    /// Custom export rectangle (used when `area == Custom`).
    pub custom_rect: ExportRect,
    /// Resolution in dots per inch (relevant for raster formats).
    pub dpi: f64,
    /// Output filename.
    pub filename: String,
    /// Whether to export in batch mode (e.g. one file per selected object).
    pub batch_mode: bool,
}

impl Default for ExportPanelState {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            area: ExportArea::Page,
            custom_rect: ExportRect::default(),
            dpi: 96.0,
            filename: String::new(),
            batch_mode: false,
        }
    }
}

impl ExportPanelState {
    /// Set the export format.
    pub fn set_format(&mut self, format: ExportFormat) {
        self.format = format;
    }

    /// Set the export area.
    pub fn set_area(&mut self, area: ExportArea) {
        self.area = area;
    }

    /// Trigger an export.
    ///
    /// This method validates the current settings and returns `Ok(())` if the
    /// export request is well-formed.  The actual file writing is done at a
    /// higher layer.
    pub fn export(&self) -> Result<(), String> {
        if self.filename.is_empty() {
            return Err("Filename must not be empty".to_string());
        }
        if self.dpi <= 0.0 {
            return Err("DPI must be positive".to_string());
        }
        if self.area == ExportArea::Custom
            && (self.custom_rect.w <= 0.0 || self.custom_rect.h <= 0.0)
        {
            return Err("Custom rectangle must have positive dimensions".to_string());
        }
        Ok(())
    }

    /// Suggest a default filename based on the format.
    pub fn suggest_filename(&self, base_name: &str) -> String {
        format!("{}.{}", base_name, self.format.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = ExportPanelState::default();
        assert_eq!(state.format, ExportFormat::Png);
        assert_eq!(state.area, ExportArea::Page);
        assert!((state.dpi - 96.0).abs() < f64::EPSILON);
        assert!(state.filename.is_empty());
        assert!(!state.batch_mode);
    }

    #[test]
    fn format_extension() {
        assert_eq!(ExportFormat::Png.extension(), "png");
        assert_eq!(ExportFormat::Jpeg.extension(), "jpg");
        assert_eq!(ExportFormat::Svg.extension(), "svg");
    }

    #[test]
    fn format_is_raster() {
        assert!(ExportFormat::Png.is_raster());
        assert!(ExportFormat::Jpeg.is_raster());
        assert!(ExportFormat::WebP.is_raster());
        assert!(!ExportFormat::Svg.is_raster());
        assert!(!ExportFormat::Pdf.is_raster());
    }

    #[test]
    fn export_empty_filename() {
        let state = ExportPanelState::default();
        assert!(state.export().is_err());
    }

    #[test]
    fn export_valid() {
        let mut state = ExportPanelState::default();
        state.filename = "output.png".to_string();
        assert!(state.export().is_ok());
    }

    #[test]
    fn export_invalid_dpi() {
        let mut state = ExportPanelState::default();
        state.filename = "output.png".to_string();
        state.dpi = 0.0;
        assert!(state.export().is_err());
    }

    #[test]
    fn export_invalid_custom_rect() {
        let mut state = ExportPanelState::default();
        state.filename = "output.png".to_string();
        state.area = ExportArea::Custom;
        state.custom_rect.w = 0.0;
        assert!(state.export().is_err());
    }

    #[test]
    fn suggest_filename() {
        let mut state = ExportPanelState::default();
        state.format = ExportFormat::Pdf;
        assert_eq!(state.suggest_filename("drawing"), "drawing.pdf");
    }

    #[test]
    fn area_labels() {
        assert_eq!(ExportArea::Page.label(), "Page");
        assert_eq!(ExportArea::all().len(), 4);
    }
}
