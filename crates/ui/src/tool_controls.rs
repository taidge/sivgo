//! Dynamic tool-controls bar that renders context-sensitive options for the active tool.
//!
//! Each tool type has its own set of controls (e.g., the Select tool shows X/Y/W/H
//! spin-boxes, the Pen tool shows smoothing options, etc.).

use dioxus::prelude::*;
use sivgo_core::ToolId;

// ---------------------------------------------------------------------------
// Action enum emitted by tool controls
// ---------------------------------------------------------------------------

/// Actions that tool-controls can emit to the parent.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolControlAction {
    // -- Select tool --
    SetX(f64),
    SetY(f64),
    SetWidth(f64),
    SetHeight(f64),

    // -- Node tool --
    SetNodeType(NodeType),
    SetSegmentType(SegmentType),

    // -- Shape tools (Rect, Ellipse, Star, Spiral, 3D Box) --
    SetShapeWidth(f64),
    SetShapeHeight(f64),
    SetCornerRadius(f64),
    SetStarCorners(u32),
    SetStarRatio(f64),
    SetSpiralTurns(f64),

    // -- Pen / Pencil --
    SetSmoothing(f64),
    SetShape(PenShape),

    // -- Text --
    SetFontFamily(String),
    SetFontSize(f64),
    ToggleBold,
    ToggleItalic,

    // -- Gradient --
    SetGradientType(GradientKind),
    AddStop,
    RemoveStop,

    // -- Snapping --
    ToggleSnapToGrid,
    ToggleSnapToGuides,
    ToggleSnapToObjects,
    ToggleSnapToPaths,
}

/// Node type for the Node editing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Cusp,
    Smooth,
    Symmetric,
    Auto,
}

/// Path-segment type for the Node editing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Line,
    Curve,
}

/// Pen stroke shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenShape {
    None,
    Triangle,
    FromClipboard,
    BendFromClipboard,
}

/// Gradient variant selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

/// A horizontal bar that shows different controls depending on the active tool.
///
/// The parent supplies the current [`ToolId`] and initial values. Every user
/// interaction emits a [`ToolControlAction`] through `on_action`.
#[component]
pub fn ToolControls(
    active_tool: ToolId,
    on_action: EventHandler<ToolControlAction>,
    // Select tool numeric fields
    #[props(default = 0.0)] sel_x: f64,
    #[props(default = 0.0)] sel_y: f64,
    #[props(default = 0.0)] sel_w: f64,
    #[props(default = 0.0)] sel_h: f64,
    // Snap toggles
    #[props(default = true)] snap_grid: bool,
    #[props(default = true)] snap_guides: bool,
    #[props(default = true)] snap_objects: bool,
    #[props(default = false)] snap_paths: bool,
) -> Element {
    rsx! {
        div { class: "tool-controls",
            match active_tool {
                ToolId::Select => rsx! {
                    SelectControls {
                        x: sel_x,
                        y: sel_y,
                        w: sel_w,
                        h: sel_h,
                        on_action: on_action,
                    }
                },
                ToolId::Node => rsx! {
                    NodeControls { on_action: on_action }
                },
                ToolId::Rect => rsx! {
                    RectControls { on_action: on_action }
                },
                ToolId::Ellipse => rsx! {
                    EllipseControls { on_action: on_action }
                },
                ToolId::Star => rsx! {
                    StarControls { on_action: on_action }
                },
                ToolId::Spiral => rsx! {
                    SpiralControls { on_action: on_action }
                },
                ToolId::Pen | ToolId::Pencil => rsx! {
                    PenPencilControls { on_action: on_action }
                },
                ToolId::Text => rsx! {
                    TextControls { on_action: on_action }
                },
                ToolId::Gradient => rsx! {
                    GradientControls { on_action: on_action }
                },
                _ => rsx! {
                    SnapControls {
                        snap_grid: snap_grid,
                        snap_guides: snap_guides,
                        snap_objects: snap_objects,
                        snap_paths: snap_paths,
                        on_action: on_action,
                    }
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-tool sub-components
// ---------------------------------------------------------------------------

#[component]
fn SelectControls(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    on_action: EventHandler<ToolControlAction>,
) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "X" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "{x}",
                step: "1",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetX(v));
                    }
                },
            }
            label { class: "tc-label", "Y" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "{y}",
                step: "1",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetY(v));
                    }
                },
            }
            label { class: "tc-label", "W" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "{w}",
                step: "1",
                min: "0",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetWidth(v));
                    }
                },
            }
            label { class: "tc-label", "H" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "{h}",
                step: "1",
                min: "0",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetHeight(v));
                    }
                },
            }
        }
    }
}

