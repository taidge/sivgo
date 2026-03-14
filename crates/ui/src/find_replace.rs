//! Find & Replace panel for searching SVG content by text, attribute,
//! or element type and optionally replacing attribute values.

use dioxus::prelude::*;

/// The type of search to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchType {
    /// Search by text content within elements.
    TextContent,
    /// Search by attribute name.
    AttributeName,
    /// Search by attribute value.
    AttributeValue,
    /// Search by element type / tag name.
    ElementType,
}

impl SearchType {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TextContent => "Text content",
            Self::AttributeName => "Attribute name",
            Self::AttributeValue => "Attribute value",
            Self::ElementType => "Element type",
        }
    }
}

/// A find query describing what to search for.
#[derive(Debug, Clone, PartialEq)]
pub struct FindQuery {
    /// The search text or pattern.
    pub text: String,
    /// What kind of search to perform.
    pub search_type: SearchType,
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether to use regex matching.
    pub use_regex: bool,
}

impl Default for FindQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            search_type: SearchType::TextContent,
            case_sensitive: false,
            use_regex: false,
        }
    }
}

/// A single search result.
#[derive(Debug, Clone, PartialEq)]
pub struct FindResult {
    /// Identifier of the matched element.
    pub element_id: String,
    /// Tag name of the matched element.
    pub tag_name: String,
    /// The matched text fragment.
    pub matched_text: String,
}

/// Events emitted by the Find & Replace panel.
#[derive(Debug, Clone)]
pub enum FindReplaceAction {
    SetSearchText(String),
    SetSearchType(SearchType),
    SetCaseSensitive(bool),
    SetUseRegex(bool),
    SetReplaceText(String),
    Find,
    ReplaceNext,
    ReplaceAll,
    SelectResult(String),
}

/// A panel for finding and replacing text content, attribute names/values,
/// or element types within the SVG document.
#[component]
pub fn FindReplacePanel(
    search_text: String,
    search_type: String,
    case_sensitive: bool,
    use_regex: bool,
    replace_text: String,
    results: Vec<FindResult>,
    on_action: EventHandler<FindReplaceAction>,
) -> Element {
    let search_val = search_text.clone();
    let replace_val = replace_text.clone();
    let type_val = search_type.clone();
    let search_types = vec![
        "TextContent",
        "AttributeName",
        "AttributeValue",
        "ElementType",
    ];
    let type_labels = vec![
        "Text content",
        "Attribute name",
        "Attribute value",
        "Element type",
    ];

    rsx! {
        div { class: "find-replace-panel",
            div { class: "panel-header", "Find & Replace" }

            // ---- Search ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Find" }
                div { class: "prop-row",
                    label { class: "prop-label", "Search" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "Search text...",
                        value: "{search_val}",
                        oninput: move |evt: Event<FormData>| {
                            on_action.call(FindReplaceAction::SetSearchText(evt.value().to_string()));
                        },
                    }
                }
                div { class: "prop-row",
                    label { class: "prop-label", "Type" }
                    select {
                        class: "prop-select",
                        value: "{type_val}",
                        onchange: move |evt: Event<FormData>| {
                            let st = match evt.value().as_str() {
                                "AttributeName" => SearchType::AttributeName,
                                "AttributeValue" => SearchType::AttributeValue,
                                "ElementType" => SearchType::ElementType,
                                _ => SearchType::TextContent,
                            };
                            on_action.call(FindReplaceAction::SetSearchType(st));
                        },
                        for (key, lbl) in search_types.iter().zip(type_labels.iter()) {
                            {
                                let is_sel = type_val == *key;
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
                div { class: "prop-row",
                    label { class: "prop-label", "Case" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: case_sensitive,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(FindReplaceAction::SetCaseSensitive(evt.value() == "true"));
                        },
                    }
                    label { class: "prop-label", "Regex" }
                    input {
                        r#type: "checkbox",
                        class: "prop-checkbox",
                        checked: use_regex,
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(FindReplaceAction::SetUseRegex(evt.value() == "true"));
                        },
                    }
                }
                button {
                    class: "action-btn",
                    onclick: move |_| on_action.call(FindReplaceAction::Find),
                    "Find"
                }
            }

            // ---- Replace ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Replace" }
                div { class: "prop-row",
                    label { class: "prop-label", "Replace" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "Replace with...",
                        value: "{replace_val}",
                        oninput: move |evt: Event<FormData>| {
                            on_action.call(FindReplaceAction::SetReplaceText(evt.value().to_string()));
                        },
                    }
                }
                div { class: "prop-row",
                    button {
                        class: "action-btn",
                        onclick: move |_| on_action.call(FindReplaceAction::ReplaceNext),
                        "Replace"
                    }
                    button {
                        class: "action-btn",
                        onclick: move |_| on_action.call(FindReplaceAction::ReplaceAll),
                        "Replace All"
                    }
                }
            }

            // ---- Results ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Results ({results.len()})" }
                div { class: "find-results-list",
                    for result in results.iter() {
                        {
                            let eid = result.element_id.clone();
                            let tag_label = format!("<{}>", result.tag_name);
                            let match_text = result.matched_text.clone();
                            rsx! {
                                div {
                                    class: "find-result-item",
                                    onclick: move |_| on_action.call(FindReplaceAction::SelectResult(eid.clone())),
                                    span { class: "find-result-tag", "{tag_label}" }
                                    span { class: "find-result-match", "{match_text}" }
                                }
                            }
                        }
                    }
                    if results.is_empty() {
                        div { class: "find-results-empty", "No results" }
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
    fn find_query_default() {
        let q = FindQuery::default();
        assert!(q.text.is_empty());
        assert_eq!(q.search_type, SearchType::TextContent);
        assert!(!q.case_sensitive);
        assert!(!q.use_regex);
    }

    #[test]
    fn search_type_labels() {
        assert_eq!(SearchType::TextContent.label(), "Text content");
        assert_eq!(SearchType::AttributeName.label(), "Attribute name");
        assert_eq!(SearchType::AttributeValue.label(), "Attribute value");
        assert_eq!(SearchType::ElementType.label(), "Element type");
    }

    #[test]
    fn find_result_construction() {
        let result = FindResult {
            element_id: "e1".to_string(),
            tag_name: "text".to_string(),
            matched_text: "Hello".to_string(),
        };
        assert_eq!(result.element_id, "e1");
        assert_eq!(result.tag_name, "text");
    }
}
