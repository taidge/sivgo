//! XML Editor panel showing a tree view of SVG nodes, an attribute list
//! for the selected node, and controls to add/edit/delete attributes and
//! do raw text editing.

use dioxus::prelude::*;
use sivgo_dom::NodeId;

/// Represents a node in the XML tree view.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlTreeNode {
    pub id: NodeId,
    pub tag_name: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// Represents a single attribute on the selected XML node.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlAttribute {
    pub name: String,
    pub value: String,
}

/// Events emitted by the XML editor.
#[derive(Debug, Clone)]
pub enum XmlEditorAction {
    /// Select a node in the tree.
    SelectNode(NodeId),
    /// Toggle expand/collapse of a node.
    ToggleExpand(NodeId),
    /// Add a new attribute to the selected node.
    AddAttribute(String, String),
    /// Edit an existing attribute value.
    EditAttribute(String, String),
    /// Delete an attribute by name.
    DeleteAttribute(String),
    /// Apply raw XML text changes.
    ApplyRawXml(String),
}

/// A panel for inspecting and editing the raw XML structure of the SVG
/// document.
///
/// Features a tree view of XML nodes, an attribute list for the selected
/// node, add/edit/delete attribute controls, and a raw text editor.
#[component]
pub fn XmlEditorPanel(
    nodes: Vec<XmlTreeNode>,
    selected_node: Option<NodeId>,
    attributes: Vec<XmlAttribute>,
    raw_xml: String,
    new_attr_name: String,
    new_attr_value: String,
    on_action: EventHandler<XmlEditorAction>,
    on_new_attr_name_change: EventHandler<String>,
    on_new_attr_value_change: EventHandler<String>,
    on_raw_xml_change: EventHandler<String>,
) -> Element {
    let raw_text = raw_xml.clone();
    let attr_name = new_attr_name.clone();
    let attr_value = new_attr_value.clone();

    rsx! {
        div { class: "xml-editor-panel",
            div { class: "panel-header", "XML Editor" }

            // ---- XML Tree View ----
            div { class: "xml-tree-section",
                div { class: "prop-section-header", "Tree" }
                div { class: "xml-tree",
                    for node in nodes.iter() {
                        {
                            let node_id = node.id;
                            let indent = node.depth * 16;
                            let is_selected = selected_node == Some(node.id);
                            let expand_icon = if node.has_children {
                                if node.expanded { "▼" } else { "▶" }
                            } else {
                                "  "
                            };
                            {
                                let style_str = format!("padding-left: {}px;", indent);
                                let tag_label = format!("<{}>", node.tag_name);
                                rsx! {
                                    div {
                                        class: if is_selected { "xml-node selected" } else { "xml-node" },
                                        style: "{style_str}",

                                        // Expand/collapse toggle
                                        span {
                                            class: "xml-expand-icon",
                                            onclick: move |_| on_action.call(XmlEditorAction::ToggleExpand(node_id)),
                                            "{expand_icon}"
                                        }

                                        // Tag name (click to select)
                                        span {
                                            class: "xml-tag-name",
                                            onclick: move |_| on_action.call(XmlEditorAction::SelectNode(node_id)),
                                            "{tag_label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if nodes.is_empty() {
                        div { class: "xml-empty", "No document" }
                    }
                }
            }

            // ---- Attribute List ----
            if selected_node.is_some() {
                div { class: "xml-attrs-section",
                    div { class: "prop-section-header", "Attributes" }
                    div { class: "xml-attr-list",
                        for attr in attributes.iter() {
                            {
                                let attr_name_key = attr.name.clone();
                                let attr_name_del = attr.name.clone();
                                let attr_current_val = attr.value.clone();
                                rsx! {
                                    div { class: "xml-attr-row",
                                        span { class: "xml-attr-name", "{attr.name}" }
                                        input {
                                            r#type: "text",
                                            class: "xml-attr-value-input",
                                            value: "{attr_current_val}",
                                            onchange: move |evt: Event<FormData>| {
                                                on_action.call(XmlEditorAction::EditAttribute(
                                                    attr_name_key.clone(),
                                                    evt.value().to_string(),
                                                ));
                                            },
                                        }
                                        button {
                                            class: "xml-attr-delete-btn",
                                            title: "Delete attribute",
                                            onclick: move |_| {
                                                on_action.call(XmlEditorAction::DeleteAttribute(
                                                    attr_name_del.clone(),
                                                ));
                                            },
                                            "✕"
                                        }
                                    }
                                }
                            }
                        }
                        if attributes.is_empty() {
                            div { class: "xml-attr-empty", "No attributes" }
                        }
                    }

                    // Add new attribute
                    div { class: "xml-add-attr",
                        input {
                            r#type: "text",
                            class: "xml-new-attr-name",
                            placeholder: "name",
                            value: "{attr_name}",
                            oninput: move |evt: Event<FormData>| {
                                on_new_attr_name_change.call(evt.value().to_string());
                            },
                        }
                        input {
                            r#type: "text",
                            class: "xml-new-attr-value",
                            placeholder: "value",
                            value: "{attr_value}",
                            oninput: move |evt: Event<FormData>| {
                                on_new_attr_value_change.call(evt.value().to_string());
                            },
                        }
                        button {
                            class: "xml-add-attr-btn",
                            title: "Add attribute",
                            onclick: move |_| {
                                if !new_attr_name.is_empty() {
                                    on_action.call(XmlEditorAction::AddAttribute(
                                        new_attr_name.clone(),
                                        new_attr_value.clone(),
                                    ));
                                }
                            },
                            "+"
                        }
                    }
                }
            }

            // ---- Raw XML Editor ----
            div { class: "xml-raw-section",
                div { class: "prop-section-header", "Raw XML" }
                textarea {
                    class: "xml-raw-textarea",
                    rows: "8",
                    value: "{raw_text}",
                    oninput: move |evt: Event<FormData>| {
                        on_raw_xml_change.call(evt.value().to_string());
                    },
                }
                button {
                    class: "xml-apply-btn",
                    onclick: move |_| {
                        on_action.call(XmlEditorAction::ApplyRawXml(raw_xml.clone()));
                    },
                    "Apply"
                }
            }
        }
    }
}
