//! PDF export for SVG documents (10.8).
//!
//! Provides [`export_pdf`] which writes a PDF 1.4 file from an
//! [`sivgo_dom::SvgDocument`].  Supports basic shapes (rect, circle, ellipse,
//! line, path, polyline, polygon), solid fills, strokes, and affine transforms.

use std::path::Path;

use sivgo_dom::{
    NodeId, Paint, PresentationAttrs, SvgDocument, SvgElement,
};
use sivgo_math::PathSegment;

use crate::IoError;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Options controlling PDF export.
#[derive(Debug, Clone)]
pub struct PdfExportOptions {
    /// Resolution in dots per inch (used for coordinate scaling).
    pub dpi: f64,
    /// Page width in points (1 pt = 1/72 inch).
    pub page_width: f64,
    /// Page height in points.
    pub page_height: f64,
    /// Whether to embed fonts (reserved for future use).
    pub embed_fonts: bool,
    /// Whether to include metadata (title, producer) in the PDF Info dictionary.
    pub include_metadata: bool,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            page_width: 595.0,   // A4 width in points
            page_height: 842.0,  // A4 height in points
            embed_fonts: true,
            include_metadata: true,
        }
    }
}

// ---------------------------------------------------------------------------
// PDF object helper
// ---------------------------------------------------------------------------

/// Minimal incremental PDF writer that tracks object offsets for the xref table.
struct PdfWriter {
    buf: Vec<u8>,
    /// Byte offset of each object (1-indexed; index 0 is unused).
    offsets: Vec<usize>,
    next_obj: u32,
}

impl PdfWriter {
    fn new() -> Self {
        let mut w = PdfWriter {
            buf: Vec::new(),
            offsets: vec![0], // placeholder for object 0
            next_obj: 1,
        };
        w.buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        w
    }

    /// Reserve the next object number without writing anything yet.
    fn reserve_obj(&mut self) -> u32 {
        let n = self.next_obj;
        self.next_obj += 1;
        n
    }

    /// Begin an indirect object and return its number.
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

    /// Begin a previously reserved object.
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

