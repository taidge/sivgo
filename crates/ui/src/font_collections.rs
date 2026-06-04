//! Font Collections panel for browsing, filtering, and previewing
//! available fonts.

use dioxus::prelude::*;

/// A single font entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FontEntry {
    /// Font family name (e.g. "Inter", "Roboto").
    pub family: String,
    /// Available styles (e.g. ["Regular", "Bold", "Italic"]).
    pub styles: Vec<String>,
    /// Text used for the preview rendering.
    pub preview_text: String,
}

impl FontEntry {
    /// Create a new font entry with default preview text.
    pub fn new(family: impl Into<String>, styles: Vec<String>) -> Self {
        Self {
            family: family.into(),
            styles,
            preview_text: "AaBbCcDd".to_string(),
        }
    }

    /// Returns true if the font family or any style matches the filter string
    /// (case-insensitive).
    pub fn matches_filter(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let lower = filter.to_lowercase();
        self.family.to_lowercase().contains(&lower)
            || self
                .styles
                .iter()
                .any(|s| s.to_lowercase().contains(&lower))
    }
}

/// Events emitted by the Font Collections panel.
#[derive(Debug, Clone)]
pub enum FontCollectionsAction {
    /// Set the search/filter string.
    SetFilter(String),
    /// Select a font family (e.g. to apply to the current text selection).
    SelectFont(String),
    /// Select a specific style of a font.
    SelectFontStyle(String, String),
    /// Update the preview text used for all font entries.
    SetPreviewText(String),
}

/// A panel for browsing available fonts with search and preview.
#[component]
pub fn FontCollectionsPanel(
    fonts: Vec<FontEntry>,
    filter: String,
    preview_text: String,
    on_action: EventHandler<FontCollectionsAction>,
) -> Element {
    let filter_val = filter.clone();
    let preview_val = preview_text.clone();

    let filtered: Vec<&FontEntry> = fonts.iter().filter(|f| f.matches_filter(&filter)).collect();

    rsx! {
        div { class: "font-collections-panel",
            div { class: "panel-header", "Font Collections" }

            // Search
            div { class: "prop-row",
                input {
                    r#type: "text",
                    class: "prop-input font-search",
                    placeholder: "Filter fonts...",
                    value: "{filter_val}",
                    oninput: move |evt: Event<FormData>| {
                        on_action.call(FontCollectionsAction::SetFilter(evt.value().to_string()));
                    },
                }
            }

            // Preview text
            div { class: "prop-row",
                label { class: "prop-label", "Preview" }
                input {
                    r#type: "text",
                    class: "prop-input",
                    value: "{preview_val}",
                    oninput: move |evt: Event<FormData>| {
                        on_action.call(FontCollectionsAction::SetPreviewText(evt.value().to_string()));
                    },
                }
            }

            // Font list
            div { class: "font-list",
                for font in filtered.iter() {
                    {
                        let family = font.family.clone();
                        let family_click = family.clone();
                        let styles = font.styles.clone();
                        let preview = if font.preview_text.is_empty() {
                            preview_text.clone()
                        } else {
                            font.preview_text.clone()
                        };
                        let font_style = format!("font-family: '{}';", family);
                        rsx! {
                            div {
                                key: "{family}",
                                class: "font-item",
                                onclick: move |_| on_action.call(FontCollectionsAction::SelectFont(family_click.clone())),

                                div { class: "font-family-name", "{family}" }
                                div {
                                    class: "font-preview",
                                    style: "{font_style}",
                                    "{preview}"
                                }
                                div { class: "font-styles",
                                    for style in styles.iter() {
                                        {
                                            let fam = family.clone();
                                            let sty = style.clone();
                                            rsx! {
                                                span {
                                                    class: "font-style-tag",
                                                    onclick: move |_| {
                                                        on_action.call(FontCollectionsAction::SelectFontStyle(
                                                            fam.clone(),
                                                            sty.clone(),
                                                        ));
                                                    },
                                                    "{style}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if filtered.is_empty() {
                    div { class: "font-list-empty", "No fonts found" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_entry_new() {
        let entry = FontEntry::new("Inter", vec!["Regular".into(), "Bold".into()]);
        assert_eq!(entry.family, "Inter");
        assert_eq!(entry.styles.len(), 2);
        assert_eq!(entry.preview_text, "AaBbCcDd");
    }

    #[test]
    fn font_entry_matches_filter() {
        let entry = FontEntry::new("Roboto", vec!["Regular".into(), "Italic".into()]);
        assert!(entry.matches_filter(""));
        assert!(entry.matches_filter("rob"));
        assert!(entry.matches_filter("ROBOTO"));
        assert!(entry.matches_filter("italic"));
        assert!(!entry.matches_filter("Arial"));
    }
}
