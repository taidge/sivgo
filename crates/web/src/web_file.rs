//! Web File API abstraction for opening, saving, and downloading files.
//!
//! Native tests use queued file-picker results and recorded save/download
//! operations. WASM callers can map the same operations to browser File API,
//! Blob, and object URL calls.

use std::cell::RefCell;
use std::collections::VecDeque;

/// Supported MIME types for file operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebMimeType {
    Svg,
    Png,
    Jpeg,
}

impl WebMimeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    pub fn from_str(mime: &str) -> Option<Self> {
        match mime {
            "image/svg+xml" => Some(Self::Svg),
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebFileError {
    Cancelled,
    UnsupportedMimeType(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFile {
    pub name: String,
    pub content: Vec<u8>,
    pub mime: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserFileOperation {
    OpenPicker {
        accept: Vec<String>,
    },
    SavePicker {
        name: String,
        mime: String,
        bytes: usize,
    },
    DownloadBlob {
        name: String,
        mime: String,
        bytes: usize,
    },
}

#[derive(Debug, Default)]
pub struct WebFileApi {
    queued_open_files: RefCell<VecDeque<WebFile>>,
    saved_files: RefCell<Vec<WebFile>>,
    downloads: RefCell<Vec<WebFile>>,
    browser_ops: RefCell<Vec<BrowserFileOperation>>,
}

impl WebFileApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_open_file(
        &self,
        name: &str,
        content: Vec<u8>,
        mime: &str,
    ) -> Result<(), WebFileError> {
        validate_file(name, &content, mime)?;
        self.queued_open_files.borrow_mut().push_back(WebFile {
            name: name.to_string(),
            content,
            mime: mime.to_string(),
        });
        Ok(())
    }

    pub fn open_file(&self) -> Result<(String, Vec<u8>), WebFileError> {
        self.browser_ops
            .borrow_mut()
            .push(BrowserFileOperation::OpenPicker {
                accept: self
                    .supported_types()
                    .iter()
                    .map(|mime| mime.as_str().to_string())
                    .collect(),
            });
        let Some(file) = self.queued_open_files.borrow_mut().pop_front() else {
            return Err(WebFileError::Cancelled);
        };
        Ok((file.name, file.content))
    }

    pub fn save_file(&self, name: &str, content: &[u8], mime: &str) -> Result<(), WebFileError> {
        validate_file(name, content, mime)?;
        self.saved_files.borrow_mut().push(WebFile {
            name: name.to_string(),
            content: content.to_vec(),
            mime: mime.to_string(),
        });
        self.browser_ops
            .borrow_mut()
            .push(BrowserFileOperation::SavePicker {
                name: name.to_string(),
                mime: mime.to_string(),
                bytes: content.len(),
            });
        Ok(())
    }

    pub fn download_blob(
        &self,
        name: &str,
        content: &[u8],
        mime: &str,
    ) -> Result<(), WebFileError> {
        validate_file(name, content, mime)?;
        self.downloads.borrow_mut().push(WebFile {
            name: name.to_string(),
            content: content.to_vec(),
            mime: mime.to_string(),
        });
        self.browser_ops
            .borrow_mut()
            .push(BrowserFileOperation::DownloadBlob {
                name: name.to_string(),
                mime: mime.to_string(),
                bytes: content.len(),
            });
        Ok(())
    }

    pub fn supported_types(&self) -> Vec<WebMimeType> {
        vec![WebMimeType::Svg, WebMimeType::Png, WebMimeType::Jpeg]
    }

    pub fn saved_files(&self) -> Vec<WebFile> {
        self.saved_files.borrow().clone()
    }

    pub fn downloads(&self) -> Vec<WebFile> {
        self.downloads.borrow().clone()
    }

    pub fn browser_operations(&self) -> Vec<BrowserFileOperation> {
        self.browser_ops.borrow().clone()
    }
}

fn validate_file(name: &str, content: &[u8], mime: &str) -> Result<(), WebFileError> {
    if WebMimeType::from_str(mime).is_none() {
        return Err(WebFileError::UnsupportedMimeType(mime.to_string()));
    }
    if name.trim().is_empty() {
        return Err(WebFileError::IoError("File name must not be empty".into()));
    }
    if content.is_empty() {
        return Err(WebFileError::IoError("Content must not be empty".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_roundtrip() {
        for mt in [WebMimeType::Svg, WebMimeType::Png, WebMimeType::Jpeg] {
            assert_eq!(WebMimeType::from_str(mt.as_str()), Some(mt));
        }
        assert_eq!(WebMimeType::from_str("image/jpg"), Some(WebMimeType::Jpeg));
        assert!(WebMimeType::from_str("application/pdf").is_none());
    }

    #[test]
    fn open_file_uses_queued_picker_results() {
        let api = WebFileApi::new();
        assert!(matches!(api.open_file(), Err(WebFileError::Cancelled)));
        api.queue_open_file("drawing.svg", b"<svg/>".to_vec(), "image/svg+xml")
            .unwrap();
        let (name, content) = api.open_file().unwrap();
        assert_eq!(name, "drawing.svg");
        assert_eq!(content, b"<svg/>");
        assert!(matches!(
            api.browser_operations()[0],
            BrowserFileOperation::OpenPicker { .. }
        ));
    }

    #[test]
    fn save_file_records_valid_file_and_rejects_invalid_inputs() {
        let api = WebFileApi::new();
        assert!(api
            .save_file("test.pdf", b"data", "application/pdf")
            .is_err());
        assert!(api.save_file("", b"data", "image/svg+xml").is_err());
        assert!(api.save_file("test.svg", b"", "image/svg+xml").is_err());

        api.save_file("drawing.svg", b"<svg></svg>", "image/svg+xml")
            .unwrap();
        let saved = api.saved_files();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "drawing.svg");
    }

    #[test]
    fn download_blob_records_download_operation() {
        let api = WebFileApi::new();
        api.download_blob("export.png", &[0x89, 0x50, 0x4e, 0x47], "image/png")
            .unwrap();
        assert_eq!(api.downloads().len(), 1);
        assert!(matches!(
            api.browser_operations().last(),
            Some(BrowserFileOperation::DownloadBlob { name, bytes: 4, .. }) if name == "export.png"
        ));
    }

    #[test]
    fn supported_types_includes_all() {
        let api = WebFileApi::new();
        assert_eq!(api.supported_types().len(), 3);
        assert_eq!(WebMimeType::Svg.extension(), "svg");
    }
}