    /// Write the cross-reference table and trailer, then return the final bytes.
    fn finish(mut self, catalog_obj: u32, info_obj: Option<u32>) -> Vec<u8> {
        let xref_offset = self.buf.len();
        let num_objects = self.offsets.len();

        let xref_header = format!("xref\n0 {num_objects}\n");
        self.buf.extend_from_slice(xref_header.as_bytes());

        // Object 0 (free head).
        self.buf
            .extend_from_slice(b"0000000000 65535 f \n");

        for i in 1..num_objects {
            let entry = format!("{:010} 00000 n \n", self.offsets[i]);
            self.buf.extend_from_slice(entry.as_bytes());
        }

        self.buf.extend_from_slice(b"trailer\n");
        let mut trailer = format!(
            "<< /Size {num_objects} /Root {catalog_obj} 0 R"
        );
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

// ---------------------------------------------------------------------------
// Content stream generation
// ---------------------------------------------------------------------------

/// Render the document's visible elements into a PDF content stream string.
/// The coordinate system is flipped so that SVG's top-left origin maps to
/// PDF's bottom-left origin.
fn render_content_stream(doc: &SvgDocument, page_height: f64) -> String {
    let mut cs = String::new();

    // Flip Y axis: PDF origin is bottom-left, SVG is top-left.
    cs.push_str(&format!(
        "1 0 0 -1 0 {h} cm\n",
        h = fmt_f64(page_height)
    ));

    if let Some(root) = doc.root() {
        render_node(doc, root, &mut cs);
    }

    cs
}

/// Recursively render a node and its children into the content stream.
fn render_node(doc: &SvgDocument, id: NodeId, cs: &mut String) {
    let node = match doc.get_node(id) {
        Some(n) => n,
        None => return,
    };

    let pres = &node.presentation;

    // Apply transform if present.
    let has_transform = pres.transform.is_some();
    if has_transform {
        cs.push_str("q\n"); // save graphics state
        if let Some(ref t) = pres.transform {
            let c = t.as_coeffs();
            cs.push_str(&format!(
                "{} {} {} {} {} {} cm\n",
                fmt_f64(c[0]),
                fmt_f64(c[1]),
                fmt_f64(c[2]),
                fmt_f64(c[3]),
                fmt_f64(c[4]),
                fmt_f64(c[5])
            ));
        }
    }

    match &node.element {
        SvgElement::Rect(a) => {
            let x = a.x.value;
            let y = a.y.value;
            let w = a.width.value;
            let h = a.height.value;
            if w > 0.0 && h > 0.0 {
                cs.push_str(&format!(
                    "{} {} {} {} re\n",
                    fmt_f64(x),
                    fmt_f64(y),
                    fmt_f64(w),
                    fmt_f64(h)
                ));
                apply_fill_stroke(pres, cs);
            }
        }
        SvgElement::Circle(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let r = a.r.value;
            if r > 0.0 {
                emit_circle_path(cx, cy, r, cs);
                apply_fill_stroke(pres, cs);
            }
        }
        SvgElement::Ellipse(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let rx = a.rx.value;
            let ry = a.ry.value;
            if rx > 0.0 && ry > 0.0 {
                emit_ellipse_path(cx, cy, rx, ry, cs);
                apply_fill_stroke(pres, cs);
            }
        }
        SvgElement::Line(a) => {
            let x1 = a.x1.value;
            let y1 = a.y1.value;
            let x2 = a.x2.value;
            let y2 = a.y2.value;
            cs.push_str(&format!(
                "{} {} m {} {} l\n",
                fmt_f64(x1),
                fmt_f64(y1),
                fmt_f64(x2),
                fmt_f64(y2)
            ));
            apply_stroke_state(pres, cs);
            cs.push_str("S\n");
        }
        SvgElement::Path(a) => {
            emit_bezpath(&a.d, cs);
            apply_fill_stroke(pres, cs);
        }
        SvgElement::Polyline(a) => {
            if let Some((&first, rest)) = a.points.split_first() {
                cs.push_str(&format!("{} {} m\n", fmt_f64(first.0), fmt_f64(first.1)));
                for pt in rest {
                    cs.push_str(&format!("{} {} l\n", fmt_f64(pt.0), fmt_f64(pt.1)));
                }
                apply_fill_stroke(pres, cs);
            }
        }
        SvgElement::Polygon(a) => {
            if let Some((&first, rest)) = a.points.split_first() {
                cs.push_str(&format!("{} {} m\n", fmt_f64(first.0), fmt_f64(first.1)));
                for pt in rest {
                    cs.push_str(&format!("{} {} l\n", fmt_f64(pt.0), fmt_f64(pt.1)));
                }
                cs.push_str("h\n"); // close path
                apply_fill_stroke(pres, cs);
            }
        }
        // Containers: recurse into children.
        SvgElement::Svg(_) | SvgElement::Group(_) => {
            for &child in doc.children(id) {
                render_node(doc, child, cs);
            }
        }
        // Other elements are skipped for now.
        _ => {}
    }

    if has_transform {
        cs.push_str("Q\n"); // restore graphics state
    }
}

/// Emit PDF path operators for a BezPath.
fn emit_bezpath(path: &sivgo_math::BezPath, cs: &mut String) {
    for seg in path.elements() {
        match seg {
            PathSegment::MoveTo(p) => {
                cs.push_str(&format!("{} {} m\n", fmt_f64(p.x), fmt_f64(p.y)));
            }
            PathSegment::LineTo(p) => {
                cs.push_str(&format!("{} {} l\n", fmt_f64(p.x), fmt_f64(p.y)));
            }
            PathSegment::QuadTo(p1, p2) => {
                // PDF has no native quadratic; promote to degenerate cubic.
                cs.push_str(&format!(
                    "{} {} {} {} {} {} c\n",
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p2.x),
                    fmt_f64(p2.y)
                ));
            }
            PathSegment::CurveTo(p1, p2, p3) => {
                cs.push_str(&format!(
                    "{} {} {} {} {} {} c\n",
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p2.x),
                    fmt_f64(p2.y),
                    fmt_f64(p3.x),
                    fmt_f64(p3.y)
                ));
            }
            PathSegment::ClosePath => {
                cs.push_str("h\n");
            }
        }
    }
}

/// Approximate a circle with four cubic Bezier arcs.
fn emit_circle_path(cx: f64, cy: f64, r: f64, cs: &mut String) {
    emit_ellipse_path(cx, cy, r, r, cs);
}

/// Approximate an ellipse with four cubic Bezier arcs (Kappa technique).
fn emit_ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64, cs: &mut String) {
    const KAPPA: f64 = 0.5522847498;
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;

    cs.push_str(&format!("{} {} m\n", fmt_f64(cx + rx), fmt_f64(cy)));
    cs.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        fmt_f64(cx + rx), fmt_f64(cy + ky),
        fmt_f64(cx + kx), fmt_f64(cy + ry),
        fmt_f64(cx),      fmt_f64(cy + ry)
    ));
    cs.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        fmt_f64(cx - kx), fmt_f64(cy + ry),
        fmt_f64(cx - rx), fmt_f64(cy + ky),
        fmt_f64(cx - rx), fmt_f64(cy)
    ));
    cs.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        fmt_f64(cx - rx), fmt_f64(cy - ky),
        fmt_f64(cx - kx), fmt_f64(cy - ry),
        fmt_f64(cx),      fmt_f64(cy - ry)
    ));
    cs.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        fmt_f64(cx + kx), fmt_f64(cy - ry),
        fmt_f64(cx + rx), fmt_f64(cy - ky),
        fmt_f64(cx + rx), fmt_f64(cy)
    ));
    cs.push_str("h\n");
}

