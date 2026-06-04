//! Vertical toolbar component showing tool buttons grouped by category.

use dioxus::prelude::*;
use sivgo_core::ToolId;

/// Describes a single toolbar entry: tool id, shortcut key, and tooltip.
struct ToolEntry {
    id: ToolId,
    #[allow(dead_code)]
    label: &'static str,
    #[allow(dead_code)]
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
                    label: "Select",
                    shortcut: "S",
                    tooltip: "Select and Transform (S)",
                },
                ToolEntry {
                    id: ToolId::Node,
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
                    label: "Rect",
                    shortcut: "R",
                    tooltip: "Rectangle (R)",
                },
                ToolEntry {
                    id: ToolId::Ellipse,
                    label: "Ellipse",
                    shortcut: "E",
                    tooltip: "Ellipse (E)",
                },
                ToolEntry {
                    id: ToolId::Star,
                    label: "Star",
                    shortcut: "*",
                    tooltip: "Star / Polygon (*)",
                },
                ToolEntry {
                    id: ToolId::Spiral,
                    label: "Spiral",
                    shortcut: "I",
                    tooltip: "Spiral (I)",
                },
                ToolEntry {
                    id: ToolId::Box3d,
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
                    label: "Pen",
                    shortcut: "B",
                    tooltip: "Pen / Bezier (B)",
                },
                ToolEntry {
                    id: ToolId::Pencil,
                    label: "Pencil",
                    shortcut: "P",
                    tooltip: "Pencil / Freehand (P)",
                },
                ToolEntry {
                    id: ToolId::Calligraphy,
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
                    label: "Text",
                    shortcut: "T",
                    tooltip: "Text (T)",
                },
                ToolEntry {
                    id: ToolId::Gradient,
                    label: "Gradient",
                    shortcut: "G",
                    tooltip: "Gradient (G)",
                },
                ToolEntry {
                    id: ToolId::Mesh,
                    label: "Mesh",
                    shortcut: "M",
                    tooltip: "Mesh Gradient (M)",
                },
                ToolEntry {
                    id: ToolId::Dropper,
                    label: "Dropper",
                    shortcut: "D",
                    tooltip: "Color Picker / Dropper (D)",
                },
                ToolEntry {
                    id: ToolId::BucketFill,
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
                    label: "Spray",
                    shortcut: "A",
                    tooltip: "Spray (A)",
                },
                ToolEntry {
                    id: ToolId::Tweak,
                    label: "Tweak",
                    shortcut: "W",
                    tooltip: "Tweak / Sculpt (W)",
                },
                ToolEntry {
                    id: ToolId::Eraser,
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
                    label: "Connect",
                    shortcut: "O",
                    tooltip: "Connector (O)",
                },
                ToolEntry {
                    id: ToolId::Marker,
                    label: "Marker",
                    shortcut: "K",
                    tooltip: "Marker (K)",
                },
                ToolEntry {
                    id: ToolId::Pages,
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
                    label: "Measure",
                    shortcut: "F5",
                    tooltip: "Measure (F5)",
                },
                ToolEntry {
                    id: ToolId::Zoom,
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
/// Each button shows an inline SVG icon. The currently active tool is highlighted.
/// Clicking a button fires `on_tool_change` with the corresponding [`ToolId`].
#[component]
pub fn Toolbar(active_tool: ToolId, on_tool_change: EventHandler<ToolId>) -> Element {
    let groups = tool_groups();

    rsx! {
        div { class: "toolbar",
            for (_group_label, entries) in groups {
                div { class: "toolbar-group",
                    for entry in entries {
                        button {
                            class: if active_tool == entry.id { "tool-btn active" } else { "tool-btn" },
                            title: "{entry.tooltip}",
                            onclick: move |_| on_tool_change.call(entry.id),
                            ToolSvgIcon { id: entry.id }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ToolSvgIcon(id: ToolId) -> Element {
    rsx! {
        svg {
            class: "tool-svg-icon",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            for path_data in tool_icon_paths(id) {
                path { d: "{path_data}" }
            }
        }
    }
}

#[cfg(test)]
fn toolbar_tool_ids() -> Vec<ToolId> {
    tool_groups()
        .into_iter()
        .flat_map(|(_, entries)| entries.into_iter().map(|entry| entry.id))
        .collect()
}

fn tool_icon_paths(id: ToolId) -> &'static [&'static str] {
    match id {
        ToolId::Select => &["M5 3 L18 12 L12 14 L9 21 Z", "M12 14 L16 20"],
        ToolId::Node => &["M5 12 C8 5 16 5 19 12 C16 19 8 19 5 12 Z", "M5 12 H19", "M12 5 V19"],
        ToolId::Rect => &["M5 6 H19 V18 H5 Z"],
        ToolId::Ellipse => &["M4 12 C4 7.6 7.6 5 12 5 C16.4 5 20 7.6 20 12 C20 16.4 16.4 19 12 19 C7.6 19 4 16.4 4 12 Z"],
        ToolId::Star => &["M12 3 L14.6 9 L21 9.5 L16 13.5 L17.7 20 L12 16.4 L6.3 20 L8 13.5 L3 9.5 L9.4 9 Z"],
        ToolId::Spiral => &["M17.5 12 C17.5 15 15 17 12 17 C8.2 17 5 14.2 5 10.7 C5 7.4 7.6 5 10.8 5 C14 5 16.5 7.1 16.5 9.8 C16.5 12.3 14.5 14 12.1 14 C10.1 14 8.8 12.8 8.8 11.2 C8.8 9.9 9.8 8.9 11.2 8.9 C12.2 8.9 13 9.5 13 10.4"],
        ToolId::Box3d => &["M5 8 L12 4 L19 8 L12 12 Z", "M5 8 V16 L12 20 V12", "M19 8 V16 L12 20"],
        ToolId::Pen => &["M5 19 L8.5 10.5 L13.5 5.5 L18.5 10.5 L13.5 15.5 Z", "M8.5 10.5 L13.5 15.5", "M5 19 L10 17"],
        ToolId::Pencil => &["M4 18 L6 21 L10 19 L19 10 L15 6 L6 15 Z", "M14 7 L18 11"],
        ToolId::Calligraphy => &["M6 20 C8 14 10 8 16 4 C18 5.5 19 7.5 18.5 10 C14 11 10 15 8 21 Z", "M8 17 C11 16 13 14 15 11"],
        ToolId::Text => &["M5 5 H19", "M12 5 V19", "M8 19 H16"],
        ToolId::Gradient => &["M5 6 H19 V18 H5 Z", "M7 16 L17 6", "M8 16 H16"],
        ToolId::Mesh => &["M5 5 H19 V19 H5 Z", "M5 12 H19", "M12 5 V19", "M7 7 C10 10 14 10 17 7", "M7 17 C10 14 14 14 17 17"],
        ToolId::Dropper => &["M14 4 L20 10", "M18.5 8.5 L9 18 L5 19 L6 15 L15.5 5.5", "M7 15 L9 17"],
        ToolId::BucketFill => &["M7 4 L17 14 L12 19 L2 9 Z", "M7 4 L10 1", "M15 16 C17 18 18 19.5 18 21 C18 22.2 17.1 23 16 23 C14.9 23 14 22.2 14 21 C14 19.5 15 18 15 16 Z"],
        ToolId::Spray => &["M9 14 C9 11 11 9 14 9 V16 C11 16 9 17.5 8 20", "M14 5 H19", "M16 8 H21", "M17 12 H20", "M5 6 L5.1 6", "M7 9 L7.1 9"],
        ToolId::Tweak => &["M5 14 C8 9 12 19 19 10", "M5 10 C9 17 13 7 19 15"],
        ToolId::Eraser => &["M6 16 L14 8 L19 13 L12 20 H7 Z", "M10 12 L15 17", "M4 20 H20"],
        ToolId::Connector => &["M6 7 H10 C14 7 10 17 14 17 H18", "M4 5 H8 V9 H4 Z", "M16 15 H20 V19 H16 Z"],
        ToolId::Marker => &["M5 19 L12 4 L19 19 L12 16 Z", "M9 13 H15"],
        ToolId::Pages => &["M7 4 H17 V16 H7 Z", "M10 8 H20 V20 H10 Z"],
        ToolId::Measure => &["M4 17 L17 4", "M7 14 L10 17", "M10 11 L13 14", "M13 8 L16 11", "M3 18 L6 21", "M18 3 L21 6"],
        ToolId::Zoom => &["M10.5 5 C13.5 5 16 7.5 16 10.5 C16 13.5 13.5 16 10.5 16 C7.5 16 5 13.5 5 10.5 C5 7.5 7.5 5 10.5 5 Z", "M15 15 L20 20", "M8 10.5 H13", "M10.5 8 V13"],
        ToolId::Lpe => &["M4 16 C8 5 16 19 20 8", "M6 6 L8 8", "M16 16 L18 18"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn toolbar_icon_paths_cover_every_exposed_tool() {
        let ids = toolbar_tool_ids();
        assert_eq!(ids.len(), 23);
        let unique = ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(unique.len(), ids.len());
        for id in ids {
            assert!(
                !tool_icon_paths(id).is_empty(),
                "missing SVG paths for {id:?}"
            );
        }
    }

    #[test]
    fn toolbar_icons_are_not_letter_fallbacks() {
        for id in toolbar_tool_ids() {
            for path in tool_icon_paths(id) {
                assert!(
                    path.contains(' ') || path.len() > 8,
                    "path is too small for {id:?}: {path}"
                );
                assert!(
                    path.starts_with('M') || path.starts_with('m'),
                    "SVG path should start with move command for {id:?}: {path}"
                );
            }
        }
    }
}
