//! Layers panel component with visibility, lock, opacity, and reordering.

use dioxus::prelude::*;
use sivgo_dom::NodeId;

/// Represents a single entry in the layers panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerEntry {
    pub id: NodeId,
    pub label: String,
    pub depth: usize,
    pub selected: bool,
    pub active: bool,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    pub blend_mode: String,
}

/// Event fired when a layer property changes.
#[derive(Debug, Clone)]
pub enum LayerAction {
    Select(NodeId),
    BeginRename(NodeId),
    Rename(NodeId, String),
    ToggleVisibility(NodeId),
    ToggleLock(NodeId),
    SetOpacity(NodeId, f64),
    SetBlendMode(NodeId, String),
    MoveUp(NodeId),
    MoveDown(NodeId),
}

/// A panel displaying the layer/object stack in the document.
#[component]
pub fn LayersPanel(
    layers: Vec<LayerEntry>,
    editing_id: Option<NodeId>,
    edit_value: String,
    on_action: EventHandler<LayerAction>,
    on_edit_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "layers-panel",
            div { class: "panel-header",
                div { class: "panel-title-wrap",
                    h3 { class: "panel-title", "Layers" }
                }
                span { class: "panel-badge", "{layers.len()}" }
            }
            div { class: "layers-list",
                for (idx, entry) in layers.iter().enumerate() {
                    {
                        let entry_id = entry.id;
                        let entry_opacity = entry.opacity;
                        let entry_blend = entry.blend_mode.clone();
                        let visibility_copy = if entry.visible { "Visible" } else { "Hidden" };
                        let edit_copy = if entry.locked { "Locked" } else { "Editable" };
                        let state_copy = if entry.active {
                            format!("Active - {edit_copy} - {visibility_copy}")
                        } else {
                            format!("{edit_copy} - {visibility_copy}")
                        };
                        let blend_copy = (entry_blend != "Normal").then(|| format!("Blend: {entry_blend}"));
                        let opacity_percent = (entry_opacity * 100.0).round() as u32;
                        let is_first = idx == 0;
                        let is_last = idx == layers.len() - 1;
                        let is_editing = editing_id == Some(entry_id);
                        let current_edit = edit_value.clone();
                        let indent = entry.depth * 14;
                        let label_style = format!("padding-left: {}px;", indent);
                        rsx! {
                            div {
                                key: "{entry_id}",
                                class: if entry.active { "layer-item active" } else if entry.selected { "layer-item selected" } else { "layer-item" },

                                div { class: "layer-main-row",
                                    button {
                                        class: "layer-btn visibility-btn",
                                        title: if entry.visible { "Hide layer" } else { "Show layer" },
                                        onclick: move |_| on_action.call(LayerAction::ToggleVisibility(entry_id)),
                                        {layer_icon(if entry.visible { "eye" } else { "eye-off" })}
                                    }
                                    button {
                                        class: "layer-btn lock-btn",
                                        title: if entry.locked { "Unlock layer" } else { "Lock layer" },
                                        onclick: move |_| on_action.call(LayerAction::ToggleLock(entry_id)),
                                        {layer_icon(if entry.locked { "lock" } else { "unlock" })}
                                    }
                                    div {
                                        class: "layer-label-wrap",
                                        style: "{label_style}",
                                        onclick: move |_| on_action.call(LayerAction::Select(entry_id)),
                                        if is_editing {
                                            input {
                                                r#type: "text",
                                                class: "layer-rename-input",
                                                value: "{current_edit}",
                                                autofocus: true,
                                                onclick: move |evt| evt.stop_propagation(),
                                                oninput: move |evt: Event<FormData>| {
                                                    on_edit_change.call(evt.value().to_string());
                                                },
                                                onkeydown: move |evt: Event<KeyboardData>| {
                                                    if evt.key() == Key::Enter {
                                                        on_action.call(LayerAction::Rename(entry_id, current_edit.clone()));
                                                    }
                                                },
                                            }
                                        } else {
                                            span {
                                                class: "layer-label",
                                                ondoubleclick: move |evt| {
                                                    evt.stop_propagation();
                                                    on_action.call(LayerAction::BeginRename(entry_id));
                                                },
                                                "{entry.label}"
                                            }
                                            span { class: "layer-meta", "{state_copy}" }
                                            if let Some(blend_copy) = blend_copy.clone() {
                                                span { class: "layer-meta layer-blend-meta", "{blend_copy}" }
                                            }
                                        }
                                    }
                                    button {
                                        class: "layer-btn move-btn",
                                        title: "Move up",
                                        disabled: is_first,
                                        onclick: move |_| on_action.call(LayerAction::MoveUp(entry_id)),
                                        {layer_icon("up")}
                                    }
                                    button {
                                        class: "layer-btn move-btn",
                                        title: "Move down",
                                        disabled: is_last,
                                        onclick: move |_| on_action.call(LayerAction::MoveDown(entry_id)),
                                        {layer_icon("down")}
                                    }
                                }

                                div { class: "layer-opacity-row",
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
                                    span { class: "layer-opacity-value", "{opacity_percent}%" }
                                }
                            }
                        }
                    }
                }
                if layers.is_empty() {
                    div { class: "layer-empty",
                        span { class: "empty-title", "No layers" }
                        span { "Add objects to the canvas to populate the layer list." }
                    }
                }
            }
        }
    }
}

fn layer_icon(kind: &'static str) -> Element {
    let paths: &[&str] = match kind {
        "eye" => &[
            "M2 12 C5 6 19 6 22 12 C19 18 5 18 2 12 Z",
            "M12 9 A3 3 0 1 1 12 15 A3 3 0 1 1 12 9 Z",
        ],
        "eye-off" => &[
            "M4 4 L20 20",
            "M3 12 C4.5 9 7 7 10 6",
            "M14 6 C17 6.5 20 8.5 22 12 C20.8 14.2 19 16 16.8 17",
        ],
        "lock" => &["M7 11 V8 A5 5 0 0 1 17 8 V11", "M6 11 H18 V21 H6 Z"],
        "unlock" => &["M7 11 V8 A5 5 0 0 1 16 5", "M6 11 H18 V21 H6 Z"],
        "up" => &["M12 7 L7 12", "M12 7 L17 12", "M12 7 V18"],
        "down" => &["M12 17 L7 12", "M12 17 L17 12", "M12 17 V6"],
        _ => &[],
    };

    rsx! {
        svg {
            class: "layer-icon",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            for path_data in paths {
                path { d: "{path_data}" }
            }
        }
    }
}