/// Set stroke state (color and width) from presentation attrs.
fn apply_stroke_state(pres: &PresentationAttrs, cs: &mut String) {
    if let Some(Paint::Color(c)) = &pres.stroke {
        cs.push_str(&format!(
            "{} {} {} RG\n",
            fmt_f64(c.r as f64 / 255.0),
            fmt_f64(c.g as f64 / 255.0),
            fmt_f64(c.b as f64 / 255.0)
        ));
    }
    if let Some(ref sw) = pres.stroke_width {
        cs.push_str(&format!("{} w\n", fmt_f64(sw.value)));
    }
}

/// Emit the fill/stroke painting operator based on presentation attrs.
fn apply_fill_stroke(pres: &PresentationAttrs, cs: &mut String) {
    let has_fill = match &pres.fill {
        Some(Paint::Color(c)) => {
            cs.push_str(&format!(
                "{} {} {} rg\n",
                fmt_f64(c.r as f64 / 255.0),
                fmt_f64(c.g as f64 / 255.0),
                fmt_f64(c.b as f64 / 255.0)
            ));
            true
        }
        Some(Paint::None) => false,
        // Default SVG fill is black.
        None => {
            cs.push_str("0 0 0 rg\n");
            true
        }
        _ => false,
    };

    let has_stroke = matches!(&pres.stroke, Some(Paint::Color(_)));
    if has_stroke {
        apply_stroke_state(pres, cs);
    }

    match (has_fill, has_stroke) {
        (true, true) => cs.push_str("B\n"),   // fill and stroke
        (true, false) => cs.push_str("f\n"),   // fill only
        (false, true) => cs.push_str("S\n"),   // stroke only
        (false, false) => cs.push_str("n\n"),  // no paint, discard path
    }
}

/// Format an f64 for PDF output (compact, no trailing zeros).
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

// ---------------------------------------------------------------------------
// Public export functions
// ---------------------------------------------------------------------------

/// Export an SVG document to PDF format and write it to `path`.
///
/// Supports basic shapes, paths, solid fills, strokes, and affine transforms.
///
/// # Errors
///
/// Returns [`IoError::Io`] if the file cannot be written.
pub fn export_pdf(
    doc: &SvgDocument,
    path: &Path,
    options: &PdfExportOptions,
) -> Result<(), IoError> {
    let bytes = render_pdf(doc, options);
    std::fs::write(path, &bytes)?;
    Ok(())
}

