//! Canvas view component: renders the SVG document and overlays selection handles.
//!
//! Converts the SVG document to a base64 PNG data URI for display in an `<img>`,
//! overlays selection handles, and dispatches mouse/keyboard events.

use base64::Engine;
use dioxus::prelude::*;
use sivgo_core::{EditorEngine, ToolId};
use sivgo_render::{DocumentRenderer, RenderBackend, TinySkiaBackend};

/// Render the document to a base64-encoded PNG data URI suitable for use
/// as an `<img src="...">` attribute.
pub fn render_to_data_uri(engine: &EditorEngine, width: u32, height: u32) -> String {
    let mut backend = TinySkiaBackend::new(width, height);
    backend.clear(sivgo_dom::Color::WHITE);
    DocumentRenderer::render(&engine.document, &mut backend);

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
