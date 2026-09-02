//! DXF (AutoCAD Drawing Exchange Format) export for SVG documents (10.10).
//!
//! Generates ASCII DXF files with HEADER, ENTITIES, and EOF sections.
//! SVG shapes are converted to DXF entities: LINE, LWPOLYLINE, CIRCLE,
//! and paths are decomposed into polylines.

use std::path::Path;

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::PathSegment;

use crate::IoError;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Options controlling DXF export.
#[derive(Debug, Clone)]
pub struct DxfExportOptions {
    /// Scale factor applied to all coordinates.
    pub scale: f64,
    /// When `true`, each SVG `<g>` element creates a separate DXF layer.
    pub layer_per_group: bool,
}

impl Default for DxfExportOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            layer_per_group: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Public export function
// ---------------------------------------------------------------------------

/// Export an SVG document to AutoCAD DXF (ASCII) format and write to `path`.
///
/// Shapes are placed in model space.  The coordinate system maps from SVG user
/// units with Y inverted (DXF uses mathematical Y-up, SVG uses Y-down).
///
/// Supported SVG elements:
///
/// * `<line>` -> DXF LINE
/// * `<circle>` -> DXF CIRCLE
/// * `<rect>`, `<polyline>`, `<polygon>` -> DXF LWPOLYLINE
/// * `<ellipse>` -> DXF LWPOLYLINE (approximated)
/// * `<path>` -> DXF LWPOLYLINE (linearized)
///
/// # Errors
///
/// Returns [`IoError::Io`] if the file cannot be written.
pub fn export_dxf(
    doc: &SvgDocument,
    path: &Path,
    options: &DxfExportOptions,
) -> Result<(), IoError> {
    let dxf = render_dxf(doc, options);
    std::fs::write(path, dxf.as_bytes())?;
    Ok(())
}

/// Render an SVG document to a DXF string (without writing to disk).
pub fn render_dxf(doc: &SvgDocument, options: &DxfExportOptions) -> String {
    let mut out = String::new();

    write_header(&mut out);
    write_entities_start(&mut out);

    if let Some(root) = doc.root() {
        render_node(doc, root, &mut out, "0", options);
    }

    write_entities_end(&mut out);
    write_eof(&mut out);

    out
}

// ---------------------------------------------------------------------------
// DXF structure helpers
// ---------------------------------------------------------------------------

fn write_header(out: &mut String) {
    out.push_str("  0\nSECTION\n");
    out.push_str("  2\nHEADER\n");
    // ACADVER - AutoCAD version (AC1015 = AutoCAD 2000).
    out.push_str("  9\n$ACADVER\n  1\nAC1015\n");
    out.push_str("  0\nENDSEC\n");
}

fn write_entities_start(out: &mut String) {
    out.push_str("  0\nSECTION\n");
    out.push_str("  2\nENTITIES\n");
}

fn write_entities_end(out: &mut String) {
    out.push_str("  0\nENDSEC\n");
}

fn write_eof(out: &mut String) {
    out.push_str("  0\nEOF\n");
}

// ---------------------------------------------------------------------------
// DXF entity writers
// ---------------------------------------------------------------------------

/// Write a DXF LINE entity.
fn write_line(x1: f64, y1: f64, x2: f64, y2: f64, layer: &str, scale: f64, out: &mut String) {
    out.push_str("  0\nLINE\n");
    out.push_str(&format!("  8\n{}\n", layer));
    out.push_str(&format!(" 10\n{}\n", fmt_f64(x1 * scale)));
    out.push_str(&format!(" 20\n{}\n", fmt_f64(-y1 * scale))); // flip Y
    out.push_str(" 30\n0.0\n");
    out.push_str(&format!(" 11\n{}\n", fmt_f64(x2 * scale)));
    out.push_str(&format!(" 21\n{}\n", fmt_f64(-y2 * scale))); // flip Y
    out.push_str(" 31\n0.0\n");
}

/// Write a DXF CIRCLE entity.
fn write_circle(cx: f64, cy: f64, r: f64, layer: &str, scale: f64, out: &mut String) {
    out.push_str("  0\nCIRCLE\n");
    out.push_str(&format!("  8\n{}\n", layer));
    out.push_str(&format!(" 10\n{}\n", fmt_f64(cx * scale)));
    out.push_str(&format!(" 20\n{}\n", fmt_f64(-cy * scale))); // flip Y
    out.push_str(" 30\n0.0\n");
    out.push_str(&format!(" 40\n{}\n", fmt_f64(r * scale)));
}

/// Write a DXF LWPOLYLINE entity from a list of 2D points.
fn write_lwpolyline(
    points: &[(f64, f64)],
    closed: bool,
    layer: &str,
    scale: f64,
    out: &mut String,
) {
    if points.is_empty() {
        return;
    }
    out.push_str("  0\nLWPOLYLINE\n");
    out.push_str(&format!("  8\n{}\n", layer));
    out.push_str(&format!(" 90\n{}\n", points.len())); // vertex count
    out.push_str(&format!(" 70\n{}\n", if closed { 1 } else { 0 })); // flags
    for &(x, y) in points {
        out.push_str(&format!(" 10\n{}\n", fmt_f64(x * scale)));
        out.push_str(&format!(" 20\n{}\n", fmt_f64(-y * scale))); // flip Y
    }
}

