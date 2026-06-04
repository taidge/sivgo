//! System clipboard integration for Sivgo desktop (12.5).
//!
//! Uses the `arboard` crate for cross-platform clipboard access.  If the
//! clipboard cannot be initialised (e.g. headless CI), every operation returns
//! a human-readable error instead of panicking.
//!
//! In addition to the struct-based [`ClipboardManager`], this module provides
//! free-standing functions ([`copy_svg`], [`paste_svg`], [`copy_text`],
//! [`paste_text`], [`copy_image`], [`paste_image`], [`paste_preferred`]) that
//! use a thread-local in-memory store, suitable for testing and environments
//! where the system clipboard is unavailable.

use arboard::{Clipboard, Error as ClipboardError};
use std::cell::RefCell;

/// Raw RGBA bitmap payload: pixels, width, height.
pub type ClipboardBitmap = (Vec<u8>, u32, u32);

/// Clipboard payload kinds in Sivgo paste precedence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardPayloadKind {
    /// Serialized SVG XML text.
    Svg,
    /// Raw RGBA bitmap image data.
    Bitmap,
    /// Standalone plain text.
    Text,
}

/// A pasteable clipboard payload after MIME precedence has been resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    /// Serialized SVG XML text.
    Svg(String),
    /// Raw RGBA bitmap image data.
    Bitmap {
        /// RGBA pixel buffer.
        pixels: Vec<u8>,
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
    },
    /// Standalone plain text.
    Text(String),
}

impl ClipboardPayload {
    /// Return the kind of this resolved payload.
    pub fn kind(&self) -> ClipboardPayloadKind {
        match self {
            Self::Svg(_) => ClipboardPayloadKind::Svg,
            Self::Bitmap { .. } => ClipboardPayloadKind::Bitmap,
            Self::Text(_) => ClipboardPayloadKind::Text,
        }
    }
}

/// Clipboard contents as independent formats before paste precedence is chosen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    /// SVG XML text payload, when available.
    pub svg_text: Option<String>,
    /// Raw RGBA bitmap payload, when available.
    pub bitmap: Option<ClipboardBitmap>,
    /// Plain text payload, when available.
    pub text: Option<String>,
}

impl ClipboardSnapshot {
    /// Resolve the payload Sivgo should paste.
    ///
    /// This encodes the desktop UI precedence: SVG fragments first, bitmap
    /// images second, and standalone plain text last.
    pub fn preferred_payload(self) -> Option<ClipboardPayload> {
        if let Some(svg) = self.svg_text.filter(|svg| looks_like_svg_text(svg)) {
            return Some(ClipboardPayload::Svg(svg));
        }

        if let Some((pixels, width, height)) = self
            .bitmap
            .filter(|(pixels, width, height)| *width > 0 && *height > 0 && !pixels.is_empty())
        {
            return Some(ClipboardPayload::Bitmap {
                pixels,
                width,
                height,
            });
        }

        self.text
            .filter(|text| !text.trim().is_empty())
            .map(ClipboardPayload::Text)
    }
}

fn looks_like_svg_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("<svg") || trimmed.starts_with("<?xml")
}

// ---------------------------------------------------------------------------
// Thread-local stub store (used by the free-standing functions)
// ---------------------------------------------------------------------------

/// In-memory clipboard contents for the stub/free-function API.
#[derive(Default)]
struct StubClipboard {
    svg_text: Option<String>,
    text: Option<String>,
    image: Option<(Vec<u8>, u32, u32)>,
}

thread_local! {
    static STUB_CLIPBOARD: RefCell<StubClipboard> = RefCell::new(StubClipboard::default());
}

// ---------------------------------------------------------------------------
// Free-standing functions (stub implementation)
// ---------------------------------------------------------------------------

/// Copy an SVG string to the thread-local clipboard store.
pub fn copy_svg(svg_text: &str) -> Result<(), String> {
    STUB_CLIPBOARD.with(|cell| {
        let mut clipboard = cell.borrow_mut();
        clipboard.svg_text = Some(svg_text.to_string());
        clipboard.text = Some(svg_text.to_string());
        clipboard.image = None;
    });
    Ok(())
}

/// Paste an SVG string from the thread-local clipboard store.
///
/// Returns `Ok(None)` if nothing has been copied.
pub fn paste_svg() -> Result<Option<String>, String> {
    STUB_CLIPBOARD.with(|cell| Ok(cell.borrow().svg_text.clone()))
}

/// Copy plain text to the thread-local clipboard store.
pub fn copy_text(text: &str) -> Result<(), String> {
    STUB_CLIPBOARD.with(|cell| {
        let mut clipboard = cell.borrow_mut();
        clipboard.svg_text = None;
        clipboard.text = Some(text.to_string());
        clipboard.image = None;
    });
    Ok(())
}

/// Paste plain text from the thread-local clipboard store.
///
/// Returns `Ok(None)` if no text has been copied.
pub fn paste_text() -> Result<Option<String>, String> {
    STUB_CLIPBOARD.with(|cell| Ok(cell.borrow().text.clone()))
}

