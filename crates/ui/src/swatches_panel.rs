//! Swatches panel displaying a grid of colour swatches from selectable
//! palettes.

use dioxus::prelude::*;

/// A single colour swatch entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchEntry {
    /// RGBA colour value.
    pub color: [u8; 4],
    /// Human-readable name of the colour (e.g. "Cadmium Red").
    pub name: String,
}

impl SwatchEntry {
    /// Create a new swatch entry from RGBA values.
    pub fn new(name: impl Into<String>, r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            color: [r, g, b, a],
            name: name.into(),
        }
    }

    /// Return a CSS hex colour string (e.g. `#ff0000`).
    pub fn to_hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.color[0], self.color[1], self.color[2]
        )
    }
}

/// A named colour palette.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchPalette {
    pub name: String,
    pub swatches: Vec<SwatchEntry>,
}

/// Events emitted by the Swatches panel.
#[derive(Debug, Clone)]
pub enum SwatchesAction {
    /// Select a different palette.
    SelectPalette(String),
    /// Apply a swatch colour as the fill.
    ApplyAsFill(String),
    /// Apply a swatch colour as the stroke.
    ApplyAsStroke(String),
}

/// A panel displaying a grid of colour swatches from a selectable palette.
#[component]
pub fn SwatchesPanel(
    palettes: Vec<String>,
    active_palette: String,
    swatches: Vec<SwatchEntry>,
    on_action: EventHandler<SwatchesAction>,
) -> Element {
    let pal_val = active_palette.clone();

    rsx! {
        div { class: "swatches-panel",
            div { class: "panel-header", "Swatches" }

            // Palette selector
            div { class: "prop-row",
                label { class: "prop-label", "Palette" }
                select {
                    class: "prop-select",
                    value: "{pal_val}",
                    onchange: move |evt: Event<FormData>| {
                        on_action.call(SwatchesAction::SelectPalette(evt.value().to_string()));
                    },
                    for p in palettes.iter() {
                        {
                            let is_sel = pal_val == *p;
                            let p_name = p.clone();
                            rsx! {
                                option {
                                    value: "{p_name}",
                                    selected: is_sel,
                                    "{p_name}"
                                }
                            }
                        }
                    }
                }
            }

            // Swatch grid
            div { class: "swatches-grid",
                for swatch in swatches.iter() {
                    {
                        let hex = swatch.to_hex();
                        let swatch_name = swatch.name.clone();
                        let fill_hex = hex.clone();
                        let stroke_hex = hex.clone();
                        rsx! {
                            div {
                                class: "swatch-item",
                                title: "{swatch_name}",
                                div {
                                    class: "swatch-color",
                                    style: "background-color: {hex};",
                                    onclick: move |_| on_action.call(SwatchesAction::ApplyAsFill(fill_hex.clone())),
                                    oncontextmenu: move |_| on_action.call(SwatchesAction::ApplyAsStroke(stroke_hex.clone())),
                                }
                            }
                        }
                    }
                }
                if swatches.is_empty() {
                    div { class: "swatches-empty", "No swatches" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatch_entry_to_hex() {
        let swatch = SwatchEntry::new("Red", 255, 0, 0, 255);
        assert_eq!(swatch.to_hex(), "#ff0000");
    }

    #[test]
    fn swatch_entry_rgba() {
        let swatch = SwatchEntry::new("Semi-transparent blue", 0, 0, 255, 128);
        assert_eq!(swatch.color, [0, 0, 255, 128]);
        assert_eq!(swatch.name, "Semi-transparent blue");
        assert_eq!(swatch.to_hex(), "#0000ff");
    }

    #[test]
    fn swatch_entry_black() {
        let swatch = SwatchEntry::new("Black", 0, 0, 0, 255);
        assert_eq!(swatch.to_hex(), "#000000");
    }
}
