//! Transform panel with move, scale, rotate, and skew controls.

use dioxus::prelude::*;

/// Events emitted by the transform panel.
#[derive(Debug, Clone)]
pub enum TransformAction {
    SetX(f64),
    SetY(f64),
    SetWidth(f64),
    SetHeight(f64),
    SetScalePercent(f64, f64),
    ToggleLockAspect,
    TogglePercentMode,
    SetRotation(f64),
    SetSkewX(f64),
    SetSkewY(f64),
}

/// A panel for controlling position, scale, rotation, and skew of selected
/// objects.
///
/// Displays numeric inputs for X/Y position, W/H dimensions (with optional
/// percentage mode and aspect-ratio lock), rotation angle, and skew values.
#[component]
pub fn TransformPanel(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rotation: f64,
    skew_x: f64,
    skew_y: f64,
    lock_aspect: bool,
    percent_mode: bool,
    has_selection: bool,
    on_action: EventHandler<TransformAction>,
) -> Element {
    if !has_selection {
        return rsx! {
            div { class: "transform-panel",
                div { class: "panel-header", "Transform" }
                div { class: "prop-empty", "No selection" }
            }
        };
    }

    rsx! {
        div { class: "transform-panel",
            div { class: "panel-header", "Transform" }

            // ---- Move ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Position" }
                div { class: "prop-row",
                    label { class: "prop-label", "X" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{x}",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TransformAction::SetX(val));
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Y" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{y}",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TransformAction::SetY(val));
                            }
                        },
                    }
                }
            }

            // ---- Scale ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Size" }
                div { class: "prop-row",
                    label { class: "prop-label", if percent_mode { "W %" } else { "W" } }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{width}",
                        min: "0",
                        step: if percent_mode { "1" } else { "0.5" },
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                if percent_mode {
                                    on_action.call(TransformAction::SetScalePercent(val, 0.0));
                                } else {
                                    on_action.call(TransformAction::SetWidth(val));
                                }
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", if percent_mode { "H %" } else { "H" } }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{height}",
                        min: "0",
                        step: if percent_mode { "1" } else { "0.5" },
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                if percent_mode {
                                    on_action.call(TransformAction::SetScalePercent(0.0, val));
                                } else {
                                    on_action.call(TransformAction::SetHeight(val));
                                }
                            }
                        },
                    }
                }
                div { class: "prop-row",
                    button {
                        class: if lock_aspect { "transform-toggle active" } else { "transform-toggle" },
                        title: "Lock aspect ratio",
                        onclick: move |_| on_action.call(TransformAction::ToggleLockAspect),
                        if lock_aspect { "🔗 Locked" } else { "🔓 Unlocked" }
                    }
                    button {
                        class: if percent_mode { "transform-toggle active" } else { "transform-toggle" },
                        title: "Toggle percentage mode",
                        onclick: move |_| on_action.call(TransformAction::TogglePercentMode),
                        "% Mode"
                    }
                }
            }

            // ---- Rotate ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Rotate" }
                div { class: "prop-row",
                    label { class: "prop-label", "Angle" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{rotation}",
                        step: "1",
                        min: "-360",
                        max: "360",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TransformAction::SetRotation(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "deg" }
                }
            }

            // ---- Skew ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Skew" }
                div { class: "prop-row",
                    label { class: "prop-label", "Horiz" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{skew_x}",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TransformAction::SetSkewX(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "deg" }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Vert" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{skew_y}",
                        step: "1",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(TransformAction::SetSkewY(val));
                            }
                        },
                    }
                    span { class: "prop-unit", "deg" }
                }
            }
        }
    }
}
