//! Symbols panel data model (11.4.10).
//!
//! Manages symbol libraries that can be browsed, searched, and inserted
//! into the document.

/// A single symbol entry within a library.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolEntry {
    /// Unique identifier within the library.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Raw SVG markup for the symbol preview.
    pub preview_svg: String,
}

impl SymbolEntry {
    /// Create a new symbol entry.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        preview_svg: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            preview_svg: preview_svg.into(),
        }
    }
}

/// A collection of related symbols loaded from a single source.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLibrary {
    /// Library display name.
    pub name: String,
    /// Path or URI of the source file.
    pub source_file: String,
    /// Symbols belonging to this library.
    pub symbols: Vec<SymbolEntry>,
}

impl SymbolLibrary {
    /// Create an empty library with the given name and source.
    pub fn new(name: impl Into<String>, source_file: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source_file: source_file.into(),
            symbols: Vec::new(),
        }
    }

    /// Return all symbols whose name contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&SymbolEntry> {
        let q = query.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&q))
            .collect()
    }
}

/// State for the Symbols panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolsPanel {
    /// Available symbol libraries.
    pub libraries: Vec<SymbolLibrary>,
    /// Index of the currently active library, or `None` if no library is
    /// selected.
    pub active_library: Option<usize>,
    /// Current search / filter string.
    pub search_filter: String,
}

impl Default for SymbolsPanel {
    fn default() -> Self {
        Self {
            libraries: Vec::new(),
            active_library: None,
            search_filter: String::new(),
        }
    }
}

impl SymbolsPanel {
    /// Simulate loading a symbol library from a file path.
    ///
    /// Actual file I/O is handled at a higher layer; this creates a
    /// placeholder library entry.
    pub fn load_library(&mut self, path: &str) -> usize {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
        let lib = SymbolLibrary::new(name, path);
        self.libraries.push(lib);
        let idx = self.libraries.len() - 1;
        self.active_library = Some(idx);
        idx
    }

    /// Search the active library for symbols matching `query`.
    /// Returns an empty vec when no library is active.
    pub fn search(&self, query: &str) -> Vec<&SymbolEntry> {
        match self.active_library {
            Some(idx) if idx < self.libraries.len() => self.libraries[idx].search(query),
            _ => Vec::new(),
        }
    }

    /// Look up a symbol by id in the active library and return a placeholder
    /// node id.  In a real implementation this would insert the `<use>` element
    /// into the DOM.
    pub fn insert_symbol(&self, symbol_id: &str) -> Option<String> {
        let lib = self.active_library.and_then(|i| self.libraries.get(i))?;
        lib.symbols
            .iter()
            .find(|s| s.id == symbol_id)
            .map(|s| format!("use-{}", s.id))
    }

    /// Return a reference to the currently active library, if any.
    pub fn active(&self) -> Option<&SymbolLibrary> {
        self.active_library.and_then(|i| self.libraries.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_library() -> SymbolLibrary {
        let mut lib = SymbolLibrary::new("Arrows", "/usr/share/symbols/arrows.svg");
        lib.symbols
            .push(SymbolEntry::new("arr-1", "Arrow Right", "<path/>"));
        lib.symbols
            .push(SymbolEntry::new("arr-2", "Arrow Left", "<path/>"));
        lib.symbols
            .push(SymbolEntry::new("star-1", "Star", "<polygon/>"));
        lib
    }

    #[test]
    fn library_search() {
        let lib = sample_library();
        let results = lib.search("arrow");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn library_search_case_insensitive() {
        let lib = sample_library();
        let results = lib.search("STAR");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "star-1");
    }

    #[test]
    fn panel_load_library() {
        let mut panel = SymbolsPanel::default();
        let idx = panel.load_library("/home/user/symbols/basic.svg");
        assert_eq!(idx, 0);
        assert_eq!(panel.active_library, Some(0));
        assert_eq!(panel.libraries[0].name, "basic.svg");
    }

    #[test]
    fn panel_search_no_active() {
        let panel = SymbolsPanel::default();
        assert!(panel.search("anything").is_empty());
    }

    #[test]
    fn panel_search_active() {
        let mut panel = SymbolsPanel::default();
        panel.libraries.push(sample_library());
        panel.active_library = Some(0);
        let results = panel.search("arrow");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn insert_symbol_found() {
        let mut panel = SymbolsPanel::default();
        panel.libraries.push(sample_library());
        panel.active_library = Some(0);
        let node_id = panel.insert_symbol("arr-1");
        assert_eq!(node_id, Some("use-arr-1".to_string()));
    }

    #[test]
    fn insert_symbol_not_found() {
        let mut panel = SymbolsPanel::default();
        panel.libraries.push(sample_library());
        panel.active_library = Some(0);
        assert_eq!(panel.insert_symbol("nonexistent"), None);
    }
}
