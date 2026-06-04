//! Canvas view component: renders the SVG document and overlays selection handles.
//!
//! Converts the SVG document to a base64 PNG data URI for display in an `<img>`,
//! overlays selection handles, and dispatches mouse/keyboard events.

use base64::Engine;
use dioxus::prelude::*;
use sivgo_core::{DisplayMode, EditorEngine, ToolId};
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
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer
            .write_image_data(&straight)
            .expect("PNG data write failed");
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

/// A selection handle overlay rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionHandle {
    /// Screen-space X coordinate.
    pub x: f64,
    /// Screen-space Y coordinate.
    pub y: f64,
    /// Width of the handle in pixels.
    pub width: f64,
    /// Height of the handle in pixels.
    pub height: f64,
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

/// Map a `ToolId` to a CSS cursor value.
fn cursor_for_tool(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select => "default",
        ToolId::Node => "crosshair",
        ToolId::Rect | ToolId::Ellipse | ToolId::Star | ToolId::Spiral | ToolId::Box3d => {
            "crosshair"
        }
        ToolId::Pen | ToolId::Pencil | ToolId::Calligraphy => "crosshair",
        ToolId::Text => "text",
        ToolId::Zoom => "zoom-in",
        ToolId::Gradient | ToolId::Mesh => "crosshair",
        ToolId::Dropper => "crosshair",
        ToolId::BucketFill => "crosshair",
        ToolId::Connector => "crosshair",
        ToolId::Eraser => "crosshair",
        ToolId::Spray => "crosshair",
        ToolId::Tweak => "crosshair",
        ToolId::Measure => "crosshair",
        ToolId::Marker => "crosshair",
        ToolId::Pages => "default",
        ToolId::Lpe => "default",
    }
}

/// The canvas view component displaying the rendered document, selection handles,
/// and dispatching mouse/keyboard/wheel events.
///
/// The canvas renders the document as a PNG data URI in an `<img>` element.
/// Selection handles are overlaid as absolutely-positioned `<div>` elements.
/// The cursor style changes based on the currently active tool.
#[component]
pub fn CanvasView(
    /// The rendered data URI for the document image.
    data_uri: String,
    /// Width of the canvas in pixels.
    width: u32,
    /// Height of the canvas in pixels.
    height: u32,
    /// The currently active tool, used to set the cursor.
    active_tool: ToolId,
    /// Selection handle overlay rectangles to display on the canvas.
    #[props(default = Vec::new())]
    selection_handles: Vec<SelectionHandle>,
    /// Called when a mouse button is pressed on the canvas.
    on_mouse_down: EventHandler<Event<dioxus::events::MouseData>>,
    /// Called when the mouse moves over the canvas.
    on_mouse_move: EventHandler<Event<dioxus::events::MouseData>>,
    /// Called when a mouse button is released on the canvas.
    on_mouse_up: EventHandler<Event<dioxus::events::MouseData>>,
    /// Called when the mouse wheel is scrolled on the canvas.
    on_wheel: EventHandler<Event<dioxus::events::WheelData>>,
    /// Called when a key is pressed while the canvas is focused.
    on_key_down: EventHandler<Event<dioxus::events::KeyboardData>>,
    /// Called when a key is released while the canvas is focused.
    on_key_up: EventHandler<Event<dioxus::events::KeyboardData>>,
) -> Element {
    let cursor = cursor_for_tool(active_tool);
    let w = width;
    let h = height;

    rsx! {
        div {
            class: "canvas-view",
            style: "position: relative; width: {w}px; height: {h}px; cursor: {cursor};",
            tabindex: "0",
            onkeydown: move |evt| on_key_down.call(evt),
            onkeyup: move |evt| on_key_up.call(evt),

            img {
                src: "{data_uri}",
                width: "{w}",
                height: "{h}",
                draggable: "false",
                style: "display: block;",
                onmousedown: move |evt| on_mouse_down.call(evt),
                onmousemove: move |evt| on_mouse_move.call(evt),
                onmouseup: move |evt| on_mouse_up.call(evt),
                onwheel: move |evt| on_wheel.call(evt),
            }

            // Selection handle overlays
            for (i, handle) in selection_handles.iter().enumerate() {
                div {
                    key: "{i}",
                    class: "selection-handle",
                    style: "position: absolute; left: {handle.x}px; top: {handle.y}px; width: {handle.width}px; height: {handle.height}px; border: 1px dashed #00aaff; pointer-events: none;",
                }
            }
        }
    }
}
