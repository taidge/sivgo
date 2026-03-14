//! Status bar component showing cursor position, zoom, selection info, and
//! object count.

use dioxus::prelude::*;

/// A horizontal status bar displayed at the bottom of the editor window.
///
/// Shows:
/// - Cursor position (x, y)
/// - Zoom level as a percentage
/// - Selection info (count and type description)
/// - Total object count in the document
#[component]
pub fn StatusBar(
    /// Cursor X position in document coordinates.
    cursor_x: f64,
    /// Cursor Y position in document coordinates.
    cursor_y: f64,
    /// Current zoom factor (1.0 = 100%).
    zoom: f64,
    /// Number of currently selected objects.
    selection_count: usize,
    /// Short description of the selected element type(s), e.g. "rect" or "mixed".
    #[props(default = String::new())]
    selection_type: String,
    /// Total number of objects in the document.
    #[props(default = 0)]
    object_count: usize,
) -> Element {
    let zoom_pct = zoom * 100.0;

    let selection_text = if selection_count == 0 {
        "No selection".to_string()
    } else if selection_count == 1 {
        if selection_type.is_empty() {
            "1 selected".to_string()
        } else {
            format!("1 {} selected", selection_type)
        }
    } else if selection_type.is_empty() {
        format!("{} selected", selection_count)
    } else {
        format!("{} {} selected", selection_count, selection_type)
    };

    rsx! {
        div { class: "status-bar",
            span { class: "status-item",
                "X: {cursor_x:.1}  Y: {cursor_y:.1}"
            }
            span { class: "status-item",
                "Zoom: {zoom_pct:.0}%"
            }
            span { class: "status-item",
                "{selection_text}"
            }
            if object_count > 0 {
                span { class: "status-item",
                    "Objects: {object_count}"
                }
            }
        }
    }
}
