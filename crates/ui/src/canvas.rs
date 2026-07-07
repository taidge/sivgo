//! Canvas rendering: converts the SVG document to a base64 PNG data URI for
//! display in an `<img>`, applying the active display mode (outline, print
//! preview, proof colors, ...).

use base64::Engine;
use sivgo_core::{DisplayMode, EditorEngine};
use sivgo_dom::{Color, LengthUnit, Paint, SvgDocument, SvgElement, SvgLength};
use sivgo_render::{DocumentRenderer, RenderBackend, TinySkiaBackend};

/// Render the document to a base64-encoded PNG data URI suitable for use
/// as an `<img src="...">` attribute.
pub fn render_to_data_uri(engine: &EditorEngine, width: u32, height: u32) -> String {
    let mut backend = TinySkiaBackend::new(width, height);
    backend.clear(sivgo_dom::Color::WHITE);

    match engine.viewport.display_mode {
        DisplayMode::Normal => DocumentRenderer::render(&engine.document, &mut backend),
        mode => {
            let mut document = engine.document.clone();
            apply_display_mode(&mut document, mode);
            DocumentRenderer::render(&document, &mut backend);
        }
    }

    // Get premultiplied RGBA pixels from the backend.
    let pixels = backend.pixels();

    // Convert premultiplied alpha to straight alpha for PNG encoding.
    let straight: Vec<u8> = pixels
        .chunks(4)
        .flat_map(|p| {
            let a = p[3] as u16;
            if a == 0 {
                [0u8, 0, 0, 0]
            } else if a < 255 {
                let r = ((p[0] as u16 * 255 + a / 2) / a).min(255) as u8;
                let g = ((p[1] as u16 * 255 + a / 2) / a).min(255) as u8;
                let b = ((p[2] as u16 * 255 + a / 2) / a).min(255) as u8;
                [r, g, b, p[3]]
            } else {
                [p[0], p[1], p[2], p[3]]
            }
        })
        .collect();

    // Encode as PNG.
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // Encode defensively: a PNG write failure must not crash the canvas.
        // On error we fall back to an empty (transparent) image rather than panic.
        let encoded = encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&straight));
        if let Err(e) = encoded {
            log::error!("canvas PNG encode failed: {e}");
            return "data:image/png;base64,".to_string();
        }
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
    format!("data:image/png;base64,{}", b64)
}

fn apply_display_mode(document: &mut SvgDocument, mode: DisplayMode) {
    if mode == DisplayMode::Normal {
        return;
    }

    let node_ids: Vec<_> = document.nodes().map(|(id, _)| *id).collect();
    for id in node_ids {
        let Some(node) = document.get_node_mut(id) else {
            continue;
        };

        if matches!(
            mode,
            DisplayMode::NoFilters
                | DisplayMode::Outline
                | DisplayMode::PrintPreview
                | DisplayMode::ProofColors
        ) {
            node.presentation.filter = None;
            node.attributes.remove("filter");
        }

        match mode {
            DisplayMode::Outline => {
                if node.element.is_shape()
                    || matches!(node.element, SvgElement::Text(_) | SvgElement::FlowRoot(_))
                {
                    node.presentation.fill = Some(Paint::None);
                    node.presentation.stroke = Some(Paint::Color(Color::BLACK));
                    node.presentation.stroke_width = Some(SvgLength::new(1.0, LengthUnit::None));
                    node.presentation.stroke_dasharray = None;
                    node.presentation.marker_start = None;
                    node.presentation.marker_mid = None;
                    node.presentation.marker_end = None;
                }
            }
            DisplayMode::VisibleHairlines | DisplayMode::PrintPreview => {
                promote_hairline_stroke(node);
            }
            DisplayMode::ProofColors => {
                node.presentation.color = node.presentation.color.map(proof_color);
                node.presentation.fill = node.presentation.fill.clone().map(proof_paint);
                node.presentation.stroke = node.presentation.stroke.clone().map(proof_paint);
            }
            DisplayMode::Normal | DisplayMode::NoFilters => {}
        }
    }
}

fn promote_hairline_stroke(node: &mut sivgo_dom::SvgNode) {
    if node.presentation.stroke.is_some() {
        let current = node
            .presentation
            .stroke_width
            .map(|width| width.to_px())
            .unwrap_or(1.0);
        if current < 1.0 {
            node.presentation.stroke_width = Some(SvgLength::new(1.0, LengthUnit::None));
        }
    }
}

fn proof_paint(paint: Paint) -> Paint {
    match paint {
        Paint::Color(color) => Paint::Color(proof_color(color)),
        other => other,
    }
}

fn proof_color(color: Color) -> Color {
    let luminance =
        (0.2126 * color.r as f64 + 0.7152 * color.g as f64 + 0.0722 * color.b as f64).round() as u8;
    Color::new(luminance, luminance, luminance, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{RectAttrs, SvgSvgAttrs};

    fn document_with_rect() -> (SvgDocument, sivgo_dom::NodeId) {
        let mut document = SvgDocument::new();
        let root = document.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::new(100.0, LengthUnit::None)),
            height: Some(SvgLength::new(100.0, LengthUnit::None)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        document.set_root(root);
        let rect = document.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::zero(),
            y: SvgLength::zero(),
            width: SvgLength::new(50.0, LengthUnit::None),
            height: SvgLength::new(50.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        document.append_child(root, rect);
        (document, rect)
    }

    #[test]
    fn print_preview_removes_filters_and_promotes_hairlines() {
        let (mut document, rect) = document_with_rect();
        let node = document.get_node_mut(rect).unwrap();
        node.presentation.filter = Some("url(#blur)".to_string());
        node.attributes
            .insert("filter".to_string(), "url(#blur)".to_string());
        node.presentation.stroke = Some(Paint::Color(Color::BLACK));
        node.presentation.stroke_width = Some(SvgLength::new(0.2, LengthUnit::None));

        apply_display_mode(&mut document, DisplayMode::PrintPreview);

        let node = document.get_node(rect).unwrap();
        assert!(node.presentation.filter.is_none());
        assert!(!node.attributes.contains_key("filter"));
        assert_eq!(
            node.presentation.stroke_width,
            Some(SvgLength::new(1.0, LengthUnit::None))
        );
    }

    #[test]
    fn proof_colors_convert_solid_paints_to_luminance() {
        let (mut document, rect) = document_with_rect();
        let node = document.get_node_mut(rect).unwrap();
        node.presentation.fill = Some(Paint::Color(Color::RED));
        node.presentation.stroke = Some(Paint::Color(Color::BLUE));

        apply_display_mode(&mut document, DisplayMode::ProofColors);

        let node = document.get_node(rect).unwrap();
        let Some(Paint::Color(fill)) = node.presentation.fill else {
            panic!("expected solid fill");
        };
        let Some(Paint::Color(stroke)) = node.presentation.stroke else {
            panic!("expected solid stroke");
        };
        assert_eq!(fill.r, fill.g);
        assert_eq!(fill.g, fill.b);
        assert_eq!(stroke.r, stroke.g);
        assert_eq!(stroke.g, stroke.b);
        assert_ne!(fill, Color::RED);
        assert_ne!(stroke, Color::BLUE);
    }
}
