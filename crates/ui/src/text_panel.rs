//! Text Properties panel with font family, size, weight, style,
//! line height, spacing, and alignment controls.

use dioxus::prelude::*;

/// Events emitted by the text properties panel.
#[derive(Debug, Clone)]
pub enum TextAction {
    SetFontFamily(String),
    SetFontSize(f64),
    SetFontWeight(String),
    SetFontStyle(String),
    SetLineHeight(f64),
    SetLetterSpacing(f64),
    SetWordSpacing(f64),
    SetTextAlign(String),
}

/// A panel for editing text-specific properties of selected text elements.
///
/// Provides controls for font family, size, weight, style, line height,
/// letter spacing, word spacing, and text alignment.
#[component]
pub fn TextPanel(
    font_family: String,
    font_size: f64,
    font_weight: String,
    font_style: String,
    line_height: f64,
    letter_spacing: f64,
    word_spacing: f64,
    text_align: String,
    has_text_selection: bool,
    on_action: EventHandler<TextAction>,
) -> Element {
    if !has_text_selection {
        return rsx! {
            div { class: "text-panel",
                div { class: "panel-header", "Text" }
                div { class: "prop-empty", "No text selected" }
            }
        };
    }

    let font_family_val = font_family.clone();
    let weight_val = font_weight.clone();
    let style_val = font_style.clone();
    let align_val = text_align.clone();

    let font_weights = vec![
        "Normal", "Bold", "100", "200", "300", "400", "500", "600", "700", "800", "900",
    ];
    let font_styles = vec!["Normal", "Italic", "Oblique"];
    let alignments = vec!["Start", "Middle", "End"];

    rsx! {
        div { class: "text-panel",
            div { class: "panel-header", "Text" }

            // ---- Font Family ----
            div { class: "prop-section",
                div { class: "prop-row",
                    label { class: "prop-label", "Font" }
                    input {
                        r#type: "text",
                        class: "prop-input text-font-input",
                        placeholder: "Font family",
                        value: "{font_family_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(TextAction::SetFontFamily(evt.value().to_string()));
                        },
                    }
                }

                // ---- Font Size ----
                div { class: "prop-row",
                    label { class: "prop-label", "Size" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{font_size}",
                        min: "1",
                        max: "1000",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TextAction::SetFontSize(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "px" }
                }

                // ---- Font Weight ----
                div { class: "prop-row",
                    label { class: "prop-label", "Weight" }
                    select {
                        class: "prop-select",
                        value: "{weight_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(TextAction::SetFontWeight(evt.value().to_string()));
                        },
                        for w in font_weights.iter() {
                            {
                                let is_sel = weight_val == *w;
                                rsx! {
                                    option {
                                        value: "{w}",
                                        selected: is_sel,
                                        "{w}"
                                    }
                                }
                            }
                        }
                    }
                }

                // ---- Font Style ----
                div { class: "prop-row",
                    label { class: "prop-label", "Style" }
                    select {
                        class: "prop-select",
                        value: "{style_val}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(TextAction::SetFontStyle(evt.value().to_string()));
                        },
                        for s in font_styles.iter() {
                            {
                                let is_sel = style_val == *s;
                                rsx! {
                                    option {
                                        value: "{s}",
                                        selected: is_sel,
                                        "{s}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- Spacing ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Spacing" }

                div { class: "prop-row",
                    label { class: "prop-label", "Line H" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{line_height}",
                        min: "0",
                        step: "0.1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TextAction::SetLineHeight(val));
                            }
                        },
                    }
                }

                div { class: "prop-row",
                    label { class: "prop-label", "Letter" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{letter_spacing}",
                        step: "0.5",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TextAction::SetLetterSpacing(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "px" }
                }

                div { class: "prop-row",
                    label { class: "prop-label", "Word" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{word_spacing}",
                        step: "0.5",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TextAction::SetWordSpacing(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "px" }
                }
            }

            // ---- Alignment ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Alignment" }
                div { class: "text-align-buttons",
                    for a in alignments.iter() {
                        {
                            let is_active = align_val == *a;
                            let a_str = a.to_string();
                            rsx! {
                                button {
                                    class: if is_active { "text-align-btn active" } else { "text-align-btn" },
                                    title: "Align {a}",
                                    onclick: move |_| {
                                        on_action.call(TextAction::SetTextAlign(a_str.clone()));
                                    },
                                    "{a}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
