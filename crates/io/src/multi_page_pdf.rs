//! Multi-page PDF export (17.6.4).
//!
//! Provides a standalone multi-page PDF exporter that takes raw SVG content
//! strings per page and writes a valid PDF file. Unlike the document-level
//! `export_pdf_multipage`, this works with arbitrary SVG content strings and
//! supports per-page dimensions.

use std::path::Path;

/// Metadata embedded in the PDF Info dictionary.
#[derive(Debug, Clone)]
pub struct PdfMetadata {
    /// Optional document title.
    pub title: Option<String>,
    /// Optional document author.
    pub author: Option<String>,
    /// Creator application name.
    pub creator: String,
}

impl Default for PdfMetadata {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            creator: "Sivgo".to_string(),
        }
    }
}

/// A single page in the multi-page PDF.
#[derive(Debug, Clone)]
pub struct PdfPage {
    /// SVG content for this page (raw SVG markup).
    pub content_svg: String,
    /// Page width in points (1 pt = 1/72 inch).
    pub width: f64,
    /// Page height in points.
    pub height: f64,
    /// 1-based page number.
    pub page_number: usize,
}

/// Multi-page PDF exporter.
///
/// Accumulates pages and metadata, then writes them all into a single PDF.
#[derive(Debug, Clone)]
pub struct MultiPagePdfExporter {
    /// The pages to export.
    pub pages: Vec<PdfPage>,
    /// PDF metadata.
    pub metadata: PdfMetadata,
}

// ---------------------------------------------------------------------------
// Internal PDF writer (mirrors the one in pdf.rs but kept self-contained)
// ---------------------------------------------------------------------------

struct PdfWriter {
    buf: Vec<u8>,
    offsets: Vec<usize>,
    next_obj: u32,
}

impl PdfWriter {
    fn new() -> Self {
        let mut w = Self {
            buf: Vec::new(),
            offsets: vec![0],
            next_obj: 1,
        };
        w.buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        w
    }

    fn reserve_obj(&mut self) -> u32 {
        let n = self.next_obj;
        self.next_obj += 1;
        n
    }

    fn begin_obj(&mut self) -> u32 {
        let n = self.next_obj;
        self.next_obj += 1;
        while self.offsets.len() <= n as usize {
            self.offsets.push(0);
        }
        self.offsets[n as usize] = self.buf.len();
        let header = format!("{n} 0 obj\n");
        self.buf.extend_from_slice(header.as_bytes());
        n
    }

    fn begin_reserved_obj(&mut self, n: u32) {
        while self.offsets.len() <= n as usize {
            self.offsets.push(0);
        }
        self.offsets[n as usize] = self.buf.len();
        let header = format!("{n} 0 obj\n");
        self.buf.extend_from_slice(header.as_bytes());
    }

    fn end_obj(&mut self) {
        self.buf.extend_from_slice(b"endobj\n");
    }