/// Render an SVG document to PDF bytes (without writing to disk).
pub fn render_pdf(doc: &SvgDocument, options: &PdfExportOptions) -> Vec<u8> {
    let mut w = PdfWriter::new();

    let width = options.page_width;
    let height = options.page_height;

    // Reserve object numbers so we can cross-reference.
    let catalog_obj = w.reserve_obj();  // 1
    let pages_obj = w.reserve_obj();    // 2
    let page_obj = w.reserve_obj();     // 3

    // --- Content stream ---
    let stream_content = render_content_stream(doc, height);
    let stream_bytes = stream_content.as_bytes();

    let stream_obj = w.begin_obj();
    let stream_dict = format!(
        "<< /Length {} >>\nstream\n",
        stream_bytes.len()
    );
    w.write_raw(stream_dict.as_bytes());
    w.write_raw(stream_bytes);
    w.write_raw(b"\nendstream\n");
    w.end_obj();

    // --- Page ---
    w.begin_reserved_obj(page_obj);
    let page_dict = format!(
        "<< /Type /Page /Parent {pages_obj} 0 R /MediaBox [0 0 {pw} {ph}] /Contents {stream_obj} 0 R >>\n",
        pw = fmt_f64(width),
        ph = fmt_f64(height),
    );
    w.write_raw(page_dict.as_bytes());
    w.end_obj();

    // --- Pages ---
    w.begin_reserved_obj(pages_obj);
    let pages_dict = format!(
        "<< /Type /Pages /Kids [{page_obj} 0 R] /Count 1 >>\n"
    );
    w.write_raw(pages_dict.as_bytes());
    w.end_obj();

    // --- Info dictionary (optional) ---
    let info_obj = if options.include_metadata {
        let obj = w.begin_obj();
        let mut info = String::from("<< ");
        info.push_str("/Producer (sivgo-io) ");
        info.push_str(&format!("/Creator (Sivgo PDF Export, DPI={}) ", options.dpi));
        info.push_str(">>\n");
        w.write_raw(info.as_bytes());
        w.end_obj();
        Some(obj)
    } else {
        None
    };

    // --- Catalog ---
    w.begin_reserved_obj(catalog_obj);
    let catalog = format!(
        "<< /Type /Catalog /Pages {pages_obj} 0 R >>\n"
    );
    w.write_raw(catalog.as_bytes());
    w.end_obj();

    w.finish(catalog_obj, info_obj)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::*;

    /// Build a minimal test document with a red rectangle.
    fn test_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(80.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
            node.presentation.stroke = Some(Paint::Color(Color::rgb(0, 0, 0)));
            node.presentation.stroke_width = Some(SvgLength::px(2.0));
        }
        doc.append_child(svg_id, rect_id);
        doc
    }

    #[test]
    fn pdf_header_and_trailer() {
        let doc = test_doc();
        let pdf = render_pdf(&doc, &PdfExportOptions::default());

        assert!(
            pdf.starts_with(b"%PDF-1.4"),
            "PDF should start with %PDF-1.4"
        );
        let tail = String::from_utf8_lossy(&pdf[pdf.len().saturating_sub(20)..]);
        assert!(
            tail.contains("%%EOF"),
            "PDF should end with %%EOF, got: {tail}"
        );
    }

    #[test]
    fn pdf_contains_rect_operator() {
        let doc = test_doc();
        let pdf = render_pdf(&doc, &PdfExportOptions::default());
        let text = String::from_utf8_lossy(&pdf);

        assert!(
            text.contains(" re\n"),
            "PDF should contain a rect operator (re)"
        );
    }

    #[test]
    fn pdf_contains_fill_color() {
        let doc = test_doc();
        let pdf = render_pdf(&doc, &PdfExportOptions::default());
        let text = String::from_utf8_lossy(&pdf);

        // Red fill uses rg operator.
        assert!(
            text.contains("1 0 0 rg"),
            "PDF should contain red fill color"
        );
    }

    #[test]
    fn pdf_contains_stroke() {
        let doc = test_doc();
        let pdf = render_pdf(&doc, &PdfExportOptions::default());
        let text = String::from_utf8_lossy(&pdf);

        // Stroke color operator (RG) and B (fill+stroke) operator.
        assert!(
            text.contains("RG"),
            "PDF should contain stroke color operator"
        );
        assert!(
            text.contains("B\n"),
            "PDF should contain fill-and-stroke operator"
        );
    }

    #[test]
    fn pdf_with_metadata() {
        let doc = test_doc();
        let opts = PdfExportOptions {
            include_metadata: true,
            ..PdfExportOptions::default()
        };
        let pdf = render_pdf(&doc, &opts);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("/Producer (sivgo-io)"), "PDF should contain Producer");
    }

    #[test]
    fn pdf_without_metadata() {
        let doc = test_doc();
        let opts = PdfExportOptions {
            include_metadata: false,
            ..PdfExportOptions::default()
        };
        let pdf = render_pdf(&doc, &opts);
        let text = String::from_utf8_lossy(&pdf);

        assert!(!text.contains("/Producer"), "PDF without metadata should not have Producer");
    }

    #[test]
    fn pdf_write_to_file() {
        let doc = test_doc();
        let dir = std::env::temp_dir().join("sivgo_pdf_export_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.pdf");

        export_pdf(&doc, &file, &PdfExportOptions::default()).unwrap();
        let data = std::fs::read(&file).unwrap();
        assert!(data.starts_with(b"%PDF-1.4"));

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn pdf_path_and_circle() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            ..SvgSvgAttrs::default()
        }));
        doc.set_root(svg_id);

        // Circle
        let circle_id = doc.add_node(SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(100.0),
            cy: SvgLength::px(100.0),
            r: SvgLength::px(50.0),
        }));
        if let Some(node) = doc.get_node_mut(circle_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLUE));
        }
        doc.append_child(svg_id, circle_id);

        // Path
        let path_data = sivgo_math::BezPath::from_svg_path("M 10 20 L 30 40 C 50 60 70 80 90 100 Z").unwrap();
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: path_data,
            ..PathAttrs::default()
        }));
        doc.append_child(svg_id, path_id);

        let pdf = render_pdf(&doc, &PdfExportOptions::default());
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains(" c\n"), "PDF should contain cubic bezier operators for circle");
        assert!(text.contains(" m\n"), "PDF should contain moveto operator");
    }

    #[test]
    fn pdf_custom_page_size() {
        let doc = test_doc();
        let opts = PdfExportOptions {
            page_width: 612.0,  // US Letter
            page_height: 792.0,
            ..PdfExportOptions::default()
        };
        let pdf = render_pdf(&doc, &opts);
        let text = String::from_utf8_lossy(&pdf);

        assert!(
            text.contains("612") && text.contains("792"),
            "PDF should use custom page dimensions"
        );
    }
}
