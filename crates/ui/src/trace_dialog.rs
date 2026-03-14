//! Trace Bitmap dialog for converting raster images to vector paths
//! using various tracing modes and parameter controls.

use dioxus::prelude::*;

/// The tracing algorithm to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TracingMode {
    /// Brightness cutoff tracing.
    Brightness,
    /// Edge detection tracing.
    EdgeDetection,
    /// Colour quantisation tracing.
    ColorQuantization,
    /// Centerline (skeleton) tracing.
    Centerline,
}

impl TracingMode {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Brightness => "Brightness cutoff",
            Self::EdgeDetection => "Edge detection",
            Self::ColorQuantization => "Color quantization",
            Self::Centerline => "Centerline",
        }
    }

    /// All available modes.
    pub fn all() -> &'static [TracingMode] {
        &[
            Self::Brightness,
            Self::EdgeDetection,
            Self::ColorQuantization,
            Self::Centerline,
        ]
    }
}

/// Parameters for the tracing operation.
#[derive(Debug, Clone, PartialEq)]
pub struct TracingParams {
    pub mode: TracingMode,
    /// Brightness threshold for brightness mode (0.0 - 1.0).
    pub threshold: f64,
    /// Number of colours for colour quantisation mode.
    pub num_colors: u32,
    /// Path smoothing factor (0.0 - 1.0).
    pub smoothing: f64,
    /// Whether to invert the image before tracing.
    pub invert: bool,
    /// Whether to remove the background.
    pub remove_background: bool,
}

impl Default for TracingParams {
    fn default() -> Self {
        Self {
            mode: TracingMode::Brightness,
            threshold: 0.5,
            num_colors: 8,
            smoothing: 0.5,
            invert: false,
            remove_background: false,
        }
    }
}

/// Events emitted by the Trace Bitmap dialog.
#[derive(Debug, Clone)]
pub enum TraceAction {
    SetMode(TracingMode),
    SetThreshold(f64),
    SetNumColors(u32),
    SetSmoothing(f64),
    SetInvert(bool),
    SetRemoveBackground(bool),
    /// Execute the trace with current parameters.
    Trace,
    /// Close the dialog.
    Close,
}

/// A dialog for tracing raster images to vector paths.
///
/// Provides mode selection (brightness, edge, color, centerline),
/// parameter controls, and a preview area.
#[component]
pub fn TraceDialog(
    mode: String,
    threshold: f64,
    num_colors: u32,
    smoothing: f64,
    invert: bool,
    remove_background: bool,
    preview_data_uri: String,
    on_action: EventHandler<TraceAction>,
) -> Element {
    let mode_val = mode.clone();
    let modes = vec![
        "Brightness",
        "EdgeDetection",
        "ColorQuantization",
        "Centerline",
    ];
    let mode_labels = vec![
        "Brightness cutoff",
        "Edge detection",
        "Color quantization",
        "Centerline",
    ];

    rsx! {
        div { class: "trace-dialog",
            div { class: "panel-header",
                span { "Trace Bitmap" }
                button {
                    class: "dialog-close-btn",
                    title: "Close",
                    onclick: move |_| on_action.call(TraceAction::Close),
                    "X"
                }
            }

            // ---- Mode selection ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Tracing Mode" }
                div { class: "prop-row",
                    label { class: "prop-label", "Mode" }
                    select {
                        class: "prop-select",
                        value: "{mode_val}",
                        onchange: move |evt: Event<FormData>| {
                            let m = match evt.value().as_str() {
                                "EdgeDetection" => TracingMode::EdgeDetection,
                                "ColorQuantization" => TracingMode::ColorQuantization,
                                "Centerline" => TracingMode::Centerline,
                                _ => TracingMode::Brightness,
                            };
                            on_action.call(TraceAction::SetMode(m));
                        },
                        for (key, lbl) in modes.iter().zip(mode_labels.iter()) {
                            {
                                let is_sel = mode_val == *key;
                                rsx! {
                                    option {
                                        value: "{key}",
                                        selected: is_sel,
                                        "{lbl}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- Parameters ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Parameters" }
                div { class: "prop-row",
                    label { class: "prop-label", "Threshold" }
                    input {
                        r#type: "range",
                        class: "prop-slider",
                        min: "0",
                        max: "1",
                        step: "0.01",
                        value: "{threshold}",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TraceAction::SetThreshold(val));
                            }
                        },
                    }
                    span { class: "prop-value", "{(threshold * 100.0) as u32}%" }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Colors" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{num_colors}",
                        min: "2",
                        max: "256",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<u32>() {
                                on_action.call(TraceAction::SetNumColors(val));
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Smoothing" }
                    input {
                        r#type: "range",
                        class: "prop-slider",
                        min: "0",
                        max: "1",
                        step: "0.01",
                        value: "{smoothing}",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TraceAction::SetSmoothing(val));
                            }
                        },
                    }
                    span { class: "prop-value", "{(smoothing * 100.0) as u32}%" }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Invert" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: invert,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(TraceAction::SetInvert(evt.value() == "true"));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Remove BG" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: remove_background,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(TraceAction::SetRemoveBackground(evt.value() == "true"));
                        },
                    }
                }
            }

            // ---- Preview ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Preview" }
                div { class: "trace-preview",
                    if preview_data_uri.is_empty() {
                        div { class: "trace-preview-empty", "No preview available" }
                    } else {
                        img {
                            class: "trace-preview-img",
                            src: "{preview_data_uri}",
                            alt: "Trace preview",
                        }
                    }
                }
            }

            // ---- Actions ----
            div { class: "dialog-actions",
                button {
                    class: "action-btn primary",
                    onclick: move |_| on_action.call(TraceAction::Trace),
                    "Trace"
                }
                button {
                    class: "action-btn",
                    onclick: move |_| on_action.call(TraceAction::Close),
                    "Cancel"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_params_default() {
        let params = TracingParams::default();
        assert_eq!(params.mode, TracingMode::Brightness);
        assert!((params.threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(params.num_colors, 8);
        assert!((params.smoothing - 0.5).abs() < f64::EPSILON);
        assert!(!params.invert);
        assert!(!params.remove_background);
    }

    #[test]
    fn tracing_mode_labels() {
        assert_eq!(TracingMode::Brightness.label(), "Brightness cutoff");
        assert_eq!(TracingMode::EdgeDetection.label(), "Edge detection");
        assert_eq!(TracingMode::ColorQuantization.label(), "Color quantization");
        assert_eq!(TracingMode::Centerline.label(), "Centerline");
        assert_eq!(TracingMode::all().len(), 4);
    }
}
