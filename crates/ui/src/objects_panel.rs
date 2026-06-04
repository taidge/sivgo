//! Objects panel component showing the full object tree with visibility,
//! lock, type icons, selection, and rename support.

use dioxus::prelude::*;
use sivgo_dom::NodeId;

/// Represents a single object in the tree hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectEntry {
    pub id: NodeId,
    pub label: String,
    pub type_name: String,
    pub depth: usize,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
}

/// Editable details for the primary selected object.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDetails {
    pub id: NodeId,
    pub label: String,
    pub type_name: String,
    pub xml_id: String,
}

/// Events emitted by the objects panel.
#[derive(Debug, Clone)]
pub enum ObjectAction {
    /// Single-click: select the object.
    Select(NodeId),
    /// Double-click: begin renaming.
    BeginRename(NodeId),
    /// Commit a rename.
    Rename(NodeId, String),
    /// Commit a serialized SVG id change.
    SetXmlId(NodeId, String),
    /// Start dragging an object row.
    BeginDrag(NodeId),
    /// End object-row dragging.
    EndDrag,
    /// Drop the dragged object onto a target object row.
    DropOn(NodeId),
    /// Drop the dragged object at the document root level.
    DropToRoot,
    /// Toggle visibility of the object.
    ToggleVisibility(NodeId),
    /// Toggle lock state of the object.
    ToggleLock(NodeId),
}

/// Returns a short type icon string for common SVG element types.
fn type_icon(type_name: &str) -> &'static str {
    match type_name {
        "rect" => "[R]",
        "circle" => "[C]",
        "ellipse" => "[E]",
        "line" => "[L]",
        "polyline" => "[PL]",
        "polygon" => "[PG]",
        "path" => "[P]",
        "text" | "tspan" | "textPath" => "[T]",
        "image" => "[I]",
        "g" => "[G]",
        "use" => "[U]",
        "svg" => "[SVG]",
        _ => "[?]",
    }
}

/// A panel showing the full object tree of the SVG document.
///
/// Each row displays: type icon, visibility toggle, lock toggle, and label.
/// Single-click selects; double-click begins renaming.
#[component]
pub fn ObjectsPanel(
    objects: Vec<ObjectEntry>,
    editing_id: Option<NodeId>,
    edit_value: String,
    selected_details: Option<ObjectDetails>,
    xml_id_edit_value: String,
    on_action: EventHandler<ObjectAction>,
    on_edit_change: EventHandler<String>,
    on_xml_id_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "objects-panel",
            div { class: "panel-header", "Objects" }
            div {
                class: "object-root-drop",
                ondragover: move |evt| evt.prevent_default(),
                ondrop: move |evt| {
                    evt.prevent_default();
                    on_action.call(ObjectAction::DropToRoot);
                },
                "Root level"
            }
            if let Some(details) = selected_details.as_ref() {
                {
                    let details_id = details.id;
                    let key_xml_id = xml_id_edit_value.clone();
                    let blur_xml_id = xml_id_edit_value.clone();
                    rsx! {
                        div { class: "prop-section object-properties",
                            div { class: "prop-section-header", "Properties" }
                            div { class: "prop-row",
                                span { class: "prop-label", "Type" }
                                span { class: "prop-value", "{details.type_name}" }
                                span { class: "prop-unit", "" }
                            }
                            div { class: "prop-row",
                                span { class: "prop-label", "Label" }
                                span { class: "prop-value", "{details.label}" }
                                span { class: "prop-unit", "" }
                            }
                            div { class: "prop-row",
                                span { class: "prop-label", "ID" }
                                input {
                                    r#type: "text",
                                    class: "prop-input",
                                    value: "{xml_id_edit_value}",
                                    oninput: move |evt: Event<FormData>| {
                                        on_xml_id_change.call(evt.value().to_string());
                                    },
                                    onkeydown: move |evt: Event<KeyboardData>| {
                                        if evt.key() == Key::Enter {
                                            on_action.call(ObjectAction::SetXmlId(details_id, key_xml_id.clone()));
                                        }
                                    },
                                    onblur: move |_| {
                                        on_action.call(ObjectAction::SetXmlId(details_id, blur_xml_id.clone()));
                                    },
                                }
                                span { class: "prop-unit", "" }
                            }
                        }
                    }
                }
            }
            div { class: "objects-tree",
                for entry in objects.iter() {
                    {
                        let entry_id = entry.id;
                        let indent = entry.depth * 16;
                        let icon = type_icon(&entry.type_name);
                        let is_editing = editing_id == Some(entry.id);
                        let current_edit = edit_value.clone();
                        let style_str = format!("padding-left: {}px;", indent);
                        rsx! {
                            div {
                                key: "{entry_id}",
                                class: if entry.selected { "object-row selected" } else { "object-row" },
                                style: "{style_str}",
                                draggable: "true",
                                ondragstart: move |_| on_action.call(ObjectAction::BeginDrag(entry_id)),
                                ondragend: move |_| on_action.call(ObjectAction::EndDrag),
                                ondragover: move |evt| evt.prevent_default(),
                                ondrop: move |evt| {
                                    evt.prevent_default();
                                    on_action.call(ObjectAction::DropOn(entry_id));
                                },

                                // Type icon
                                span { class: "object-type-icon", "{icon}" }

                                // Visibility toggle
                                button {
                                    class: "object-btn",
                                    title: if entry.visible { "Hide" } else { "Show" },
                                    onclick: move |_| on_action.call(ObjectAction::ToggleVisibility(entry_id)),
                                    if entry.visible { "👁" } else { "—" }
                                }

                                // Lock toggle
                                button {
                                    class: "object-btn",
                                    title: if entry.locked { "Unlock" } else { "Lock" },
                                    onclick: move |_| on_action.call(ObjectAction::ToggleLock(entry_id)),
                                    if entry.locked { "🔒" } else { "🔓" }
                                }

                                // Label or rename input
                                if is_editing {
                                    input {
                                        r#type: "text",
                                        class: "object-rename-input",
                                        value: "{current_edit}",
                                        autofocus: true,
                                        oninput: move |evt: Event<FormData>| {
                                            on_edit_change.call(evt.value().to_string());
                                        },
                                        onkeydown: move |evt: Event<KeyboardData>| {
                                            if evt.key() == Key::Enter {
                                                on_action.call(ObjectAction::Rename(entry_id, current_edit.clone()));
                                            }
                                        },
                                    }
                                } else {
                                    span {
                                        class: "object-label",
                                        onclick: move |_| on_action.call(ObjectAction::Select(entry_id)),
                                        ondoubleclick: move |_| on_action.call(ObjectAction::BeginRename(entry_id)),
                                        "{entry.label}"
                                    }
                                }
                            }
                        }
                    }
                }
                if objects.is_empty() {
                    div { class: "object-empty", "No objects" }
                }
            }
        }
    }
}