/// Copy raw RGBA pixel data to the thread-local clipboard store.
///
/// `pixels` must contain exactly `w * h * 4` bytes (RGBA order).
pub fn copy_image(pixels: &[u8], w: u32, h: u32) -> Result<(), String> {
    let expected = (w as usize) * (h as usize) * 4;
    if pixels.len() != expected {
        return Err(format!(
            "Pixel buffer size mismatch: expected {expected} bytes, got {}",
            pixels.len()
        ));
    }
    STUB_CLIPBOARD.with(|cell| {
        let mut clipboard = cell.borrow_mut();
        clipboard.svg_text = None;
        clipboard.text = None;
        clipboard.image = Some((pixels.to_vec(), w, h));
    });
    Ok(())
}

/// Paste image data from the thread-local clipboard store.
///
/// Returns `Ok(None)` if no image has been copied.
pub fn paste_image() -> Result<Option<(Vec<u8>, u32, u32)>, String> {
    STUB_CLIPBOARD.with(|cell| Ok(cell.borrow().image.clone()))
}

/// Paste the best available payload from the thread-local clipboard store.
pub fn paste_preferred() -> Result<Option<ClipboardPayload>, String> {
    STUB_CLIPBOARD.with(|cell| {
        let clipboard = cell.borrow();
        Ok(ClipboardSnapshot {
            svg_text: clipboard.svg_text.clone(),
            bitmap: clipboard.image.clone(),
            text: clipboard.text.clone(),
        }
        .preferred_payload())
    })
}

/// Manages reading and writing SVG / bitmap data to the system clipboard.
pub struct ClipboardManager {
    clipboard: Clipboard,
}

impl ClipboardManager {
    /// Create a new `ClipboardManager`.
    ///
    /// Returns `Err` when the platform clipboard is unavailable.
    pub fn new() -> Result<Self, String> {
        let clipboard = Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
        Ok(Self { clipboard })
    }

    /// Copy an SVG string to the clipboard as plain text.
    pub fn copy_svg(&mut self, svg_content: &str) -> Result<(), String> {
        self.clipboard
            .set_text(svg_content)
            .map_err(|e| format!("Failed to copy SVG to clipboard: {e}"))
    }

    /// Copy plain text to the clipboard.
    pub fn copy_text(&mut self, text: &str) -> Result<(), String> {
        self.clipboard
            .set_text(text)
            .map_err(|e| format!("Failed to copy text to clipboard: {e}"))
    }

    /// Try to paste text from the clipboard and return it only when it looks
    /// like SVG (starts with `<svg` or `<?xml`).
    pub fn paste_svg(&mut self) -> Result<Option<String>, String> {
        match self.clipboard.get_text() {
            Ok(text) if looks_like_svg_text(&text) => Ok(Some(text)),
            Ok(_) => Ok(None),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(e) => Err(format!("Failed to read clipboard: {e}")),
        }
    }

    /// Paste plain text from the clipboard.
    pub fn paste_text(&mut self) -> Result<Option<String>, String> {
        match self.clipboard.get_text() {
            Ok(text) if !text.trim().is_empty() => Ok(Some(text)),
            Ok(_) => Ok(None),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(e) => Err(format!("Failed to read text clipboard: {e}")),
        }
    }

    /// Copy raw RGBA pixel data to the clipboard as an image.
    ///
    /// `pixels` must contain exactly `width * height * 4` bytes (RGBA order).
    pub fn copy_bitmap(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
        let expected = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected {
            return Err(format!(
                "Pixel buffer size mismatch: expected {expected} bytes, got {}",
                pixels.len()
            ));
        }

        let img = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Borrowed(pixels),
        };

