//! Web File API abstraction for opening and saving files.
//!
//! Provides a platform-agnostic interface for file open/save operations
//! that will map to the browser File API in the WASM build.  The methods
//! are stubs that return sensible defaults or errors until wired to real
//! browser bindings.

/// Supported MIME types for file operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebMimeType {
    Svg,
    Png,
    Jpeg,
}

impl WebMimeType {
    /// Return the MIME type string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    /// Parse a MIME type string into a `WebMimeType`.
    pub fn from_str(mime: &str) -> Option<Self> {
        match mime {
            "image/svg+xml" => Some(Self::Svg),
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            _ => None,
        }
    }

    /// Return a typical file extension for this MIME type.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

/// Error type for web file operations.
#[derive(Debug, Clone)]
pub enum WebFileError {
    /// The user cancelled the file dialog.
    Cancelled,
    /// The requested MIME type is not supported.
    UnsupportedMimeType(String),
    /// A generic I/O or browser API error.
    IoError(String),
}

impl std::fmt::Display for WebFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "File operation cancelled"),
            Self::UnsupportedMimeType(t) => write!(f, "Unsupported MIME type: {t}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

/// Stub for the browser File API.
///
/// In the WASM build this will use `web_sys` / `js_sys` to interact with
/// the browser.  For now the methods document the expected interface and
/// return stub results.
#[derive(Debug, Default)]
pub struct WebFileApi;

impl WebFileApi {
    /// Create a new `WebFileApi` instance.
    pub fn new() -> Self {
        Self
    }

    /// Open a file using the browser file picker.
    ///
    /// Returns `(filename, file_contents)` on success.
    ///
    /// # Stub behaviour
    ///
    /// Always returns `Err(Cancelled)` because there is no browser context.
    pub fn open_file(&self) -> Result<(String, Vec<u8>), WebFileError> {
        // Stub -- in WASM this would invoke <input type="file">
        Err(WebFileError::Cancelled)
    }

    /// Save content to a file using the browser "Save As" dialog.
    ///
    /// # Parameters
    ///
    /// * `name` -- suggested file name.
    /// * `content` -- raw bytes to save.
    /// * `mime` -- MIME type string (must be one of the supported types).
    pub fn save_file(&self, name: &str, content: &[u8], mime: &str) -> Result<(), WebFileError> {
        // Validate the MIME type.
        if WebMimeType::from_str(mime).is_none() {
            return Err(WebFileError::UnsupportedMimeType(mime.to_string()));
        }
        // Validate inputs.
        if name.is_empty() {
            return Err(WebFileError::IoError("File name must not be empty".into()));
        }
        if content.is_empty() {
            return Err(WebFileError::IoError("Content must not be empty".into()));
        }
        // Stub -- in WASM this would create a Blob and trigger a download.
        Ok(())
    }

    /// Trigger a browser download of a binary blob.
    ///
    /// Unlike `save_file`, this does not open a "Save As" dialog -- the
    /// browser immediately starts downloading the file with the given name.
    pub fn download_blob(&self, name: &str, content: &[u8], mime: &str) {
        // Stub -- in WASM this would create an <a> element with a blob URL,
        // click it, then revoke the URL.
        let _ = (name, content, mime);
    }

    /// Return the list of supported MIME types.
    pub fn supported_types(&self) -> Vec<WebMimeType> {
        vec![WebMimeType::Svg, WebMimeType::Png, WebMimeType::Jpeg]
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_roundtrip() {
        for mt in [WebMimeType::Svg, WebMimeType::Png, WebMimeType::Jpeg] {
            let s = mt.as_str();
            let parsed = WebMimeType::from_str(s);
            assert_eq!(parsed, Some(mt));
        }
    }

    #[test]
    fn mime_type_extensions() {
        assert_eq!(WebMimeType::Svg.extension(), "svg");
        assert_eq!(WebMimeType::Png.extension(), "png");
        assert_eq!(WebMimeType::Jpeg.extension(), "jpg");
    }

    #[test]
    fn unknown_mime_returns_none() {
        assert!(WebMimeType::from_str("application/pdf").is_none());
    }

    #[test]
    fn jpeg_alias() {
        assert_eq!(WebMimeType::from_str("image/jpg"), Some(WebMimeType::Jpeg));
    }

    #[test]
    fn open_file_stub_returns_cancelled() {
        let api = WebFileApi::new();
        let result = api.open_file();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WebFileError::Cancelled));
    }

    #[test]
    fn save_file_rejects_unsupported_mime() {
        let api = WebFileApi::new();
        let result = api.save_file("test.pdf", b"data", "application/pdf");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WebFileError::UnsupportedMimeType(_)
        ));
    }

    #[test]
    fn save_file_rejects_empty_name() {
        let api = WebFileApi::new();
        let result = api.save_file("", b"data", "image/svg+xml");
        assert!(result.is_err());
    }

    #[test]
    fn save_file_rejects_empty_content() {
        let api = WebFileApi::new();
        let result = api.save_file("test.svg", b"", "image/svg+xml");
        assert!(result.is_err());
    }

    #[test]
    fn save_file_accepts_valid_inputs() {
        let api = WebFileApi::new();
        let result = api.save_file("drawing.svg", b"<svg></svg>", "image/svg+xml");
        assert!(result.is_ok());
    }

    #[test]
    fn download_blob_does_not_panic() {
        let api = WebFileApi::new();
        api.download_blob("export.png", &[0x89, 0x50, 0x4E, 0x47], "image/png");
    }

    #[test]
    fn supported_types_includes_all() {
        let api = WebFileApi::new();
        let types = api.supported_types();
        assert_eq!(types.len(), 3);
    }
}
