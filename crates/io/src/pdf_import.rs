//! Stub PDF importer for the Sivgo editor.
//!
//! Full PDF parsing is extremely complex, so this module provides a minimal
//! importer that creates an `SvgDocument` with an `<image>` placeholder
//! element.  If the input looks like a simple text-based PDF, basic text
//! content is extracted into an SVG `<text>` element.

use sivgo_dom::{
    ImageAttrs, LengthUnit, SvgDocument, SvgElement, SvgLength, SvgSvgAttrs, TextAttrs, ViewBox,
};

/// Options for PDF import.
#[derive(Debug, Clone)]
pub struct PdfImportOptions {
    /// Zero-based page index to import.
    pub page: usize,
    /// Resolution (dots per inch) used for rasterization sizing.
    pub dpi: f64,
}

impl Default for PdfImportOptions {
    fn default() -> Self {
        Self { page: 0, dpi: 96.0 }
    }
}

/// Import a PDF from raw bytes and return an `SvgDocument`.
///
/// This is currently a **stub implementation**:
///
/// * If the data is empty or does not start with the PDF magic bytes (`%PDF`),
///   an error is returned.
/// * If basic text content can be detected between `BT` / `ET` markers, a
///   `<text>` element is included.
/// * Otherwise a placeholder `<image>` element is created representing where
///   a rasterised page would go.
pub fn import_pdf(data: &[u8], options: &PdfImportOptions) -> Result<SvgDocument, String> {
    // Validate input.
    if data.is_empty() {
        return Err("PDF data is empty".into());
    }
    if data.len() < 4 || &data[0..4] != b"%PDF" {
        return Err("Not a valid PDF (missing %PDF header)".into());
    }

    // Compute page dimensions based on US-Letter at the requested DPI.
    let page_w_pt = 612.0_f64; // 8.5 in * 72 pt/in
    let page_h_pt = 792.0_f64; // 11  in * 72 pt/in
    let scale = options.dpi / 72.0;
    let width = page_w_pt * scale;
    let height = page_h_pt * scale;

    let mut doc = SvgDocument::new();

    let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
        width: Some(SvgLength::px(width)),
        height: Some(SvgLength::px(height)),
        view_box: Some(ViewBox::new(0.0, 0.0, width, height)),
        preserve_aspect_ratio: None,
    }));
    doc.set_root(svg_id);

    // Attempt rudimentary text extraction.
    let text = extract_pdf_text(data);
    if !text.is_empty() {
        let text_id = doc.add_node(SvgElement::Text(TextAttrs {
            x: vec![SvgLength::px(10.0)],
            y: vec![SvgLength::px(20.0)],
            font_size: Some(SvgLength::new(12.0, LengthUnit::Px)),
            text_content: text,
            ..Default::default()
        }));
        doc.append_child(svg_id, text_id);
    }

    // Always add a placeholder <image> element.
    let img_id = doc.add_node(SvgElement::Image(ImageAttrs {
        href: Some(format!(
            "data:application/pdf;page={};base64,STUB",
            options.page
        )),
        x: Some(SvgLength::px(0.0)),
        y: Some(SvgLength::px(0.0)),
        width: Some(SvgLength::px(width)),
        height: Some(SvgLength::px(height)),
        ..Default::default()
    }));
    doc.append_child(svg_id, img_id);

    Ok(doc)
}

/// Very naive text extraction: look for `BT ... (text) ... ET` sequences.
fn extract_pdf_text(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    let mut result = String::new();

    let mut in_bt = false;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed == "BT" {
            in_bt = true;
            continue;
        }
        if trimmed == "ET" {
            in_bt = false;
            continue;
        }
        if in_bt {
            // Look for parenthesised strings: (Hello World) Tj
            if let Some(start) = trimmed.find('(') {
                if let Some(end) = trimmed.rfind(')') {
                    if end > start {
                        let fragment = &trimmed[start + 1..end];
                        if !result.is_empty() {
                            result.push(' ');
                        }
                        result.push_str(fragment);
                    }
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_invalid_data_returns_error() {
        let empty_err = import_pdf(&[], &PdfImportOptions::default());
        assert!(empty_err.is_err());
        assert_eq!(empty_err.unwrap_err(), "PDF data is empty");

        let junk_err = import_pdf(b"not a pdf", &PdfImportOptions::default());
        assert!(junk_err.is_err());
        assert!(junk_err.unwrap_err().contains("missing %PDF header"));
    }

    #[test]
    fn stub_returns_document_with_image_placeholder() {
        // Minimal fake PDF header (not a real PDF, but enough for the stub).
        let fake_pdf = b"%PDF-1.4 fake content";
        let opts = PdfImportOptions { page: 0, dpi: 96.0 };
        let doc = import_pdf(fake_pdf, &opts).expect("should succeed for data starting with %PDF");
        let root_id = doc.root().expect("document should have a root");
        let root = doc.get_node(root_id).unwrap();

        // Root must be <svg>.
        assert_eq!(root.element.tag_name(), "svg");

        // Should have at least one child (the <image> placeholder).
        let children = doc.children(root_id);
        assert!(!children.is_empty(), "expected at least one child element");

        // The last child should be the <image> placeholder.
        let img = doc.get_node(*children.last().unwrap()).unwrap();
        assert_eq!(img.element.tag_name(), "image");
    }
}