        self.clipboard
            .set_image(img)
            .map_err(|e| format!("Failed to copy bitmap to clipboard: {e}"))
    }

    /// Paste raw RGBA pixel data from the clipboard.
    pub fn paste_bitmap(&mut self) -> Result<Option<(Vec<u8>, u32, u32)>, String> {
        match self.clipboard.get_image() {
            Ok(img) => Ok(Some((
                img.bytes.into_owned(),
                img.width as u32,
                img.height as u32,
            ))),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(e) => Err(format!("Failed to read bitmap clipboard: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_stub_clipboard() {
        STUB_CLIPBOARD.with(|cell| {
            *cell.borrow_mut() = StubClipboard::default();
        });
    }

    /// Verify construction succeeds (or fails gracefully on headless systems).
    #[test]
    fn test_clipboard_manager_creation() {
        // On CI / headless environments, `new()` may legitimately fail.
        // We just assert it doesn't panic.
        let _result = ClipboardManager::new();
    }

    /// `paste_svg` should return `None` when clipboard text is not SVG.
    #[test]
    fn test_paste_svg_returns_none_for_non_svg() {
        if let Ok(mut mgr) = ClipboardManager::new() {
            // Put plain text on the clipboard.
            let _ = mgr.clipboard.set_text("hello world");
            match mgr.paste_svg() {
                Ok(None) => {} // expected
                Ok(Some(_)) => panic!("Expected None for non-SVG text"),
                Err(_) => {} // clipboard error is acceptable on some platforms
            }
        }
    }

    /// `copy_bitmap` should reject a pixel buffer with the wrong size.
    #[test]
    fn test_copy_bitmap_size_mismatch() {
        if let Ok(mut mgr) = ClipboardManager::new() {
            let result = mgr.copy_bitmap(&[0u8; 10], 2, 2);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("size mismatch"));
        }
    }

    // -------------------------------------------------------------------
    // Free-standing function tests (thread-local stub)
    // -------------------------------------------------------------------

    #[test]
    fn test_stub_copy_paste_svg() {
        clear_stub_clipboard();
        let svg = "<svg><rect/></svg>";
        copy_svg(svg).unwrap();
        let result = paste_svg().unwrap();
        assert_eq!(result.as_deref(), Some(svg));
        assert_eq!(paste_text().unwrap().as_deref(), Some(svg));
    }

    #[test]
    fn test_stub_paste_svg_empty() {
        clear_stub_clipboard();
        let result = paste_svg().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_stub_copy_paste_text() {
        clear_stub_clipboard();
        copy_text("hello").unwrap();
        assert_eq!(paste_text().unwrap().as_deref(), Some("hello"));
    }

    #[test]
    fn test_stub_copy_paste_image() {
        clear_stub_clipboard();
        // 2x2 RGBA = 16 bytes
        let pixels = vec![0u8; 16];
        copy_image(&pixels, 2, 2).unwrap();
        let result = paste_image().unwrap().unwrap();
        assert_eq!(result.0.len(), 16);
        assert_eq!(result.1, 2);
        assert_eq!(result.2, 2);
    }

    #[test]
    fn test_stub_copy_image_size_mismatch() {
        let result = copy_image(&[0u8; 5], 2, 2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("size mismatch"));
    }

    #[test]
    fn test_stub_paste_image_empty() {
        clear_stub_clipboard();
        let result = paste_image().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_clipboard_snapshot_prefers_svg_over_bitmap_and_text() {
        let payload = ClipboardSnapshot {
            svg_text: Some("<svg><rect/></svg>".to_string()),
            bitmap: Some((vec![255; 4], 1, 1)),
            text: Some("plain text".to_string()),
        }
        .preferred_payload()
        .unwrap();

        assert_eq!(payload.kind(), ClipboardPayloadKind::Svg);
        assert!(matches!(payload, ClipboardPayload::Svg(svg) if svg.contains("<rect")));
    }

    #[test]
    fn test_clipboard_snapshot_prefers_bitmap_over_text() {
        let payload = ClipboardSnapshot {
            svg_text: None,
            bitmap: Some((vec![0, 0, 0, 255], 1, 1)),
            text: Some("plain text".to_string()),
        }
        .preferred_payload()
        .unwrap();

        assert_eq!(payload.kind(), ClipboardPayloadKind::Bitmap);
        assert!(matches!(
            payload,
            ClipboardPayload::Bitmap {
                pixels,
                width: 1,
                height: 1,
            } if pixels == vec![0, 0, 0, 255]
        ));
    }

    #[test]
    fn test_clipboard_snapshot_uses_text_last() {
        let payload = ClipboardSnapshot {
            svg_text: None,
            bitmap: None,
            text: Some("plain text".to_string()),
        }
        .preferred_payload()
        .unwrap();

        assert_eq!(payload.kind(), ClipboardPayloadKind::Text);
        assert_eq!(payload, ClipboardPayload::Text("plain text".to_string()));
    }

    #[test]
    fn test_stub_copy_text_clears_stale_svg_and_image() {
        clear_stub_clipboard();
        copy_svg("<svg><rect/></svg>").unwrap();
        copy_text("hello").unwrap();
        assert!(paste_svg().unwrap().is_none());
        assert_eq!(paste_text().unwrap().as_deref(), Some("hello"));

        copy_image(&[0, 0, 0, 255], 1, 1).unwrap();
        copy_text("hello again").unwrap();

        assert!(paste_svg().unwrap().is_none());
        assert!(paste_image().unwrap().is_none());
        assert_eq!(paste_text().unwrap().as_deref(), Some("hello again"));
        assert_eq!(
            paste_preferred().unwrap(),
            Some(ClipboardPayload::Text("hello again".to_string()))
        );
    }

    #[test]
    fn test_stub_copy_image_clears_stale_svg_and_text() {
        clear_stub_clipboard();
        copy_svg("<svg><rect/></svg>").unwrap();
        copy_text("hello").unwrap();
        copy_image(&[0, 0, 0, 255], 1, 1).unwrap();

        assert!(paste_svg().unwrap().is_none());
        assert!(paste_text().unwrap().is_none());
        assert_eq!(
            paste_preferred().unwrap(),
            Some(ClipboardPayload::Bitmap {
                pixels: vec![0, 0, 0, 255],
                width: 1,
                height: 1,
            })
        );
    }
}
