//! Vertical toolbar component showing tool buttons grouped by category.

use dioxus::prelude::*;
use sivgo_core::ToolId;

/// Describes a single toolbar entry: tool id, Unicode icon, shortcut key, and tooltip.
struct ToolEntry {
    id: ToolId,
    icon: &'static str,
    label: &'static str,
    shortcut: &'static str,
    tooltip: &'static str,
}

/// Returns all tools grouped by category.
/// Each group is (group_label, Vec<ToolEntry>).
fn tool_groups() -> Vec<(&'static str, Vec<ToolEntry>)> {
    vec![
        (
            "Selection",
            vec![
                ToolEntry {
                    id: ToolId::Select,
                    icon: "\u{25ED}", // pointer
                    label: "Select",
                    shortcut: "S",
                    tooltip: "Select and Transform (S)",
                },
                ToolEntry {
                    id: ToolId::Node,
                    icon: "\u{25C7}", // diamond
                    label: "Nodes",
                    shortcut: "N",
                    tooltip: "Edit Nodes (N)",
                },
            ],
        ),
        (
            "Shapes",
            vec![
                ToolEntry {
                    id: ToolId::Rect,
                    icon: "\u{25AD}", // rectangle
                    label: "Rect",
                    shortcut: "R",
                    tooltip: "Rectangle (R)",
                },
                ToolEntry {
                    id: ToolId::Ellipse,
                    icon: "\u{25CB}", // circle
                    label: "Ellipse",
                    shortcut: "E",
                    tooltip: "Ellipse (E)",
                },
                ToolEntry {
                    id: ToolId::Star,
                    icon: "\u{2606}", // star
                    label: "Star",
                    shortcut: "*",
                    tooltip: "Star / Polygon (*)",
                },
                ToolEntry {
                    id: ToolId::Spiral,
                    icon: "\u{0040}", // @-spiral
                    label: "Spiral",
                    shortcut: "I",
                    tooltip: "Spiral (I)",
                },
                ToolEntry {
                    id: ToolId::Box3d,
                    icon: "\u{2B1C}", // 3D box
                    label: "3D Box",
                    shortcut: "X",
                    tooltip: "3D Box (X)",
                },
            ],
        ),
        (
            "Drawing",
            vec![
                ToolEntry {
                    id: ToolId::Pen,
                    icon: "\u{2712}", // nib
                    label: "Pen",
                    shortcut: "B",
                    tooltip: "Pen / Bezier (B)",
                },
                ToolEntry {
                    id: ToolId::Pencil,
                    icon: "\u{270F}", // pencil
                    label: "Pencil",
                    shortcut: "P",
                    tooltip: "Pencil / Freehand (P)",
                },
                ToolEntry {
                    id: ToolId::Calligraphy,
                    icon: "\u{1F58B}", // calligraphy pen (may fallback)
                    label: "Ink",
                    shortcut: "C",
                    tooltip: "Calligraphy (C)",
                },
            ],
        ),
        (
            "Text & Paint",
            vec![
                ToolEntry {
                    id: ToolId::Text,
                    icon: "A",
                    label: "Text",
                    shortcut: "T",
                    tooltip: "Text (T)",
                },
                ToolEntry {
                    id: ToolId::Gradient,
                    icon: "\u{25B3}", // gradient triangle
                    label: "Gradient",
                    shortcut: "G",
                    tooltip: "Gradient (G)",
                },
                ToolEntry {
                    id: ToolId::Mesh,
                    icon: "\u{2593}", // mesh fill
                    label: "Mesh",
                    shortcut: "M",
                    tooltip: "Mesh Gradient (M)",
                },
                ToolEntry {
                    id: ToolId::Dropper,
                    icon: "\u{1F4A7}", // dropper
                    label: "Dropper",
                    shortcut: "D",
                    tooltip: "Color Picker / Dropper (D)",
                },
                ToolEntry {
                    id: ToolId::BucketFill,
                    icon: "\u{25A8}", // bucket
                    label: "Bucket",
                    shortcut: "U",
                    tooltip: "Bucket Fill (U)",
                },
            ],
        ),
        (
            "Modify",
            vec![
                ToolEntry {
                    id: ToolId::Spray,
                    icon: "\u{2055}", // spray
                    label: "Spray",
                    shortcut: "A",
                    tooltip: "Spray (A)",
                },
                ToolEntry {
                    id: ToolId::Tweak,
                    icon: "\u{219D}", // tweak
                    label: "Tweak",
                    shortcut: "W",
                    tooltip: "Tweak / Sculpt (W)",
                },
                ToolEntry {
                    id: ToolId::Eraser,
                    icon: "\u{232B}", // eraser / delete
                    label: "Erase",
                    shortcut: "Shift+E",
                    tooltip: "Eraser (Shift+E)",
                },
            ],
        ),
        (
            "Connectors & Layout",
            vec![
                ToolEntry {
                    id: ToolId::Connector,
                    icon: "\u{2194}", // connector / arrow
                    label: "Connect",
                    shortcut: "O",
                    tooltip: "Connector (O)",
                },
                ToolEntry {
                    id: ToolId::Marker,
                    icon: "\u{25C9}", // marker
                    label: "Marker",
                    shortcut: "K",
                    tooltip: "Marker (K)",
                },
                ToolEntry {
                    id: ToolId::Pages,
                    icon: "\u{2630}", // pages
                    label: "Pages",
                    shortcut: "",
                    tooltip: "Pages / Multi-page",
                },
            ],
        ),
        (
            "Measure & Zoom",
            vec![
                ToolEntry {
                    id: ToolId::Measure,
                    icon: "\u{2702}", // measure / ruler
                    label: "Measure",
                    shortcut: "F5",
                    tooltip: "Measure (F5)",
                },
                ToolEntry {
                    id: ToolId::Zoom,
                    icon: "\u{2315}", // zoom / magnifier
                    label: "Zoom",
                    shortcut: "Z",
                    tooltip: "Zoom (Z)",
                },
            ],
        ),
    ]
}

/// A vertical toolbar displaying all available drawing tools, grouped by category.
///
/// Each button shows a Unicode icon. The currently active tool is highlighted.
/// Clicking a button fires `on_tool_change` with the corresponding [`ToolId`].
#[component]
pub fn Toolbar(active_tool: ToolId, on_tool_change: EventHandler<ToolId>) -> Element {
    let groups = tool_groups();

    rsx! {
        div { class: "toolbar",
            for (group_label, entries) in groups {
                div { class: "toolbar-group",
                    div { class: "toolbar-group-label", "{group_label}" }
                    for entry in entries {
                        button {
                            class: if active_tool == entry.id { "tool-btn active" } else { "tool-btn" },
                            title: "{entry.tooltip}",
                            onclick: move |_| on_tool_change.call(entry.id),
                            div { class: "tool-btn-main",
                                span { class: "tool-icon", "{entry.icon}" }
                                div { class: "tool-meta",
                                    span { class: "tool-label", "{entry.label}" }
                                    if !entry.shortcut.is_empty() {
                                        span { class: "tool-shortcut", "{entry.shortcut}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
