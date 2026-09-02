//! Vector export formats must honour SVG length units and produce output the
//! target format's readers accept.
//!
//! The exporters used to read a length's raw number and ignore its unit, so
//! `width="10mm"` became 10 user units instead of about 37.8. They also had no
//! test coverage beyond "does not panic".

use sivgo_dom::parse_svg;
use sivgo_io::{
    export_pdf, render_dxf, render_eps, DxfExportOptions, EpsExportOptions, PdfExportOptions,
};

fn doc(body: &str) -> sivgo_dom::SvgDocument {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">{body}</svg>"##
    );
    parse_svg(&svg).expect("parse")
}

/// Pull every number out of the output so a coordinate can be looked for
/// without depending on the exact formatting around it.
fn numbers(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || (ch == '-' && current.is_empty()) {
            current.push(ch);
        } else {
            if let Ok(v) = current.parse::<f64>() {
                out.push(v);
            }
            current.clear();
        }
    }
    if let Ok(v) = current.parse::<f64>() {
        out.push(v);
    }
    out
}

#[track_caller]
fn assert_has_number(text: &str, expected: f64, what: &str) {
    let found = numbers(text)
        .into_iter()
        .any(|v| (v - expected).abs() < 0.5);
    assert!(found, "{what}: expected a coordinate near {expected}");
}

