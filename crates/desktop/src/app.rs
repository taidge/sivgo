//! Dioxus desktop window and main application layout (12.2).
//!
//! Provides the `XsvgApp` component that establishes the top-level window
//! structure: menu bar, toolbar, canvas area, and side panels.

use dioxus::prelude::*;

/// Initial window width in logical pixels.
pub const INITIAL_WIDTH: f64 = 1280.0;
/// Initial window height in logical pixels.
pub const INITIAL_HEIGHT: f64 = 800.0;
/// Default window title.
pub const WINDOW_TITLE: &str = "Sivgo - SVG Editor";

// ---------------------------------------------------------------------------
// XsvgApp component
// ---------------------------------------------------------------------------

/// Top-level Dioxus component that defines the main editor layout.
///
/// The layout is composed of four placeholder regions:
/// 1. **Menu bar** -- across the top.
/// 2. **Toolbar** -- vertical strip on the left.
/// 3. **Canvas area** -- central workspace.
/// 4. **Panel area** -- right-side panels (properties, layers, etc.).
#[allow(non_snake_case)]
pub fn XsvgApp() -> Element {
    rsx! {
        style { {APP_CSS} }
        div { class: "sivgo-app",
            // Menu bar placeholder
            div { class: "sivgo-menu-bar",
                span { class: "sivgo-menu-title", "Sivgo" }
                span { class: "sivgo-menu-item", "File" }
                span { class: "sivgo-menu-item", "Edit" }
                span { class: "sivgo-menu-item", "View" }
                span { class: "sivgo-menu-item", "Object" }
                span { class: "sivgo-menu-item", "Path" }
                span { class: "sivgo-menu-item", "Help" }
            }

            div { class: "sivgo-main-area",
                // Toolbar placeholder
                div { class: "sivgo-toolbar",
                    div { class: "sivgo-tool-btn", title: "Select", "S" }
                    div { class: "sivgo-tool-btn", title: "Rectangle", "R" }
                    div { class: "sivgo-tool-btn", title: "Ellipse", "E" }
                    div { class: "sivgo-tool-btn", title: "Pen", "P" }
                    div { class: "sivgo-tool-btn", title: "Text", "T" }
                    div { class: "sivgo-tool-btn", title: "Zoom", "Z" }
                }

                // Canvas area placeholder
                div { class: "sivgo-canvas-area",
                    div { class: "sivgo-canvas-placeholder",
                        "Canvas Area"
                    }
                }

                // Panel area placeholder
                div { class: "sivgo-panel-area",
                    div { class: "sivgo-panel",
                        div { class: "sivgo-panel-header", "Properties" }
                        div { class: "sivgo-panel-body", "No selection" }
                    }
                    div { class: "sivgo-panel",
                        div { class: "sivgo-panel-header", "Layers" }
                        div { class: "sivgo-panel-body", "No layers" }
                    }
                }
            }

            // Status bar
            div { class: "sivgo-status-bar",
                span { "Ready" }
            }
        }
    }
}

/// Embedded CSS for the XsvgApp layout.
const APP_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}
body {
    font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
    font-size: 13px;
    overflow: hidden;
    background: #1e1e1e;
    color: #ccc;
    user-select: none;
}
.sivgo-app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
}
.sivgo-menu-bar {
    height: 32px;
    background: #2d2d2d;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-bottom: 1px solid #444;
}
.sivgo-menu-title {
    font-weight: bold;
    color: #0078d4;
    margin-right: 16px;
}
.sivgo-menu-item {
    color: #ddd;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 3px;
}
.sivgo-menu-item:hover {
    background: #3e3e3e;
}
.sivgo-main-area {
    display: flex;
    flex: 1;
    overflow: hidden;
}
.sivgo-toolbar {
    width: 42px;
    background: #2d2d2d;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding-top: 4px;
    border-right: 1px solid #444;
}
.sivgo-tool-btn {
    width: 34px;
    height: 34px;
    margin: 2px;
    background: #3e3e3e;
    color: #ddd;
    border: 1px solid #555;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    font-weight: bold;
}
.sivgo-tool-btn:hover {
    background: #4e4e4e;
    border-color: #777;
}
.sivgo-canvas-area {
    flex: 1;
    background: #404040;
    display: flex;
    align-items: center;
    justify-content: center;
}
.sivgo-canvas-placeholder {
    color: #666;
    font-size: 18px;
}
.sivgo-panel-area {
    width: 220px;
    background: #2d2d2d;
    border-left: 1px solid #444;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
}
.sivgo-panel {
    border-bottom: 1px solid #444;
}
.sivgo-panel-header {
    padding: 6px 10px;
    font-weight: bold;
    font-size: 11px;
    text-transform: uppercase;
    color: #999;
    background: #333;
    border-bottom: 1px solid #444;
}
.sivgo-panel-body {
    padding: 10px;
    color: #666;
    font-style: italic;
    font-size: 12px;
}
.sivgo-status-bar {
    height: 24px;
    background: #007acc;
    color: #fff;
    display: flex;
    align-items: center;
    padding: 0 12px;
    font-size: 12px;
}
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_dimensions() {
        assert!((INITIAL_WIDTH - 1280.0).abs() < f64::EPSILON);
        assert!((INITIAL_HEIGHT - 800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_window_title_constant() {
        assert_eq!(WINDOW_TITLE, "Sivgo - SVG Editor");
    }

    #[test]
    fn test_app_css_not_empty() {
        assert!(!APP_CSS.is_empty());
        assert!(APP_CSS.contains(".sivgo-app"));
        assert!(APP_CSS.contains(".sivgo-menu-bar"));
        assert!(APP_CSS.contains(".sivgo-toolbar"));
        assert!(APP_CSS.contains(".sivgo-canvas-area"));
        assert!(APP_CSS.contains(".sivgo-panel-area"));
    }
}
