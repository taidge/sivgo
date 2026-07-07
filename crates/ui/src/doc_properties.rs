//! Document Properties panel with page size, background, orientation,
//! metadata, and grid settings.

use dioxus::prelude::*;

/// Page size preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSizePreset {
    A4,
    A3,
    Letter,
    Legal,
    Custom,
}

impl PageSizePreset {
    /// Returns (width, height) in millimetres for standard presets.
    /// Returns `None` for `Custom`.
    pub fn dimensions_mm(&self) -> Option<(f64, f64)> {
        match self {
            Self::A4 => Some((210.0, 297.0)),
            Self::A3 => Some((297.0, 420.0)),
            Self::Letter => Some((215.9, 279.4)),
            Self::Legal => Some((215.9, 355.6)),
            Self::Custom => None,
        }
    }

    /// All available presets in display order.
    pub fn all() -> &'static [PageSizePreset] {
        &[Self::A4, Self::A3, Self::Letter, Self::Legal, Self::Custom]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::A3 => "A3",
            Self::Letter => "Letter",
            Self::Legal => "Legal",
            Self::Custom => "Custom",
        }
    }
}

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

/// Grid settings for the document.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSettings {
    pub enabled: bool,
    pub spacing_x: f64,
    pub spacing_y: f64,
    pub snap_to_grid: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing_x: 10.0,
            spacing_y: 10.0,
            snap_to_grid: false,
        }
    }
}

/// Document metadata fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentMetadata {
    pub title: String,
    pub author: String,
    pub description: String,
}

/// Events emitted by the Document Properties panel.
#[derive(Debug, Clone)]
pub enum DocPropertiesAction {
    SetPreset(String),
    SetWidth(f64),
    SetHeight(f64),
    SetBackgroundColor(String),
    SetOrientation(PageOrientation),
    SetTitle(String),
    SetAuthor(String),
    SetDescription(String),
    SetGridEnabled(bool),
    SetGridSpacingX(f64),
    SetGridSpacingY(f64),
    SetSnapToGrid(bool),
}

/// A panel for editing document-level properties such as page size,
/// background color, orientation, metadata, and grid settings.
#[component]
pub fn DocPropertiesPanel(
    preset: String,
    width: f64,
    height: f64,
    background_color: String,
    orientation: String,
    title: String,
    author: String,
    description: String,
    grid_enabled: bool,
    grid_spacing_x: f64,
    grid_spacing_y: f64,
    snap_to_grid: bool,
    on_action: EventHandler<DocPropertiesAction>,
) -> Element {
    let presets = ["A4", "A3", "Letter", "Legal", "Custom"];
    let preset_val = preset.clone();
    let bg_val = background_color.clone();
    let orient_val = orientation.clone();
    let title_val = title.clone();
    let author_val = author.clone();
    let desc_val = description.clone();

    rsx! {
        div { class: "doc-properties-panel",
            div { class: "panel-header", "Document Properties" }

            // ---- Page Size ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Page Size" }
                div { class: "prop-row",
                    label { class: "prop-label", "Preset" }
                    select {
                        class: "prop-select",
                        value: "{preset_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetPreset(evt.value().to_string()));
                        },
                        for p in presets.iter() {
                            {
                                let is_sel = preset_val == *p;
                                rsx! {
                                    option {
                                        value: "{p}",
                                        selected: is_sel,
                                        "{p}"
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Width" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{width}",
                        min: "1",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(DocPropertiesAction::SetWidth(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "mm" }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Height" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{height}",
                        min: "1",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(DocPropertiesAction::SetHeight(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "mm" }
                }
            }

            // ---- Background & Orientation ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Appearance" }
                div { class: "prop-row",
                    label { class: "prop-label", "Background" }
                    input {
                        r#type: "color",
                        class: "prop-input color-input",
                        value: "{bg_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetBackgroundColor(evt.value().to_string()));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Orientation" }
                    button {
                        class: if orient_val == "Portrait" { "toggle-btn active" } else { "toggle-btn" },
                        onclick: move |_| on_action.call(DocPropertiesAction::SetOrientation(PageOrientation::Portrait)),
                        "Portrait"
                    }
                    button {
                        class: if orient_val == "Landscape" { "toggle-btn active" } else { "toggle-btn" },
                        onclick: move |_| on_action.call(DocPropertiesAction::SetOrientation(PageOrientation::Landscape)),
                        "Landscape"
                    }
                }
            }

            // ---- Metadata ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Metadata" }
                div { class: "prop-row",
                    label { class: "prop-label", "Title" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "Document title",
                        value: "{title_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetTitle(evt.value().to_string()));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Author" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "Author name",
                        value: "{author_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetAuthor(evt.value().to_string()));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Description" }
                    textarea {
                        class: "prop-textarea",
                        rows: "3",
                        placeholder: "Document description",
                        value: "{desc_val}",
                        oninput: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetDescription(evt.value().to_string()));
                        },
                    }
                }
            }

            // ---- Grid Settings ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Grid" }
                div { class: "prop-row",
                    label { class: "prop-label", "Enabled" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: grid_enabled,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetGridEnabled(evt.value() == "true"));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Spacing X" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{grid_spacing_x}",
                        min: "0.1",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(DocPropertiesAction::SetGridSpacingX(val));
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Spacing Y" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{grid_spacing_y}",
                        min: "0.1",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(DocPropertiesAction::SetGridSpacingY(val));
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Snap" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: snap_to_grid,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(DocPropertiesAction::SetSnapToGrid(evt.value() == "true"));
                        },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_preset_dimensions() {
        let a4 = PageSizePreset::A4;
        let dims = a4.dimensions_mm().unwrap();
        assert!((dims.0 - 210.0).abs() < f64::EPSILON);
        assert!((dims.1 - 297.0).abs() < f64::EPSILON);

        assert!(PageSizePreset::Custom.dimensions_mm().is_none());
    }

    #[test]
    fn grid_settings_default() {
        let grid = GridSettings::default();
        assert!(!grid.enabled);
        assert!((grid.spacing_x - 10.0).abs() < f64::EPSILON);
        assert!((grid.spacing_y - 10.0).abs() < f64::EPSILON);
        assert!(!grid.snap_to_grid);
    }

    #[test]
    fn document_metadata_default() {
        let meta = DocumentMetadata::default();
        assert!(meta.title.is_empty());
        assert!(meta.author.is_empty());
        assert!(meta.description.is_empty());
    }
}