#[component]
fn NodeControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            span { class: "tc-section-label", "Node type:" }
            button {
                class: "tc-btn",
                title: "Cusp node",
                onclick: move |_| on_action.call(ToolControlAction::SetNodeType(NodeType::Cusp)),
                "\u{25C7}" // diamond
            }
            button {
                class: "tc-btn",
                title: "Smooth node",
                onclick: move |_| on_action.call(ToolControlAction::SetNodeType(NodeType::Smooth)),
                "\u{25CB}" // circle
            }
            button {
                class: "tc-btn",
                title: "Symmetric node",
                onclick: move |_| on_action.call(ToolControlAction::SetNodeType(NodeType::Symmetric)),
                "\u{25CE}" // bullseye
            }
            button {
                class: "tc-btn",
                title: "Auto-smooth node",
                onclick: move |_| on_action.call(ToolControlAction::SetNodeType(NodeType::Auto)),
                "\u{229A}" // circled ring
            }
        }
        div { class: "tc-group",
            span { class: "tc-section-label", "Segment:" }
            button {
                class: "tc-btn",
                title: "Make segment a line",
                onclick: move |_| on_action.call(ToolControlAction::SetSegmentType(SegmentType::Line)),
                "\u{2500}" // horizontal line
            }
            button {
                class: "tc-btn",
                title: "Make segment a curve",
                onclick: move |_| on_action.call(ToolControlAction::SetSegmentType(SegmentType::Curve)),
                "\u{223F}" // sine wave
            }
        }
    }
}

#[component]
fn RectControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "W" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                step: "1",
                min: "0",
                placeholder: "width",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetShapeWidth(v));
                    }
                },
            }
            label { class: "tc-label", "H" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                step: "1",
                min: "0",
                placeholder: "height",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetShapeHeight(v));
                    }
                },
            }
            label { class: "tc-label", "Rx" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                step: "1",
                min: "0",
                placeholder: "corner",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetCornerRadius(v));
                    }
                },
            }
        }
    }
}

#[component]
fn EllipseControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "Rx" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                step: "1",
                min: "0",
                placeholder: "rx",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetShapeWidth(v));
                    }
                },
            }
            label { class: "tc-label", "Ry" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                step: "1",
                min: "0",
                placeholder: "ry",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetShapeHeight(v));
                    }
                },
            }
        }
    }
}

#[component]
fn StarControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "Corners" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "5",
                step: "1",
                min: "3",
                max: "100",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<u32>() {
                        on_action.call(ToolControlAction::SetStarCorners(v));
                    }
                },
            }
            label { class: "tc-label", "Spoke ratio" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "0.5",
                step: "0.05",
                min: "0.01",
                max: "1.0",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetStarRatio(v));
                    }
                },
            }
        }
    }
}

#[component]
fn SpiralControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "Turns" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "3",
                step: "0.5",
                min: "0.5",
                max: "50",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetSpiralTurns(v));
                    }
                },
            }
        }
    }
}