    fn write_raw(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    fn finish(mut self, catalog_obj: u32, info_obj: Option<u32>) -> Vec<u8> {
        let xref_offset = self.buf.len();
        let num_objects = self.offsets.len();

        let xref_header = format!("xref\n0 {num_objects}\n");
        self.buf.extend_from_slice(xref_header.as_bytes());

        self.buf.extend_from_slice(b"0000000000 65535 f \n");
        for i in 1..num_objects {
            let entry = format!("{:010} 00000 n \n", self.offsets[i]);
            self.buf.extend_from_slice(entry.as_bytes());
        }

        self.buf.extend_from_slice(b"trailer\n");
        let mut trailer = format!("<< /Size {num_objects} /Root {catalog_obj} 0 R");
        if let Some(info) = info_obj {
            trailer.push_str(&format!(" /Info {info} 0 R"));
        }
        trailer.push_str(" >>\n");
        self.buf.extend_from_slice(trailer.as_bytes());

        let startxref = format!("startxref\n{xref_offset}\n%%EOF\n");
        self.buf.extend_from_slice(startxref.as_bytes());

        self.buf
    }
}

/// Escape special characters in a PDF string literal.
fn pdf_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

/// Format an f64 compactly for PDF output.
fn fmt_f64(v: f64) -> String {
    if v == v.round() && v.abs() < 1e12 {
        format!("{}", v as i64)
    } else {
        format!("{:.4}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

impl MultiPagePdfExporter {
    /// Create a new exporter with no pages and default metadata.
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
        }
    }

    /// Add a page with the given SVG content and dimensions.
    pub fn add_page(&mut self, content_svg: String, width: f64, height: f64) {
        let page_number = self.pages.len() + 1;
        self.pages.push(PdfPage {
            content_svg,
            width,
            height,
            page_number,
        });
    }

    /// Export the accumulated pages to a PDF file at the given `path`.
    ///
    /// Each page's SVG content is embedded as a text stream in the
    /// corresponding PDF page object. Returns an error if the file cannot
    /// be written.
    pub fn export(&self, path: &Path) -> Result<(), std::io::Error> {
        let bytes = self.render_pdf();
        std::fs::write(path, bytes)
    }

    /// Render the PDF to an in-memory byte vector.
    pub fn render_pdf(&self) -> Vec<u8> {
        if self.pages.is_empty() {
            return self.render_empty_pdf();
        }

        let mut w = PdfWriter::new();
        let num_pages = self.pages.len();

        let catalog_obj = w.reserve_obj();
        let pages_obj = w.reserve_obj();

        // Reserve page objects.
        let page_objs: Vec<u32> = (0..num_pages).map(|_| w.reserve_obj()).collect();

        // Write content streams and page objects.
        for (i, page) in self.pages.iter().enumerate() {
            // Build a simple content stream. We embed the SVG content as a
            // text comment followed by a basic drawing command placeholder.
            // In a full implementation this would render the SVG to PDF
            // drawing operators; here we create a valid stream with the
            // SVG source embedded.
            let stream_content = self.build_page_stream(page);
            let stream_bytes = stream_content.as_bytes();

            let stream_obj = w.begin_obj();
            let stream_dict = format!("<< /Length {} >>\nstream\n", stream_bytes.len());
            w.write_raw(stream_dict.as_bytes());
            w.write_raw(stream_bytes);
            w.write_raw(b"\nendstream\n");
            w.end_obj();

            w.begin_reserved_obj(page_objs[i]);
            let page_dict = format!(
                "<< /Type /Page /Parent {pages_obj} 0 R /MediaBox [0 0 {} {}] /Contents {stream_obj} 0 R >>\n",
                fmt_f64(page.width),
                fmt_f64(page.height),
            );
            w.write_raw(page_dict.as_bytes());
            w.end_obj();
        }

        // Pages object.
        w.begin_reserved_obj(pages_obj);
        let kids: Vec<String> = page_objs.iter().map(|n| format!("{n} 0 R")).collect();
        let pages_dict = format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>\n",
            kids.join(" "),
            num_pages
        );
        w.write_raw(pages_dict.as_bytes());
        w.end_obj();

        // Info dictionary.
        let info_obj = {
            let obj = w.begin_obj();
            let mut info = String::from("<< ");
            if let Some(ref title) = self.metadata.title {
                info.push_str(&format!("/Title ({}) ", pdf_escape_string(title)));
            }
            if let Some(ref author) = self.metadata.author {
                info.push_str(&format!("/Author ({}) ", pdf_escape_string(author)));
            }
            info.push_str(&format!(
                "/Creator ({}) ",
                pdf_escape_string(&self.metadata.creator)
            ));
            info.push_str(">>\n");
            w.write_raw(info.as_bytes());
            w.end_obj();
            Some(obj)
        };

        // Catalog.
        w.begin_reserved_obj(catalog_obj);
        let catalog = format!("<< /Type /Catalog /Pages {pages_obj} 0 R >>\n");
        w.write_raw(catalog.as_bytes());
        w.end_obj();

        w.finish(catalog_obj, info_obj)
    }

    /// Build the content stream for a single page.
    fn build_page_stream(&self, page: &PdfPage) -> String {
        // Parse the SVG content and render drawing operators.
        // For arbitrary SVG strings we attempt to parse through sivgo-dom;
        // if that fails we emit a placeholder comment stream.
        match sivgo_dom::parse_svg(&page.content_svg) {
            Ok(doc) => {
                // Use the existing PDF rendering infrastructure.
                let opts = crate::pdf::PdfExportOptions {
                    width: page.width,
                    height: page.height,
                    title: None,
                    author: None,
                };
                // We re-use the content stream generator from the pdf module.
                // Since render_content_stream is private, we render a full PDF
                // and extract just the stream portion. This is expedient for
                // now; a future refactor could expose the stream builder.
                let full_pdf = crate::pdf::export_pdf(&doc, &opts);
                let text = String::from_utf8_lossy(&full_pdf);
                // Extract content between "stream\n" and "\nendstream".
                if let Some(start) = text.find("stream\n") {
                    let after = &text[start + 7..];
                    if let Some(end) = after.find("\nendstream") {
                        return after[..end].to_string();
                    }
                }
                // Fallback: empty stream with a comment.
                format!("% Page {}\n", page.page_number)
            }
            Err(_) => {
                // If SVG parsing fails, embed a comment noting the failure.
                format!("% Page {} (SVG parse error)\n", page.page_number)
            }
        }
    }

    /// Render an empty single-page PDF (used when there are no pages).
    fn render_empty_pdf(&self) -> Vec<u8> {
        let mut w = PdfWriter::new();
        let catalog_obj = w.reserve_obj();
        let pages_obj = w.reserve_obj();
        let page_obj = w.reserve_obj();

        // Empty content stream.
        let stream_obj = w.begin_obj();
        w.write_raw(b"<< /Length 0 >>\nstream\n\nendstream\n");
        w.end_obj();

        w.begin_reserved_obj(page_obj);
        let page_dict = format!(
            "<< /Type /Page /Parent {pages_obj} 0 R /MediaBox [0 0 595 842] /Contents {stream_obj} 0 R >>\n"
        );
        w.write_raw(page_dict.as_bytes());
        w.end_obj();

        w.begin_reserved_obj(pages_obj);
        let pages_dict = format!("<< /Type /Pages /Kids [{page_obj} 0 R] /Count 1 >>\n");
        w.write_raw(pages_dict.as_bytes());
        w.end_obj();

        w.begin_reserved_obj(catalog_obj);
        let catalog = format!("<< /Type /Catalog /Pages {pages_obj} 0 R >>\n");
        w.write_raw(catalog.as_bytes());
        w.end_obj();

        w.finish(catalog_obj, None)
    }
}

impl Default for MultiPagePdfExporter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <rect x="10" y="10" width="80" height="80" fill="red"/>
    </svg>"#;

