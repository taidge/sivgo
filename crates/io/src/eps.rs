//! EPS (Encapsulated PostScript) export for SVG documents (10.9).
//!
//! Generates EPS level 3 files with proper headers, bounding box, and
//! PostScript path operators for SVG shapes.

use std::path::Path;

use sivgo_dom::{Color, NodeId, Paint, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::PathSegment;

use crate::IoError;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Options controlling EPS export.
#[derive(Debug, Clone)]
pub struct EpsExportOptions {
    /// Resolution in dots per inch (used for coordinate scaling).
    pub dpi: f64,
    /// Whether to include a TIFF/EPSI preview image (reserved for future use).
    pub include_preview: bool,
}

impl Default for EpsExportOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            include_preview: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Public export function
// ---------------------------------------------------------------------------

/// Export an SVG document to Encapsulated PostScript (EPS) format and write
/// to `path`.
///
/// The output conforms to EPSF-3.0 with a `%%BoundingBox` comment.
/// Shapes are rendered using PostScript path operators with support for
/// solid fills, strokes, and affine transforms.
///
/// # Arguments
///
/// * `doc`     - The SVG document to export.
/// * `path`    - Output file path.
/// * `options` - Export options (DPI, preview).
///
/// # Errors
///
/// Returns [`IoError::Io`] if the file cannot be written.
pub fn export_eps(
    doc: &SvgDocument,
    path: &Path,
    options: &EpsExportOptions,
) -> Result<(), IoError> {
    let eps = render_eps(doc, options);
    std::fs::write(path, eps.as_bytes())?;
    Ok(())
}

/// Render an SVG document to an EPS string (without writing to disk).
pub fn render_eps(doc: &SvgDocument, options: &EpsExportOptions) -> String {
    // Determine bounding box from the SVG root element.
    let (width, height) = get_svg_dimensions(doc, options.dpi);

    let mut out = String::new();

    // EPS header.
    out.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    out.push_str(&format!(
        "%%BoundingBox: 0 0 {} {}\n",
        width.ceil() as i64,
        height.ceil() as i64
    ));
    out.push_str(&format!(
        "%%HiResBoundingBox: 0 0 {:.4} {:.4}\n",
        width, height
    ));
    out.push_str("%%Creator: sivgo-io\n");
    out.push_str("%%DocumentData: Clean7Bit\n");
    out.push_str("%%EndComments\n\n");

    if options.include_preview {
        out.push_str("%%BeginPreview: (preview not yet implemented)\n");
        out.push_str("%%EndPreview\n\n");
    }

    // Flip coordinate system: SVG origin is top-left, PS is bottom-left.
    out.push_str(&format!("0 {} translate\n", fmt_f64(height)));
    out.push_str("1 -1 scale\n\n");

    // Render content.
    if let Some(root) = doc.root() {
        render_node(doc, root, &mut out);
    }

    out.push_str("\nshowpage\n");
    out.push_str("%%EOF\n");

    out
}

/// Get the SVG document dimensions in points, falling back to defaults.
fn get_svg_dimensions(doc: &SvgDocument, _dpi: f64) -> (f64, f64) {
    if let Some(root_id) = doc.root() {
        if let Some(root) = doc.get_node(root_id) {
            if let SvgElement::Svg(a) = &root.element {
                let w = a.width.map(|l| l.to_px()).unwrap_or(100.0);
                let h = a.height.map(|l| l.to_px()).unwrap_or(100.0);
                return (w, h);
            }
        }
    }
    (100.0, 100.0)
}

// ---------------------------------------------------------------------------
// Internal rendering
// ---------------------------------------------------------------------------

/// Recursively render a node and its children into PostScript operators.
fn render_node(doc: &SvgDocument, id: NodeId, out: &mut String) {
    let node = match doc.get_node(id) {
        Some(n) => n,
        None => return,
    };

    let pres = &node.presentation;

    // Apply transform via gsave/concat/grestore.
    let has_transform = pres.transform.is_some();
    if has_transform {
        out.push_str("gsave\n");
        if let Some(ref t) = pres.transform {
            let c = t.as_coeffs();
            out.push_str(&format!(
                "[{} {} {} {} {} {}] concat\n",
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
            let x = a.x.to_px();
            let y = a.y.to_px();
            let w = a.width.to_px();
            let h = a.height.to_px();
            if w > 0.0 && h > 0.0 {
                out.push_str("newpath\n");
                out.push_str(&format!("{} {} moveto\n", fmt_f64(x), fmt_f64(y)));
                out.push_str(&format!("{} {} lineto\n", fmt_f64(x + w), fmt_f64(y)));
                out.push_str(&format!("{} {} lineto\n", fmt_f64(x + w), fmt_f64(y + h)));
                out.push_str(&format!("{} {} lineto\n", fmt_f64(x), fmt_f64(y + h)));
                out.push_str("closepath\n");
                apply_fill_stroke(pres, out);
            }
        }
        SvgElement::Circle(a) => {
            let cx = a.cx.to_px();
            let cy = a.cy.to_px();
            let r = a.r.to_px();
            if r > 0.0 {
                out.push_str("newpath\n");
                emit_ellipse_path(cx, cy, r, r, out);
                apply_fill_stroke(pres, out);
            }
        }
        SvgElement::Ellipse(a) => {
            let cx = a.cx.to_px();
            let cy = a.cy.to_px();
            let rx = a.rx.to_px();
            let ry = a.ry.to_px();
            if rx > 0.0 && ry > 0.0 {
                out.push_str("newpath\n");
                emit_ellipse_path(cx, cy, rx, ry, out);
                apply_fill_stroke(pres, out);
            }
        }
        SvgElement::Line(a) => {
            out.push_str("newpath\n");
            out.push_str(&format!(
                "{} {} moveto\n",
                fmt_f64(a.x1.to_px()),
                fmt_f64(a.y1.to_px())
            ));
            out.push_str(&format!(
                "{} {} lineto\n",
                fmt_f64(a.x2.to_px()),
                fmt_f64(a.y2.to_px())
            ));
            apply_stroke_only(pres, out);
        }
        SvgElement::Path(a) => {
            out.push_str("newpath\n");
            emit_bezpath(&a.d, out);
            apply_fill_stroke(pres, out);
        }
        SvgElement::Polyline(a) => {
            if let Some((&first, rest)) = a.points.split_first() {
                out.push_str("newpath\n");
                out.push_str(&format!(
                    "{} {} moveto\n",
                    fmt_f64(first.0),
                    fmt_f64(first.1)
                ));
                for pt in rest {
                    out.push_str(&format!("{} {} lineto\n", fmt_f64(pt.0), fmt_f64(pt.1)));
                }
                apply_fill_stroke(pres, out);
            }
        }
        SvgElement::Polygon(a) => {
            if let Some((&first, rest)) = a.points.split_first() {
                out.push_str("newpath\n");
                out.push_str(&format!(
                    "{} {} moveto\n",
                    fmt_f64(first.0),
                    fmt_f64(first.1)
                ));
                for pt in rest {
                    out.push_str(&format!("{} {} lineto\n", fmt_f64(pt.0), fmt_f64(pt.1)));
                }
                out.push_str("closepath\n");
                apply_fill_stroke(pres, out);
            }
        }
        // Containers: recurse.
        SvgElement::Svg(_) | SvgElement::Group(_) => {
            for &child in doc.children(id) {
                render_node(doc, child, out);
            }
        }
        _ => {}
    }

    if has_transform {
        out.push_str("grestore\n");
    }
}

/// Emit PostScript path operators for a BezPath.
fn emit_bezpath(path: &sivgo_math::BezPath, out: &mut String) {
    for seg in path.elements() {
        match seg {
            PathSegment::MoveTo(p) => {
                out.push_str(&format!("{} {} moveto\n", fmt_f64(p.x), fmt_f64(p.y)));
            }
            PathSegment::LineTo(p) => {
                out.push_str(&format!("{} {} lineto\n", fmt_f64(p.x), fmt_f64(p.y)));
            }
            PathSegment::QuadTo(p1, p2) => {
                // PostScript has no native quad; use curveto with duplicate CPs.
                out.push_str(&format!(
                    "{} {} {} {} {} {} curveto\n",
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p2.x),
                    fmt_f64(p2.y)
                ));
            }
            PathSegment::CurveTo(p1, p2, p3) => {
                out.push_str(&format!(
                    "{} {} {} {} {} {} curveto\n",
                    fmt_f64(p1.x),
                    fmt_f64(p1.y),
                    fmt_f64(p2.x),
                    fmt_f64(p2.y),
                    fmt_f64(p3.x),
                    fmt_f64(p3.y)
                ));
            }
            PathSegment::ClosePath => {
                out.push_str("closepath\n");
            }
        }
    }
}

/// Approximate an ellipse with four cubic Bezier arcs (PostScript curveto).
fn emit_ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64, out: &mut String) {
    const KAPPA: f64 = 0.5522847498;
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;

    out.push_str(&format!("{} {} moveto\n", fmt_f64(cx + rx), fmt_f64(cy)));
    out.push_str(&format!(
        "{} {} {} {} {} {} curveto\n",
        fmt_f64(cx + rx),
        fmt_f64(cy + ky),
        fmt_f64(cx + kx),
        fmt_f64(cy + ry),
        fmt_f64(cx),
        fmt_f64(cy + ry)
    ));
    out.push_str(&format!(
        "{} {} {} {} {} {} curveto\n",
        fmt_f64(cx - kx),
        fmt_f64(cy + ry),
        fmt_f64(cx - rx),
        fmt_f64(cy + ky),
        fmt_f64(cx - rx),
        fmt_f64(cy)
    ));
    out.push_str(&format!(
        "{} {} {} {} {} {} curveto\n",
        fmt_f64(cx - rx),
        fmt_f64(cy - ky),
        fmt_f64(cx - kx),
        fmt_f64(cy - ry),
        fmt_f64(cx),
        fmt_f64(cy - ry)
    ));
    out.push_str(&format!(
        "{} {} {} {} {} {} curveto\n",
        fmt_f64(cx + kx),
        fmt_f64(cy - ry),
        fmt_f64(cx + rx),
        fmt_f64(cy - ky),
        fmt_f64(cx + rx),
        fmt_f64(cy)
    ));
    out.push_str("closepath\n");
}

/// Set fill color and emit fill, then optionally stroke.
fn apply_fill_stroke(pres: &PresentationAttrs, out: &mut String) {
    let has_fill = match &pres.fill {
        Some(Paint::Color(c)) => {
            set_color(c, out);
            true
        }
        Some(Paint::None) => false,
        // SVG default fill is black.
        None => {
            out.push_str("0 0 0 setrgbcolor\n");
            true
        }
        _ => false,
    };

    let has_stroke = matches!(&pres.stroke, Some(Paint::Color(_)));

    if has_fill && has_stroke {
        out.push_str("gsave fill grestore\n");
        apply_stroke_only(pres, out);
    } else if has_fill {
        out.push_str("fill\n");
    } else if has_stroke {
        apply_stroke_only(pres, out);
    }
}

/// Set stroke state and stroke the current path.
fn apply_stroke_only(pres: &PresentationAttrs, out: &mut String) {
    if let Some(Paint::Color(c)) = &pres.stroke {
        set_color(c, out);
    }
    if let Some(ref sw) = pres.stroke_width {
        out.push_str(&format!("{} setlinewidth\n", fmt_f64(sw.to_px())));
    }
    out.push_str("stroke\n");
}

fn set_color(c: &Color, out: &mut String) {
    out.push_str(&format!(
        "{} {} {} setrgbcolor\n",
        fmt_f64(c.r as f64 / 255.0),
        fmt_f64(c.g as f64 / 255.0),
        fmt_f64(c.b as f64 / 255.0)
    ));
}

/// Format an f64 for PS output.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::*;

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
        }
        doc.append_child(svg_id, rect_id);
        doc
    }

    #[test]
    fn eps_header_and_bounding_box() {
        let doc = test_doc();
        let eps = render_eps(&doc, &EpsExportOptions::default());

        assert!(
            eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"),
            "EPS should start with proper header"
        );
        assert!(
            eps.contains("%%BoundingBox: 0 0 100 100"),
            "EPS should contain BoundingBox"
        );
        assert!(eps.contains("%%EOF"), "EPS should end with %%EOF");
    }

    #[test]
    fn eps_contains_rect_operators() {
        let doc = test_doc();
        let eps = render_eps(&doc, &EpsExportOptions::default());

        assert!(eps.contains("moveto"), "EPS should contain moveto");
        assert!(eps.contains("lineto"), "EPS should contain lineto");
        assert!(eps.contains("closepath"), "EPS should contain closepath");
        assert!(eps.contains("fill"), "EPS should contain fill");
    }

    #[test]
    fn eps_contains_color() {
        let doc = test_doc();
        let eps = render_eps(&doc, &EpsExportOptions::default());

        assert!(
            eps.contains("1 0 0 setrgbcolor"),
            "EPS should set red color, got:\n{}",
            eps
        );
    }

    #[test]
    fn eps_with_stroke() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(0.0),
            y1: SvgLength::px(0.0),
            x2: SvgLength::px(100.0),
            y2: SvgLength::px(100.0),
        }));
        if let Some(node) = doc.get_node_mut(line_id) {
            node.presentation.stroke = Some(Paint::Color(Color::rgb(0, 0, 0)));
            node.presentation.stroke_width = Some(SvgLength::px(3.0));
        }
        doc.append_child(svg_id, line_id);

        let eps = render_eps(&doc, &EpsExportOptions::default());
        assert!(eps.contains("setlinewidth"), "EPS should set line width");
        assert!(eps.contains("stroke"), "EPS should contain stroke operator");
    }

    #[test]
    fn eps_write_to_file() {
        let doc = test_doc();
        let dir = std::env::temp_dir().join("sivgo_eps_export_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.eps");

        export_eps(&doc, &file, &EpsExportOptions::default()).unwrap();
        let data = std::fs::read_to_string(&file).unwrap();
        assert!(data.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn eps_path_operators() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let path_data =
            sivgo_math::BezPath::from_svg_path("M 10 20 L 30 40 C 50 60 70 80 90 100 Z").unwrap();
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: path_data,
            ..PathAttrs::default()
        }));
        doc.append_child(svg_id, path_id);

        let eps = render_eps(&doc, &EpsExportOptions::default());

        assert!(eps.contains("moveto"), "EPS should contain moveto for path");
        assert!(eps.contains("lineto"), "EPS should contain lineto for path");
        assert!(
            eps.contains("curveto"),
            "EPS should contain curveto for path"
        );
        assert!(
            eps.contains("closepath"),
            "EPS should contain closepath for path"
        );
    }

    #[test]
    fn eps_hi_res_bounding_box() {
        let doc = test_doc();
        let eps = render_eps(&doc, &EpsExportOptions::default());

        assert!(
            eps.contains("%%HiResBoundingBox:"),
            "EPS should contain HiResBoundingBox"
        );
    }
}
