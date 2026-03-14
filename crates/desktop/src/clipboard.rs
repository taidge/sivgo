//! System clipboard integration for Sivgo desktop (12.5).
//!
//! Uses the `arboard` crate for cross-platform clipboard access.  If the
//! clipboard cannot be initialised (e.g. headless CI), every operation returns
//! a human-readable error instead of panicking.
//!
//! In addition to the struct-based [`ClipboardManager`], this module provides
//! free-standing functions ([`copy_svg`], [`paste_svg`], [`copy_image`],
//! [`paste_image`]) that use a thread-local in-memory store, suitable for
//! testing and environments where the system clipboard is unavailable.

use arboard::Clipboard;
use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Thread-local stub store (used by the free-standing functions)
// ---------------------------------------------------------------------------

/// In-memory clipboard contents for the stub/free-function API.
#[derive(Default)]
struct StubClipboard {
    svg_text: Option<String>,
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
        cell.borrow_mut().svg_text = Some(svg_text.to_string());
    });
    Ok(())
}

/// Paste an SVG string from the thread-local clipboard store.
///
/// Returns `Ok(None)` if nothing has been copied.
pub fn paste_svg() -> Result<Option<String>, String> {
    STUB_CLIPBOARD.with(|cell| Ok(cell.borrow().svg_text.clone()))
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
        cell.borrow_mut().image = Some((pixels.to_vec(), w, h));
    });
    Ok(())
}

/// Paste image data from the thread-local clipboard store.
///
/// Returns `Ok(None)` if no image has been copied.
pub fn paste_image() -> Result<Option<(Vec<u8>, u32, u32)>, String> {
    STUB_CLIPBOARD.with(|cell| Ok(cell.borrow().image.clone()))
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

    /// Try to paste text from the clipboard and return it only when it looks
    /// like SVG (starts with `<svg` or `<?xml`).
    pub fn paste_svg(&mut self) -> Result<Option<String>, String> {
        let text = self
            .clipboard
            .get_text()
            .map_err(|e| format!("Failed to read clipboard: {e}"))?;

        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") {
            Ok(Some(text))
        } else {
            Ok(None)
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        let svg = "<svg><rect/></svg>";
        copy_svg(svg).unwrap();
        let result = paste_svg().unwrap();
        assert_eq!(result.as_deref(), Some(svg));
    }

    #[test]
    fn test_stub_paste_svg_empty() {
        // Clear state first.
        STUB_CLIPBOARD.with(|cell| cell.borrow_mut().svg_text = None);
        let result = paste_svg().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_stub_copy_paste_image() {
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
        STUB_CLIPBOARD.with(|cell| cell.borrow_mut().image = None);
        let result = paste_image().unwrap();
        assert!(result.is_none());
    }
}
