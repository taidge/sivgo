//! Main window layout component that arranges all UI panels in a grid.
//!
//! Layout structure:
//! ```text
//! +-----+-------------------------------+--------+
//! |     |         Menu Bar              |        |
//! +-----+-------------------------------+--------+
//! |     | Ruler (H)                     |        |
//! |     +-------------------------------+        |
//! | T   | Ruler | Canvas area           | Right  |
//! | o   | (V)   | (with scrollbars)     | Panels |
//! | o   |       |                       |        |
//! | l   |       |                       |        |
//! | b   +-------+-----------------------+        |
//! | a   |         Color Bar             |        |
//! | r   +-------------------------------+--------+
//! +-----+-------------------------------+--------+
//! |               Status Bar                     |
//! +----------------------------------------------+
//! ```

use dioxus::prelude::*;

/// The main application layout component.
///
/// Arranges the toolbar on the left, canvas with rulers in the center,
/// right panels on the right, a menu bar at the top, a color bar below
/// the canvas, and a status bar at the very bottom.
///
/// All child elements are passed as named slots via props so the layout
/// itself is purely structural.
#[component]
pub fn MainLayout(
    /// The menu bar element (top of the window).
    menu_bar: Element,
    /// The toolbar element (left side).
    toolbar: Element,
    /// The horizontal ruler element (above the canvas).
    horizontal_ruler: Element,
    /// The vertical ruler element (left of the canvas).
    vertical_ruler: Element,
    /// The canvas view element (center).
    canvas: Element,
    /// The horizontal scrollbar element (below the canvas).
    horizontal_scrollbar: Element,
    /// The vertical scrollbar element (right of the canvas).
    vertical_scrollbar: Element,
    /// The right-side panels element (properties, layers, etc.).
    right_panels: Element,
    /// The color indicator bar (below the canvas area).
    color_bar: Element,
    /// The status bar element (bottom of the window).
    status_bar: Element,
) -> Element {
    rsx! {
        div { class: "app",
            // Menu bar at the top
            {menu_bar}

            // Main content area (toolbar + center + right panels)
            div { class: "main-area",
                // Toolbar on the left
                {toolbar}

                // Center column: rulers + canvas + color bar
                div {
                    class: "center-column",
                    style: "flex: 1; display: flex; flex-direction: column; overflow: hidden;",

                    // Horizontal ruler row (with a corner spacer for the ruler intersection)
                    div {
                        style: "display: flex; flex-direction: row;",
                        // Corner spacer (ruler intersection)
                        div {
                            class: "ruler-corner",
                            style: "width: 20px; height: 20px; background: #2a2a2a; border-right: 1px solid #444; border-bottom: 1px solid #444; flex-shrink: 0;",
                        }
                        // Horizontal ruler
                        div {
                            style: "flex: 1; overflow: hidden;",
                            {horizontal_ruler}
                        }
                    }

                    // Canvas row with vertical ruler and scrollbar
                    div {
                        style: "flex: 1; display: flex; flex-direction: row; overflow: hidden;",
                        // Vertical ruler
                        div {
                            style: "flex-shrink: 0; overflow: hidden;",
                            {vertical_ruler}
                        }
                        // Canvas area
                        div {
                            class: "canvas-container",
                            style: "flex: 1; overflow: hidden; position: relative; display: flex; align-items: center; justify-content: center; background: #404040;",
                            {canvas}
                        }
                        // Vertical scrollbar
                        div {
                            style: "flex-shrink: 0;",
                            {vertical_scrollbar}
                        }
                    }

                    // Horizontal scrollbar row
                    div {
                        style: "display: flex; flex-direction: row;",
                        // Spacer for vertical ruler width
                        div {
                            style: "width: 20px; flex-shrink: 0;",
                        }
                        div {
                            style: "flex: 1;",
                            {horizontal_scrollbar}
                        }
                    }

                    // Color bar
                    {color_bar}
                }

                // Right panels
                {right_panels}
            }

            // Status bar at the bottom
            {status_bar}
        }
    }
}
