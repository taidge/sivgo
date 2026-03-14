//! Native drag-and-drop handling (12.4).
//!
//! Classifies dropped file paths into actionable operations based on their
//! file extension.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An action to take when a file is dropped onto the application window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragDropAction {
    /// Open the file as a new SVG document.
    OpenDocument(PathBuf),
    /// Import the file as a raster image into the current document.
    ImportImage(PathBuf),
    /// Import an external SVG fragment into the current document.
    ImportSvg(PathBuf),
}

/// Handles drag-and-drop events and classifies file paths.
#[derive(Debug, Default)]
pub struct DragDropHandler;

impl DragDropHandler {
    /// Create a new `DragDropHandler`.
    pub fn new() -> Self {
        Self
    }

    /// Classify a list of dropped file paths into appropriate actions.
    ///
    /// Rules:
    /// * `.svg` / `.svgz` -- If a single SVG is dropped it becomes
    ///   [`DragDropAction::OpenDocument`]; if dropped alongside other files it
    ///   becomes [`DragDropAction::ImportSvg`].
    /// * `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`, `.tiff`, `.tif` --
    ///   [`DragDropAction::ImportImage`].
    /// * Anything else is silently ignored.
    pub fn handle_drop(&self, paths: Vec<PathBuf>) -> Vec<DragDropAction> {
        let multiple = paths.len() > 1;
        let mut actions = Vec::new();

        for path in paths {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match ext.as_str() {
                "svg" | "svgz" => {
                    if multiple {
                        actions.push(DragDropAction::ImportSvg(path));
                    } else {
                        actions.push(DragDropAction::OpenDocument(path));
                    }
                }
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" => {
                    actions.push(DragDropAction::ImportImage(path));
                }
                _ => {
                    // Unsupported file type -- skip silently.
                }
            }
        }

        actions
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> DragDropHandler {
        DragDropHandler::new()
    }

    #[test]
    fn test_single_svg_opens_document() {
        let actions = handler().handle_drop(vec![PathBuf::from("drawing.svg")]);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            DragDropAction::OpenDocument(PathBuf::from("drawing.svg"))
        );
    }

    #[test]
    fn test_single_svgz_opens_document() {
        let actions = handler().handle_drop(vec![PathBuf::from("drawing.svgz")]);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            DragDropAction::OpenDocument(PathBuf::from("drawing.svgz"))
        );
    }

    #[test]
    fn test_multiple_svg_imports() {
        let actions = handler().handle_drop(vec![
            PathBuf::from("a.svg"),
            PathBuf::from("b.svg"),
        ]);
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            DragDropAction::ImportSvg(PathBuf::from("a.svg"))
        );
        assert_eq!(
            actions[1],
            DragDropAction::ImportSvg(PathBuf::from("b.svg"))
        );
    }

    #[test]
    fn test_image_files_import() {
        let actions = handler().handle_drop(vec![PathBuf::from("photo.png")]);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            DragDropAction::ImportImage(PathBuf::from("photo.png"))
        );
    }

    #[test]
    fn test_jpeg_variants() {
        let actions = handler().handle_drop(vec![
            PathBuf::from("a.jpg"),
            PathBuf::from("b.jpeg"),
        ]);
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], DragDropAction::ImportImage(_)));
        assert!(matches!(actions[1], DragDropAction::ImportImage(_)));
    }

    #[test]
    fn test_mixed_drop() {
        let actions = handler().handle_drop(vec![
            PathBuf::from("icon.svg"),
            PathBuf::from("photo.png"),
        ]);
        assert_eq!(actions.len(), 2);
        // SVG becomes ImportSvg when mixed with other files.
        assert_eq!(
            actions[0],
            DragDropAction::ImportSvg(PathBuf::from("icon.svg"))
        );
        assert_eq!(
            actions[1],
            DragDropAction::ImportImage(PathBuf::from("photo.png"))
        );
    }

    #[test]
    fn test_unsupported_extension_ignored() {
        let actions = handler().handle_drop(vec![PathBuf::from("notes.txt")]);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_no_extension_ignored() {
        let actions = handler().handle_drop(vec![PathBuf::from("README")]);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_empty_drop() {
        let actions = handler().handle_drop(vec![]);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_case_insensitive_extension() {
        let actions = handler().handle_drop(vec![PathBuf::from("drawing.SVG")]);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], DragDropAction::OpenDocument(_)));
    }
}