// ---------------------------------------------------------------------------
// Node rendering
// ---------------------------------------------------------------------------

/// Recursively render SVG nodes as DXF entities.
fn render_node(
    doc: &SvgDocument,
    id: NodeId,
    out: &mut String,
    layer: &str,
    options: &DxfExportOptions,
) {
    let node = match doc.get_node(id) {
        Some(n) => n,
        None => return,
    };

    let scale = options.scale;

    match &node.element {
        SvgElement::Rect(a) => {
            let x = a.x.to_px();
            let y = a.y.to_px();
            let w = a.width.to_px();
            let h = a.height.to_px();
            if w > 0.0 && h > 0.0 {
                let points = vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
                write_lwpolyline(&points, true, layer, scale, out);
            }
        }
        SvgElement::Circle(a) => {
            let cx = a.cx.to_px();
            let cy = a.cy.to_px();
            let r = a.r.to_px();
            if r > 0.0 {
                write_circle(cx, cy, r, layer, scale, out);
            }
        }
        SvgElement::Ellipse(a) => {
            // Approximate ellipse as a polyline (32 segments).
            let cx = a.cx.to_px();
            let cy = a.cy.to_px();
            let rx = a.rx.to_px();
            let ry = a.ry.to_px();
            if rx > 0.0 && ry > 0.0 {
                let segments = 32;
                let points: Vec<(f64, f64)> = (0..segments)
                    .map(|i| {
                        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (segments as f64);
                        (cx + rx * angle.cos(), cy + ry * angle.sin())
                    })
                    .collect();
                write_lwpolyline(&points, true, layer, scale, out);
            }
        }
        SvgElement::Line(a) => {
            write_line(
                a.x1.to_px(),
                a.y1.to_px(),
                a.x2.to_px(),
                a.y2.to_px(),
                layer,
                scale,
                out,
            );
        }
        SvgElement::Polyline(a) => {
            if !a.points.is_empty() {
                write_lwpolyline(&a.points, false, layer, scale, out);
            }
        }
        SvgElement::Polygon(a) => {
            if !a.points.is_empty() {
                write_lwpolyline(&a.points, true, layer, scale, out);
            }
        }
        SvgElement::Path(a) => {
            let polylines = linearize_path(&a.d);
            for (pts, closed) in polylines {
                if !pts.is_empty() {
                    write_lwpolyline(&pts, closed, layer, scale, out);
                }
            }
        }
        // Groups: optionally create a new layer.
        SvgElement::Group(_) => {
            let child_layer = if options.layer_per_group {
                // Use the node's xml_id or a generated name for the layer.
                node.xml_id.as_deref().unwrap_or(layer)
            } else {
                layer
            };
            for &child in doc.children(id) {
                render_node(doc, child, out, child_layer, options);
            }
        }
        SvgElement::Svg(_) => {
            for &child in doc.children(id) {
                render_node(doc, child, out, layer, options);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Path linearization
// ---------------------------------------------------------------------------

/// Convert a BezPath to a sequence of polylines.
///
/// Returns a vector of `(points, is_closed)` tuples, one per subpath.
fn linearize_path(path: &sivgo_math::BezPath) -> Vec<(Vec<(f64, f64)>, bool)> {
    let mut result: Vec<(Vec<(f64, f64)>, bool)> = Vec::new();
    let mut current_pts: Vec<(f64, f64)> = Vec::new();
    let mut closed = false;

    for seg in path.elements() {
        match seg {
            PathSegment::MoveTo(p) => {
                if !current_pts.is_empty() {
                    result.push((std::mem::take(&mut current_pts), closed));
                    closed = false;
                }
                current_pts.push((p.x, p.y));
            }
            PathSegment::LineTo(p) => {
                current_pts.push((p.x, p.y));
            }
            PathSegment::QuadTo(p1, p2) => {
                if let Some(&last) = current_pts.last() {
                    subdivide_quad(last, (p1.x, p1.y), (p2.x, p2.y), &mut current_pts, 8);
                }
            }
            PathSegment::CurveTo(p1, p2, p3) => {
                if let Some(&last) = current_pts.last() {
                    subdivide_cubic(
                        last,
                        (p1.x, p1.y),
                        (p2.x, p2.y),
                        (p3.x, p3.y),
                        &mut current_pts,
                        16,
                    );
                }
            }
            PathSegment::ClosePath => {
                closed = true;
            }
        }
    }

    if !current_pts.is_empty() {
        result.push((current_pts, closed));
    }

    result
}

/// Subdivide a quadratic Bezier into `n` line segments.
fn subdivide_quad(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    pts: &mut Vec<(f64, f64)>,
    n: usize,
) {
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let mt = 1.0 - t;
        let x = mt * mt * p0.0 + 2.0 * mt * t * p1.0 + t * t * p2.0;
        let y = mt * mt * p0.1 + 2.0 * mt * t * p1.1 + t * t * p2.1;
        pts.push((x, y));
    }
}

/// Subdivide a cubic Bezier into `n` line segments.
fn subdivide_cubic(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    pts: &mut Vec<(f64, f64)>,
    n: usize,
) {
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let mt = 1.0 - t;
        let x = mt * mt * mt * p0.0
            + 3.0 * mt * mt * t * p1.0
            + 3.0 * mt * t * t * p2.0
            + t * t * t * p3.0;
        let y = mt * mt * mt * p0.1
            + 3.0 * mt * mt * t * p1.1
            + 3.0 * mt * t * t * p2.1
            + t * t * t * p3.1;
        pts.push((x, y));
    }
}

/// Format a float for DXF output.
fn fmt_f64(v: f64) -> String {
    if v == 0.0 {
        "0.0".to_string()
    } else if v == v.round() && v.abs() < 1e12 {
        format!("{}.0", v as i64)
    } else {
        format!("{:.6}", v).trim_end_matches('0').to_string()
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
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            ..SvgSvgAttrs::default()
        }));
        doc.set_root(svg_id);

        // Rect
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(20.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(60.0),
            ..RectAttrs::default()
        }));
        doc.append_child(svg_id, rect_id);

        // Circle
        let circle_id = doc.add_node(SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(100.0),
            cy: SvgLength::px(100.0),
            r: SvgLength::px(30.0),
        }));
        doc.append_child(svg_id, circle_id);

        // Line
        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(0.0),
            y1: SvgLength::px(0.0),
            x2: SvgLength::px(200.0),
            y2: SvgLength::px(200.0),
        }));
        doc.append_child(svg_id, line_id);

        doc
    }

    #[test]
    fn dxf_structure() {
        let doc = test_doc();
        let dxf = render_dxf(&doc, &DxfExportOptions::default());

        assert!(dxf.contains("HEADER"), "DXF should contain HEADER section");
        assert!(
            dxf.contains("ENTITIES"),
            "DXF should contain ENTITIES section"
        );
        assert!(dxf.trim_end().ends_with("EOF"), "DXF should end with EOF");
    }

    #[test]
    fn dxf_contains_expected_entities() {
        let doc = test_doc();
        let dxf = render_dxf(&doc, &DxfExportOptions::default());

        assert!(
            dxf.contains("LWPOLYLINE"),
            "DXF should contain LWPOLYLINE for rect"
        );
        assert!(dxf.contains("CIRCLE"), "DXF should contain CIRCLE entity");
        assert!(dxf.contains("LINE"), "DXF should contain LINE entity");
    }

    #[test]
    fn dxf_path_linearization() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let path_data =
            sivgo_math::BezPath::from_svg_path("M 0 0 C 10 20 30 40 50 50 L 80 10 Z").unwrap();
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: path_data,
            ..PathAttrs::default()
        }));
        doc.append_child(svg_id, path_id);

        let dxf = render_dxf(&doc, &DxfExportOptions::default());

        assert!(
            dxf.contains("LWPOLYLINE"),
            "DXF should convert path to LWPOLYLINE"
        );
    }

    #[test]
    fn dxf_scale_option() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(0.0),
            y1: SvgLength::px(0.0),
            x2: SvgLength::px(100.0),
            y2: SvgLength::px(100.0),
        }));
        doc.append_child(svg_id, line_id);

        let opts = DxfExportOptions {
            scale: 2.0,
            layer_per_group: false,
        };
        let dxf = render_dxf(&doc, &opts);

        // The endpoint (100, 100) scaled by 2.0 should be 200.0.
        assert!(
            dxf.contains("200.0"),
            "DXF should contain scaled coordinate 200.0"
        );
    }

    #[test]
    fn dxf_layer_per_group() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        if let Some(node) = doc.get_node_mut(g_id) {
            node.xml_id = Some("my_layer".to_string());
        }
        doc.append_child(svg_id, g_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(50.0),
            height: SvgLength::px(50.0),
            ..RectAttrs::default()
        }));
        doc.append_child(g_id, rect_id);

        let opts = DxfExportOptions {
            scale: 1.0,
            layer_per_group: true,
        };
        let dxf = render_dxf(&doc, &opts);

        assert!(
            dxf.contains("my_layer"),
            "DXF with layer_per_group should use group id as layer name"
        );
    }

    #[test]
    fn dxf_empty_document() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let dxf = render_dxf(&doc, &DxfExportOptions::default());

        assert!(dxf.contains("HEADER"), "Empty doc should still have HEADER");
        assert!(
            dxf.contains("ENTITIES"),
            "Empty doc should still have ENTITIES"
        );
        assert!(
            dxf.trim_end().ends_with("EOF"),
            "Empty doc should end with EOF"
        );
    }

    #[test]
    fn dxf_write_to_file() {
        let doc = test_doc();
        let dir = std::env::temp_dir().join("sivgo_dxf_export_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.dxf");

        export_dxf(&doc, &file, &DxfExportOptions::default()).unwrap();
        let data = std::fs::read_to_string(&file).unwrap();
        assert!(data.contains("HEADER"));
        assert!(data.contains("EOF"));

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }
}
