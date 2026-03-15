//! Layers panel component with visibility, lock, opacity, blend mode, and reordering.

use dioxus::prelude::*;
use sivgo_dom::NodeId;

/// Represents a single entry in the layers panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerEntry {
    pub id: NodeId,
    pub label: String,
    pub selected: bool,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    pub blend_mode: String,
}

/// Event fired when a layer property changes.
#[derive(Debug, Clone)]
pub enum LayerAction {
    Select(NodeId),
    ToggleVisibility(NodeId),
    ToggleLock(NodeId),
    SetOpacity(NodeId, f64),
    SetBlendMode(NodeId, String),
    MoveUp(NodeId),
    MoveDown(NodeId),
}

/// A panel displaying the list of layers in the document.
///
/// Each entry shows visibility toggle, lock toggle, label, opacity slider,
/// blend mode dropdown, and up/down reorder buttons.
#[component]
pub fn LayersPanel(layers: Vec<LayerEntry>, on_action: EventHandler<LayerAction>) -> Element {
    let blend_modes = vec![
        "Normal",
        "Multiply",
        "Screen",
        "Overlay",
        "Darken",
        "Lighten",
        "ColorDodge",
        "ColorBurn",
        "HardLight",
        "SoftLight",
        "Difference",
        "Exclusion",
        "Hue",
        "Saturation",
        "Color",
        "Luminosity",
    ];

    rsx! {
        div { class: "layers-panel",
            div { class: "panel-header",
                div { class: "panel-title-wrap",
                    span { class: "panel-kicker", "Structure" }
                    h3 { class: "panel-title", "Layers" }
                    p { class: "panel-meta", "Visibility, locking, blend mode, and stacking order for the document tree." }
                }
                span { class: "panel-badge", "{layers.len()} items" }
            }
            div { class: "layers-list",
                for (idx, entry) in layers.iter().enumerate() {
                    {
                        let entry_id = entry.id;
                        let entry_opacity = entry.opacity;
                        let entry_blend = entry.blend_mode.clone();
                        let visibility_copy = if entry.visible { "Visible" } else { "Hidden" };
                        let edit_copy = if entry.locked { "Locked" } else { "Editable" };
                        let is_first = idx == 0;
                        let is_last = idx == layers.len() - 1;
                        let blend_modes = blend_modes.clone();
                        rsx! {
                            div {
                                class: if entry.selected { "layer-item selected" } else { "layer-item" },

                                // Visibility toggle
                                button {
                                    class: "layer-btn visibility-btn",
                                    title: if entry.visible { "Hide layer" } else { "Show layer" },
                                    onclick: move |_| on_action.call(LayerAction::ToggleVisibility(entry_id)),
                                    if entry.visible { "👁" } else { "—" }
                                }

                                // Lock toggle
                                button {
                                    class: "layer-btn lock-btn",
                                    title: if entry.locked { "Unlock layer" } else { "Lock layer" },
                                    onclick: move |_| on_action.call(LayerAction::ToggleLock(entry_id)),
                                    if entry.locked { "🔒" } else { "🔓" }
                                }

                                // Label (click to select)
                                div {
                                    class: "layer-label-wrap",
                                    onclick: move |_| on_action.call(LayerAction::Select(entry_id)),
                                    span { class: "layer-label", "{entry.label}" }
                                    span { class: "layer-meta", "{edit_copy} / {visibility_copy}" }
                                }

                                // Move up / down buttons
                                button {
                                    class: "layer-btn move-btn",
                                    title: "Move up",
                                    disabled: is_first,
                                    onclick: move |_| on_action.call(LayerAction::MoveUp(entry_id)),
                                    "▲"
                                }
                                button {
                                    class: "layer-btn move-btn",
                                    title: "Move down",
                                    disabled: is_last,
                                    onclick: move |_| on_action.call(LayerAction::MoveDown(entry_id)),
                                    "▼"
                                }

                                // Opacity slider row
                                div { class: "layer-controls",
                                    label { class: "layer-control-label", "Opacity" }
                                    input {
                                        r#type: "range",
                                        class: "layer-opacity-slider",
                                        min: "0",
                                        max: "1",
                                        step: "0.01",
                                        value: "{entry_opacity}",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<f64>() {
                                                on_action.call(LayerAction::SetOpacity(entry_id, val));
                                            }
                                        },
                                    }
                                    span { class: "layer-opacity-value",
                                        "{(entry_opacity * 100.0) as u32}%"
                                    }
                                }

                                // Blend mode dropdown
                                div { class: "layer-controls",
                                    label { class: "layer-control-label", "Blend" }
                                    select {
                                        class: "layer-blend-select",
                                        value: "{entry_blend}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(LayerAction::SetBlendMode(entry_id, evt.value().to_string()));
                                        },
                                        for mode in blend_modes.iter() {
                                            option {
                                                value: "{mode}",
                                                selected: entry_blend == *mode,
                                                "{mode}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if layers.is_empty() {
                    div { class: "layer-empty",
                        span { class: "empty-title", "No layers yet" }
                        span { "As objects and named layers appear in the document, this stack becomes the primary place to manage order and visibility." }
                    }
                }
            }
        }
    }
}
