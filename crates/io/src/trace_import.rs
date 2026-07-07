//! Bitmap → SVG tracing entry point.
//!
//! Wraps the `sivgo-trace` crate (which produces `BezPath` contours from a
//! raster) and assembles the result into an [`SvgDocument`] of filled `<path>`
//! elements, so callers (desktop "Trace Bitmap", import pipelines) get a ready
//! document without depending on `sivgo-trace` directly.

use sivgo_dom::{
    Color, Paint, PathAttrs, PresentationAttrs, SvgDocument, SvgElement, SvgLength, SvgSvgAttrs,
    ViewBox,
};

pub use sivgo_trace::{
    trace_centerline, trace_colors, CenterlineOptions, ColorTraceOptions, TraceOptions,
};

/// Trace an RGBA8 bitmap into an [`SvgDocument`] of black filled paths
/// (monochrome / single-layer trace).
pub fn trace_bitmap(rgba: &[u8], width: u32, height: u32, options: &TraceOptions) -> SvgDocument {
    let paths = sivgo_trace::trace_rgba(rgba, width, height, options);

    let mut doc = SvgDocument::new();
    let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
        width: Some(SvgLength::px(width as f64)),
        height: Some(SvgLength::px(height as f64)),
        view_box: Some(ViewBox::new(0.0, 0.0, width as f64, height as f64)),
        preserve_aspect_ratio: None,
    }));
    doc.set_root(svg_id);

    for d in paths {
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        if let Some(node) = doc.get_node_mut(path_id) {
            node.presentation = PresentationAttrs {
                fill: Some(Paint::Color(Color::BLACK)),
                ..PresentationAttrs::default()
            };
        }
        doc.append_child(svg_id, path_id);
    }

    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_a_filled_square_into_paths() {
        // 16x16 RGBA: opaque black square in the middle on transparent ground.
        let (w, h) = (16u32, 16u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 4..12 {
            for x in 4..12 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = 0;
                rgba[i + 1] = 0;
                rgba[i + 2] = 0;
                rgba[i + 3] = 255;
            }
        }
        let doc = trace_bitmap(&rgba, w, h, &TraceOptions::default());
        // Root <svg> plus at least one traced <path>.
        let path_count = doc
            .nodes()
            .filter(|(_, n)| matches!(n.element, SvgElement::Path(_)))
            .count();
        assert!(path_count >= 1, "expected at least one traced path");
    }
}