#[component]
fn PenPencilControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "Smoothing" }
            input {
                r#type: "range",
                class: "tc-input tc-range",
                min: "0",
                max: "100",
                value: "50",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetSmoothing(v));
                    }
                },
            }
            span { class: "tc-section-label", "Shape:" }
            button {
                class: "tc-btn",
                title: "No shape",
                onclick: move |_| on_action.call(ToolControlAction::SetShape(PenShape::None)),
                "None"
            }
            button {
                class: "tc-btn",
                title: "Triangle shape",
                onclick: move |_| on_action.call(ToolControlAction::SetShape(PenShape::Triangle)),
                "\u{25B3}"
            }
            button {
                class: "tc-btn",
                title: "From clipboard",
                onclick: move |_| on_action.call(ToolControlAction::SetShape(PenShape::FromClipboard)),
                "Clip"
            }
        }
    }
}

#[component]
fn TextControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            label { class: "tc-label", "Font" }
            input {
                r#type: "text",
                class: "tc-input tc-text",
                value: "sans-serif",
                placeholder: "font family",
                onchange: move |evt: Event<FormData>| {
                    on_action.call(ToolControlAction::SetFontFamily(evt.value().to_string()));
                },
            }
            label { class: "tc-label", "Size" }
            input {
                r#type: "number",
                class: "tc-input tc-num",
                value: "16",
                step: "1",
                min: "1",
                max: "999",
                onchange: move |evt: Event<FormData>| {
                    if let Ok(v) = evt.value().parse::<f64>() {
                        on_action.call(ToolControlAction::SetFontSize(v));
                    }
                },
            }
            button {
                class: "tc-btn",
                title: "Bold",
                onclick: move |_| on_action.call(ToolControlAction::ToggleBold),
                "B"
            }
            button {
                class: "tc-btn",
                title: "Italic",
                onclick: move |_| on_action.call(ToolControlAction::ToggleItalic),
                "I"
            }
        }
    }
}

#[component]
fn GradientControls(on_action: EventHandler<ToolControlAction>) -> Element {
    rsx! {
        div { class: "tc-group",
            span { class: "tc-section-label", "Type:" }
            button {
                class: "tc-btn",
                title: "Linear gradient",
                onclick: move |_| on_action.call(ToolControlAction::SetGradientType(GradientKind::Linear)),
                "\u{2192}" // arrow right
            }
            button {
                class: "tc-btn",
                title: "Radial gradient",
                onclick: move |_| on_action.call(ToolControlAction::SetGradientType(GradientKind::Radial)),
                "\u{25CE}" // bullseye
            }
            span { class: "tc-separator" }
            span { class: "tc-section-label", "Stops:" }
            button {
                class: "tc-btn",
                title: "Add gradient stop",
                onclick: move |_| on_action.call(ToolControlAction::AddStop),
                "+"
            }
            button {
                class: "tc-btn",
                title: "Remove selected gradient stop",
                onclick: move |_| on_action.call(ToolControlAction::RemoveStop),
                "\u{2212}" // minus
            }
        }
    }
}

#[component]
fn SnapControls(
    snap_grid: bool,
    snap_guides: bool,
    snap_objects: bool,
    snap_paths: bool,
    on_action: EventHandler<ToolControlAction>,
) -> Element {
    rsx! {
        div { class: "tc-group",
            span { class: "tc-section-label", "Snap:" }
            button {
                class: if snap_grid { "tc-btn active" } else { "tc-btn" },
                title: "Snap to grid",
                onclick: move |_| on_action.call(ToolControlAction::ToggleSnapToGrid),
                "\u{2591} Grid"
            }
            button {
                class: if snap_guides { "tc-btn active" } else { "tc-btn" },
                title: "Snap to guides",
                onclick: move |_| on_action.call(ToolControlAction::ToggleSnapToGuides),
                "\u{2502} Guides"
            }
            button {
                class: if snap_objects { "tc-btn active" } else { "tc-btn" },
                title: "Snap to objects",
                onclick: move |_| on_action.call(ToolControlAction::ToggleSnapToObjects),
                "\u{25A1} Objects"
            }
            button {
                class: if snap_paths { "tc-btn active" } else { "tc-btn" },
                title: "Snap to paths",
                onclick: move |_| on_action.call(ToolControlAction::ToggleSnapToPaths),
                "\u{2312} Paths"
            }
        }
    }
}