#[test]
fn dxf_output_has_the_required_structure() {
    let out = render_dxf(
        &doc(r##"<rect x="10" y="20" width="30" height="40"/>"##),
        &DxfExportOptions::default(),
    );
    for marker in ["SECTION", "HEADER", "ENTITIES", "ENDSEC", "EOF"] {
        assert!(out.contains(marker), "DXF must contain {marker}");
    }
    assert!(out.contains("LWPOLYLINE"), "a rect becomes a polyline");
}

#[test]
fn dxf_converts_millimetres_to_user_units() {
    // 10mm at 96dpi is about 37.8 user units.
    let out = render_dxf(
        &doc(r##"<circle cx="10mm" cy="10mm" r="10mm"/>"##),
        &DxfExportOptions::default(),
    );
    assert!(out.contains("CIRCLE"));
    assert_has_number(&out, 37.795, "millimetre radius");
}

#[test]
fn dxf_flips_the_y_axis() {
    // DXF is Y-up and SVG is Y-down, so a positive SVG y becomes negative.
    let out = render_dxf(
        &doc(r##"<circle cx="10" cy="25" r="5"/>"##),
        &DxfExportOptions::default(),
    );
    assert_has_number(&out, -25.0, "flipped centre");
}

#[test]
fn eps_output_has_the_required_structure() {
    let out = render_eps(
        &doc(r##"<rect x="10" y="20" width="30" height="40" fill="#ff0000"/>"##),
        &EpsExportOptions::default(),
    );
    assert!(out.starts_with("%!PS-Adobe"), "EPS needs its magic header");
    assert!(out.contains("%%BoundingBox"), "EPS needs a bounding box");
    assert!(out.contains("%%EOF"), "EPS needs a trailer");
}

#[test]
fn eps_converts_millimetres_to_user_units() {
    let out = render_eps(
        &doc(r##"<rect x="10mm" y="0" width="10" height="10"/>"##),
        &EpsExportOptions::default(),
    );
    assert_has_number(&out, 37.795, "millimetre x position");
}

#[test]
fn pdf_output_has_the_required_structure() {
    let out = export_pdf(
        &doc(r##"<rect x="10" y="20" width="30" height="40"/>"##),
        &PdfExportOptions::default(),
    );
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("%PDF-"), "PDF needs its magic header");
    assert!(text.contains("/Type /Catalog"), "PDF needs a catalog");
    assert!(text.contains("trailer"), "PDF needs a trailer");
    assert!(
        text.trim_end().ends_with("%%EOF"),
        "PDF needs a trailer marker"
    );
}

#[test]
fn pdf_converts_millimetres_to_user_units() {
    let out = export_pdf(
        &doc(r##"<rect x="10mm" y="0" width="10" height="10"/>"##),
        &PdfExportOptions::default(),
    );
    let text = String::from_utf8_lossy(&out);
    assert_has_number(&text, 37.795, "millimetre x position");
}

#[test]
fn every_exporter_handles_an_empty_document() {
    let empty = parse_svg(r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"##)
        .expect("parse");

    assert!(render_dxf(&empty, &DxfExportOptions::default()).contains("EOF"));
    assert!(render_eps(&empty, &EpsExportOptions::default()).contains("%%EOF"));
    assert!(!export_pdf(&empty, &PdfExportOptions::default()).is_empty());
}

#[test]
fn every_exporter_handles_all_basic_shapes() {
    let all = doc(
        r##"<rect width="10" height="10"/><circle cx="5" cy="5" r="3"/><ellipse cx="5" cy="5" rx="3" ry="2"/><line x1="0" y1="0" x2="9" y2="9"/><polyline points="0,0 5,5 9,0"/><polygon points="0,0 5,5 9,0"/><path d="M 0 0 C 1 1 2 2 3 3 Z"/>"##,
    );

    let dxf = render_dxf(&all, &DxfExportOptions::default());
    assert!(dxf.contains("CIRCLE") && dxf.contains("LINE") && dxf.contains("LWPOLYLINE"));

    let eps = render_eps(&all, &EpsExportOptions::default());
    assert!(eps.contains("%%BoundingBox") && eps.contains("%%EOF"));

    let pdf = export_pdf(&all, &PdfExportOptions::default());
    assert!(String::from_utf8_lossy(&pdf).contains("%%EOF"));
}

#[test]
fn plain_svg_export_strips_editor_specific_markup() {
    // The desktop menu item used to call the ordinary serializer, so "plain"
    // output still carried every `inkscape:` and `sodipodi:` attribute.
    let src = "<svg xmlns=\"http://www.w3.org/2000/svg\" \
               xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" \
               xmlns:sodipodi=\"http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd\" \
               width=\"10\" height=\"10\">\
               <sodipodi:namedview id=\"nv\"/>\
               <g inkscape:groupmode=\"layer\" inkscape:label=\"Layer 1\">\
               <rect width=\"5\" height=\"5\" fill=\"#ff0000\"/></g></svg>";
    let document = parse_svg(src).expect("parse");

    let plain = sivgo_io::export_plain_svg(&document);
    assert!(
        !plain.contains("inkscape:"),
        "inkscape markup survived: {plain}"
    );
    assert!(
        !plain.contains("sodipodi:"),
        "sodipodi markup survived: {plain}"
    );

    // The drawing itself is still there and still parses.
    assert!(plain.contains("<rect"));
    assert!(plain.contains("#ff0000"));
    parse_svg(&plain).expect("plain output must re-parse");
}

#[test]
fn tracing_a_bitmap_file_produces_paths() {
    // A black square on a white background, large enough for the contour
    // tracer to find an outline.
    const SIZE: u32 = 32;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let inside = (8..24).contains(&x) && (8..24).contains(&y);
            let value = if inside { 0 } else { 255 };
            pixels.extend_from_slice(&[value, value, value, 255]);
        }
    }

    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, SIZE, SIZE);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(&pixels).expect("png data");
    }

    let traced = sivgo_io::trace_bitmap_file(&png, &sivgo_io::TraceOptions::default())
        .expect("tracing should succeed");
    let root = traced.root().expect("traced document needs a root");
    assert!(
        !traced.children(root).is_empty(),
        "tracing a filled square should produce at least one path"
    );
}

#[test]
fn tracing_rejects_data_that_is_not_an_image() {
    assert!(
        sivgo_io::trace_bitmap_file(b"not an image", &sivgo_io::TraceOptions::default()).is_err()
    );
}