    #[test]
    fn new_exporter_has_no_pages() {
        let exp = MultiPagePdfExporter::new();
        assert!(exp.pages.is_empty());
        assert_eq!(exp.metadata.creator, "Sivgo");
    }

    #[test]
    fn add_page_increments_count() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);
        assert_eq!(exp.pages.len(), 1);
        assert_eq!(exp.pages[0].page_number, 1);

        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);
        assert_eq!(exp.pages.len(), 2);
        assert_eq!(exp.pages[1].page_number, 2);
    }

    #[test]
    fn render_empty_produces_valid_pdf() {
        let exp = MultiPagePdfExporter::new();
        let pdf = exp.render_pdf();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let tail = String::from_utf8_lossy(&pdf[pdf.len().saturating_sub(20)..]);
        assert!(tail.contains("%%EOF"));
    }

    #[test]
    fn render_single_page_pdf() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);
        exp.metadata.title = Some("Test Title".to_string());

        let pdf = exp.render_pdf();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 1"));
        assert!(text.contains("/Title (Test Title)"));
        assert!(text.contains("/Creator (Sivgo)"));
    }

    #[test]
    fn render_multi_page_pdf() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);
        exp.add_page(SIMPLE_SVG.to_string(), 612.0, 792.0);
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);

        let pdf = exp.render_pdf();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 3"));
        assert!(pdf.starts_with(b"%PDF-1.4"));
    }

    #[test]
    fn render_with_metadata() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);
        exp.metadata.title = Some("My Document".to_string());
        exp.metadata.author = Some("Test Author".to_string());
        exp.metadata.creator = "Sivgo Test".to_string();

        let pdf = exp.render_pdf();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Title (My Document)"));
        assert!(text.contains("/Author (Test Author)"));
        assert!(text.contains("/Creator (Sivgo Test)"));
    }

    #[test]
    fn export_to_file() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 595.0, 842.0);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_output.pdf");
        exp.export(&path).unwrap();

        assert!(path.exists());
        let contents = std::fs::read(&path).unwrap();
        assert!(contents.starts_with(b"%PDF-1.4"));
    }

    #[test]
    fn per_page_dimensions() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page(SIMPLE_SVG.to_string(), 400.0, 300.0);
        exp.add_page(SIMPLE_SVG.to_string(), 800.0, 600.0);

        let pdf = exp.render_pdf();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("400 300"), "first page dimensions");
        assert!(text.contains("800 600"), "second page dimensions");
    }

    #[test]
    fn invalid_svg_produces_fallback_stream() {
        let mut exp = MultiPagePdfExporter::new();
        exp.add_page("not valid svg at all".to_string(), 595.0, 842.0);

        let pdf = exp.render_pdf();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        // Should still produce a valid PDF structure.
        let tail = String::from_utf8_lossy(&pdf[pdf.len().saturating_sub(20)..]);
        assert!(tail.contains("%%EOF"));
    }

    #[test]
    fn default_metadata() {
        let meta = PdfMetadata::default();
        assert!(meta.title.is_none());
        assert!(meta.author.is_none());
        assert_eq!(meta.creator, "Sivgo");
    }

    #[test]
    fn pdf_escape_special_chars() {
        assert_eq!(pdf_escape_string("hello"), "hello");
        assert_eq!(pdf_escape_string("a(b)c"), "a\\(b\\)c");
        assert_eq!(pdf_escape_string("a\\b"), "a\\\\b");
    }
}
