//! Symbols panel for browsing symbol libraries and inserting symbols
//! into the document.

use dioxus::prelude::*;

/// A single symbol entry in a symbol library.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolEntry {
    /// Unique identifier within the library.
    pub id: String,
    /// Human-readable symbol name.
    pub name: String,
    /// Inline SVG markup for the symbol preview.
    pub svg_preview: String,
}

impl SymbolEntry {
    /// Create a new symbol entry.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        svg_preview: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            svg_preview: svg_preview.into(),
        }
    }
}

/// A named symbol library containing multiple symbols.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLibrary {
    pub name: String,
    pub symbols: Vec<SymbolEntry>,
}

/// Events emitted by the Symbols panel.
#[derive(Debug, Clone)]
pub enum SymbolsAction {
    /// Select a different library by name.
    SelectLibrary(String),
    /// Insert a symbol by its id into the document.
    InsertSymbol(String),
    /// Update the search/filter text.
    SetFilter(String),
}

/// A panel for browsing symbol libraries and inserting symbols into
/// the current document.
#[component]
pub fn SymbolsPanel(
    libraries: Vec<String>,
    active_library: String,
    symbols: Vec<SymbolEntry>,
    filter: String,
    on_action: EventHandler<SymbolsAction>,
) -> Element {
    let active_lib_val = active_library.clone();
    let filter_val = filter.clone();

    // Apply client-side filter to symbols.
    let filtered: Vec<&SymbolEntry> = symbols
        .iter()
        .filter(|s| filter.is_empty() || s.name.to_lowercase().contains(&filter.to_lowercase()))
        .collect();

    rsx! {
        div { class: "symbols-panel",
            div { class: "panel-header", "Symbols" }

            // Library selector
            div { class: "prop-row",
                label { class: "prop-label", "Library" }
                select {
                    class: "prop-select",
                    value: "{active_lib_val}",
                    onchange: move |evt: Event<FormData>| {
                        on_action.call(SymbolsAction::SelectLibrary(evt.value().to_string()));
                    },
                    for lib in libraries.iter() {
                        {
                            let is_sel = active_lib_val == *lib;
                            let lib_name = lib.clone();
                            rsx! {
                                option {
                                    value: "{lib_name}",
                                    selected: is_sel,
                                    "{lib_name}"
                                }
                            }
                        }
                    }
                }
            }

            // Search / filter
            div { class: "prop-row",
                input {
                    r#type: "text",
                    class: "prop-input symbols-search",
                    placeholder: "Search symbols...",
                    value: "{filter_val}",
                    oninput: move |evt: Event<FormData>| {
                        on_action.call(SymbolsAction::SetFilter(evt.value().to_string()));
                    },
                }
            }

            // Symbol grid
            div { class: "symbols-grid",
                for sym in filtered.iter() {
                    {
                        let sym_id = sym.id.clone();
                        let sym_name = sym.name.clone();
                        let sym_preview = sym.svg_preview.clone();
                        rsx! {
                            div {
                                key: "{sym_id}",
                                class: "symbol-item",
                                title: "{sym_name}",
                                onclick: move |_| on_action.call(SymbolsAction::InsertSymbol(sym_id.clone())),
                                div {
                                    class: "symbol-preview",
                                    dangerous_inner_html: "{sym_preview}",
                                }
                                span { class: "symbol-name", "{sym_name}" }
                            }
                        }
                    }
                }
                if filtered.is_empty() {
                    div { class: "symbols-empty", "No symbols found" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_entry_new() {
        let sym = SymbolEntry::new("s1", "Arrow", "<svg/>");
        assert_eq!(sym.id, "s1");
        assert_eq!(sym.name, "Arrow");
        assert_eq!(sym.svg_preview, "<svg/>");
    }

    #[test]
    fn symbol_library_filtering() {
        let symbols = vec![
            SymbolEntry::new("1", "Arrow Right", ""),
            SymbolEntry::new("2", "Circle", ""),
            SymbolEntry::new("3", "Arrow Left", ""),
        ];
        let filter = "arrow";
        let filtered: Vec<&SymbolEntry> = symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&filter.to_lowercase()))
            .collect();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "Arrow Right");
        assert_eq!(filtered[1].name, "Arrow Left");
    }
}
