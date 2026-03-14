//! Native file dialogs (12.3).
//!
//! Thin wrappers around the `rfd` (Rust File Dialog) crate for opening,
//! saving, and exporting files with appropriate file-type filters.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// File filters
// ---------------------------------------------------------------------------

/// SVG file extensions.
const SVG_EXTENSIONS: &[&str] = &["svg", "svgz"];
/// PNG file extensions.
const PNG_EXTENSIONS: &[&str] = &["png"];
/// JPEG file extensions.
const JPEG_EXTENSIONS: &[&str] = &["jpg", "jpeg"];
/// PDF file extensions.
const PDF_EXTENSIONS: &[&str] = &["pdf"];
/// All supported image import extensions.
const ALL_SUPPORTED: &[&str] = &["svg", "svgz", "png", "jpg", "jpeg", "pdf"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Show an "Open File" dialog filtered for SVG (and other supported) files.
///
/// Returns `Some(path)` if the user selects a file, `None` if cancelled.
pub fn open_file_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open File")
        .add_filter("SVG files", SVG_EXTENSIONS)
        .add_filter("All supported", ALL_SUPPORTED)
        .add_filter("All files", &["*"])
        .pick_file()
}

/// Show a "Save File" dialog for SVG files.
///
/// `default_name` is used as the suggested file name (e.g. `"drawing.svg"`).
/// Returns `Some(path)` if the user confirms, `None` if cancelled.
pub fn save_file_dialog(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save As")
        .set_file_name(default_name)
        .add_filter("SVG files", SVG_EXTENSIONS)
        .add_filter("All files", &["*"])
        .save_file()
}

/// Show an "Export" dialog filtered for a specific output format.
///
/// `format` should be one of `"png"`, `"jpeg"`, `"pdf"`, or `"svg"`.
/// Returns `Some(path)` if the user confirms, `None` if cancelled.
pub fn export_file_dialog(format: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Export");

    match format.to_lowercase().as_str() {
        "png" => {
            dialog = dialog
                .add_filter("PNG images", PNG_EXTENSIONS)
                .set_file_name("export.png");
        }
        "jpeg" | "jpg" => {
            dialog = dialog
                .add_filter("JPEG images", JPEG_EXTENSIONS)
                .set_file_name("export.jpg");
        }
        "pdf" => {
            dialog = dialog
                .add_filter("PDF documents", PDF_EXTENSIONS)
                .set_file_name("export.pdf");
        }
        "svg" => {
            dialog = dialog
                .add_filter("SVG files", SVG_EXTENSIONS)
                .set_file_name("export.svg");
        }
        _ => {
            dialog = dialog
                .add_filter("All files", &["*"])
                .set_file_name(&format!("export.{}", format));
        }
    }

    dialog.save_file()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svg_extensions() {
        assert!(SVG_EXTENSIONS.contains(&"svg"));
        assert!(SVG_EXTENSIONS.contains(&"svgz"));
    }

    #[test]
    fn test_png_extensions() {
        assert!(PNG_EXTENSIONS.contains(&"png"));
    }

    #[test]
    fn test_jpeg_extensions() {
        assert!(JPEG_EXTENSIONS.contains(&"jpg"));
        assert!(JPEG_EXTENSIONS.contains(&"jpeg"));
    }

    #[test]
    fn test_pdf_extensions() {
        assert!(PDF_EXTENSIONS.contains(&"pdf"));
    }

    #[test]
    fn test_all_supported_includes_main_formats() {
        for ext in &["svg", "png", "jpg", "jpeg", "pdf"] {
            assert!(
                ALL_SUPPORTED.contains(ext),
                "ALL_SUPPORTED should contain '{}'",
                ext
            );
        }
    }

    // NOTE: We cannot test the actual dialogs in headless CI because they
    // require a windowing system.  The filter constants are the testable part.
}
