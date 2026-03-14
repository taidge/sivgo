//! Color indicator bar component showing quick fill/stroke color display
//! with click-to-edit functionality.

use dioxus::prelude::*;

/// A compact color indicator bar showing the current fill and stroke colors.
///
/// Displays two overlapping color swatches (fill in front, stroke behind)
/// and a "none" indicator. Clicking on the fill or stroke swatch opens
/// an edit interaction via the corresponding event handler.
#[component]
pub fn ColorBar(
    /// Current fill color as a CSS hex string (e.g. "#ff0000"), or empty for none.
    fill_color: String,
    /// Current stroke color as a CSS hex string (e.g. "#000000"), or empty for none.
    stroke_color: String,
    /// Whether the fill is set to "none" (no fill).
    #[props(default = false)]
    fill_none: bool,
    /// Whether the stroke is set to "none" (no stroke).
    #[props(default = false)]
    stroke_none: bool,
    /// Called when the user clicks the fill swatch to edit the fill color.
    on_fill_click: EventHandler<()>,
    /// Called when the user clicks the stroke swatch to edit the stroke color.
    on_stroke_click: EventHandler<()>,
    /// Called when the user clicks the "no paint" button to set fill or stroke to none.
    on_set_none: EventHandler<()>,
) -> Element {
    // Fill swatch style: show a checkerboard pattern for "none", otherwise the color.
    let fill_bg = if fill_none || fill_color.is_empty() {
        "repeating-conic-gradient(#666 0% 25%, #999 0% 50%) 50% / 8px 8px".to_string()
    } else {
        fill_color.clone()
    };

    let fill_style = if fill_none || fill_color.is_empty() {
        format!(
            "width: 24px; height: 24px; border: 2px solid #888; border-radius: 2px; cursor: pointer; background: {};",
            fill_bg
        )
    } else {
        format!(
            "width: 24px; height: 24px; border: 2px solid #888; border-radius: 2px; cursor: pointer; background-color: {};",
            fill_color
        )
    };

    // Stroke swatch style.
    let stroke_bg = if stroke_none || stroke_color.is_empty() {
        "repeating-conic-gradient(#666 0% 25%, #999 0% 50%) 50% / 8px 8px".to_string()
    } else {
        stroke_color.clone()
    };

    let stroke_style = if stroke_none || stroke_color.is_empty() {
        format!(
            "width: 24px; height: 24px; border: 2px solid #888; border-radius: 2px; cursor: pointer; background: {};",
            stroke_bg
        )
    } else {
        format!(
            "width: 24px; height: 24px; border: 2px solid #888; border-radius: 2px; cursor: pointer; background-color: {};",
            stroke_color
        )
    };

    rsx! {
        div {
            class: "color-bar",
            style: "display: flex; align-items: center; gap: 6px; padding: 4px 8px; background: #2d2d2d; border-top: 1px solid #444;",

            // Fill swatch
            div {
                title: if fill_none { "Fill: none" } else if fill_color.is_empty() { "Fill: none" } else { "Fill: {fill_color}" },
                style: "{fill_style}",
                onclick: move |_| on_fill_click.call(()),
                if fill_none || fill_color.is_empty() {
                    span {
                        style: "display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; color: #ff4444; font-size: 16px; font-weight: bold;",
                        "/"
                    }
                }
            }
            span {
                style: "font-size: 11px; color: #999;",
                "Fill"
            }

            // Separator
            div {
                style: "width: 1px; height: 18px; background: #555; margin: 0 4px;",
            }

            // Stroke swatch
            div {
                title: if stroke_none { "Stroke: none" } else if stroke_color.is_empty() { "Stroke: none" } else { "Stroke: {stroke_color}" },
                style: "{stroke_style}",
                onclick: move |_| on_stroke_click.call(()),
                if stroke_none || stroke_color.is_empty() {
                    span {
                        style: "display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; color: #ff4444; font-size: 16px; font-weight: bold;",
                        "/"
                    }
                }
            }
            span {
                style: "font-size: 11px; color: #999;",
                "Stroke"
            }

            // Separator
            div {
                style: "width: 1px; height: 18px; background: #555; margin: 0 4px;",
            }

            // "None" button -- sets the active paint to none
            button {
                class: "menu-btn",
                title: "Set to none (no paint)",
                style: "padding: 2px 6px; font-size: 11px;",
                onclick: move |_| on_set_none.call(()),
                "None"
            }
        }
    }
}
