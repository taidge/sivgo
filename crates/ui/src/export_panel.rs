//! Export panel for exporting the document or selection to various
//! formats with configurable area and quality settings.

use dioxus::prelude::*;

/// Export file format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Svg,
    Pdf,
}

impl ExportFormat {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Svg => "SVG",
            Self::Pdf => "PDF",
        }
    }

    /// File extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// All available export formats.
    pub fn all() -> &'static [ExportFormat] {
        &[Self::Png, Self::Jpeg, Self::Svg, Self::Pdf]
    }
}

/// What area of the document to export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportArea {
    Page,
    Drawing,
    Selection,
    Custom,
}

impl ExportArea {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Page => "Page",
            Self::Drawing => "Drawing",
            Self::Selection => "Selection",
            Self::Custom => "Custom",
        }
    }
}

/// Export settings state.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub area: ExportArea,
    pub dpi: f64,
    /// JPEG quality (0-100).
    pub quality: u32,
    /// Custom area bounds (x, y, width, height) when area is Custom.
    pub custom_x: f64,
    pub custom_y: f64,
    pub custom_width: f64,
    pub custom_height: f64,
    /// Output file path.
    pub file_path: String,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            area: ExportArea::Page,
            dpi: 96.0,
            quality: 90,
            custom_x: 0.0,
            custom_y: 0.0,
            custom_width: 100.0,
            custom_height: 100.0,
            file_path: String::new(),
        }
    }
}

/// Events emitted by the Export panel.
#[derive(Debug, Clone)]
pub enum ExportAction {
    SetFormat(ExportFormat),
    SetArea(ExportArea),
    SetDpi(f64),
    SetQuality(u32),
    SetCustomX(f64),
    SetCustomY(f64),
    SetCustomWidth(f64),
    SetCustomHeight(f64),
    SetFilePath(String),
    /// Execute the export.
    Export,
}

/// A panel for exporting the document or selection to PNG, JPEG, SVG,
/// or PDF with DPI, quality, and area controls.
#[component]
pub fn ExportPanel(
    format: String,
    area: String,
    dpi: f64,
    quality: u32,
    custom_x: f64,
    custom_y: f64,
    custom_width: f64,
    custom_height: f64,
    file_path: String,
    on_action: EventHandler<ExportAction>,
) -> Element {
    let format_val = format.clone();
    let area_val = area.clone();
    let file_path_val = file_path.clone();
    let formats = vec!["PNG", "JPEG", "SVG", "PDF"];
    let areas = vec!["Page", "Drawing", "Selection", "Custom"];

    rsx! {
        div { class: "export-panel",
            div { class: "panel-header", "Export" }

            // ---- Format ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Format" }
                div { class: "prop-row",
                    label { class: "prop-label", "Format" }
                    select {
                        class: "prop-select",
                        value: "{format_val}",
                        onchange: move |evt: Event<FormData>| {
                            let f = match evt.value().as_str() {
                                "JPEG" => ExportFormat::Jpeg,
                                "SVG" => ExportFormat::Svg,
                                "PDF" => ExportFormat::Pdf,
                                _ => ExportFormat::Png,
                            };
                            on_action.call(ExportAction::SetFormat(f));
                        },
                        for f in formats.iter() {
                            {
                                let is_sel = format_val == *f;
                                rsx! { option { value: "{f}", selected: is_sel, "{f}" } }
                            }
                        }
                    }
                }
            }

            // ---- Area ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Area" }
                div { class: "prop-row",
                    label { class: "prop-label", "Area" }
                    select {
                        class: "prop-select",
                        value: "{area_val}",
                        onchange: move |evt: Event<FormData>| {
                            let a = match evt.value().as_str() {
                                "Drawing" => ExportArea::Drawing,
                                "Selection" => ExportArea::Selection,
                                "Custom" => ExportArea::Custom,
                                _ => ExportArea::Page,
                            };
                            on_action.call(ExportAction::SetArea(a));
                        },
                        for a in areas.iter() {
                            {
                                let is_sel = area_val == *a;
                                rsx! { option { value: "{a}", selected: is_sel, "{a}" } }
                            }
                        }
                    }
                }

                // Custom area controls
                if area_val == "Custom" {
                    div { class: "prop-row",
                        label { class: "prop-label", "X" }
                        input {
                            r#type: "number",
                            class: "prop-input",
                            value: "{custom_x}",
                            step: "1",
                            onchange: move |evt: Event<FormData>| {
                                if let Ok(val) = evt.value().parse::<f64>() {
                                    on_action.call(ExportAction::SetCustomX(val));
                                }
                            },
                        }
                    }
                    div { class: "prop-row",
                        label { class: "prop-label", "Y" }
                        input {
                            r#type: "number",
                            class: "prop-input",
                            value: "{custom_y}",
                            step: "1",
                            onchange: move |evt: Event<FormData>| {
                                if let Ok(val) = evt.value().parse::<f64>() {
                                    on_action.call(ExportAction::SetCustomY(val));
                                }
                            },
                        }
                    }
                    div { class: "prop-row",
                        label { class: "prop-label", "W" }
                        input {
                            r#type: "number",
                            class: "prop-input",
                            value: "{custom_width}",
                            min: "1",
                            step: "1",
                            onchange: move |evt: Event<FormData>| {
                                if let Ok(val) = evt.value().parse::<f64>() {
                                    on_action.call(ExportAction::SetCustomWidth(val));
                                }
                            },
                        }
                    }
                    div { class: "prop-row",
                        label { class: "prop-label", "H" }
                        input {
                            r#type: "number",
                            class: "prop-input",
                            value: "{custom_height}",
                            min: "1",
                            step: "1",
                            onchange: move |evt: Event<FormData>| {
                                if let Ok(val) = evt.value().parse::<f64>() {
                                    on_action.call(ExportAction::SetCustomHeight(val));
                                }
                            },
                        }
                    }
                }
            }

            // ---- Quality ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Quality" }
                div { class: "prop-row",
                    label { class: "prop-label", "DPI" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{dpi}",
                        min: "1",
                        max: "2400",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(ExportAction::SetDpi(val));
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Quality" }
                    input {
                        r#type: "range",
                        class: "prop-slider",
                        min: "0",
                        max: "100",
                        step: "1",
                        value: "{quality}",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<u32>() {
                                on_action.call(ExportAction::SetQuality(val));
                            }
                        },
                    }
                    span { class: "prop-value", "{quality}%" }
                }
            }

            // ---- File path ----
            div { class: "prop-section",
                div { class: "prop-row",
                    label { class: "prop-label", "File" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "Export path...",
                        value: "{file_path_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(ExportAction::SetFilePath(evt.value().to_string()));
                        },
                    }
                }
            }

            // ---- Export button ----
            div { class: "dialog-actions",
                button {
                    class: "action-btn primary",
                    onclick: move |_| on_action.call(ExportAction::Export),
                    "Export"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_extensions() {
        assert_eq!(ExportFormat::Png.extension(), "png");
        assert_eq!(ExportFormat::Jpeg.extension(), "jpg");
        assert_eq!(ExportFormat::Svg.extension(), "svg");
        assert_eq!(ExportFormat::Pdf.extension(), "pdf");
    }

    #[test]
    fn export_settings_default() {
        let settings = ExportSettings::default();
        assert_eq!(settings.format, ExportFormat::Png);
        assert_eq!(settings.area, ExportArea::Page);
        assert!((settings.dpi - 96.0).abs() < f64::EPSILON);
        assert_eq!(settings.quality, 90);
        assert!(settings.file_path.is_empty());
    }
}
