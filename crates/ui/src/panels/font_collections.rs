//! Font Collections panel data model (11.4.14).
//!
//! Browse, search, filter, and organise fonts into user-defined collections.

/// Font category classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontCategory {
    Serif,
    Sans,
    Mono,
    Display,
    Handwriting,
}

impl FontCategory {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            FontCategory::Serif => "Serif",
            FontCategory::Sans => "Sans-serif",
            FontCategory::Mono => "Monospace",
            FontCategory::Display => "Display",
            FontCategory::Handwriting => "Handwriting",
        }
    }

    /// All categories in display order.
    pub fn all() -> &'static [FontCategory] {
        &[
            FontCategory::Serif,
            FontCategory::Sans,
            FontCategory::Mono,
            FontCategory::Display,
            FontCategory::Handwriting,
        ]
    }
}

/// Information about a single font family.
#[derive(Debug, Clone, PartialEq)]
pub struct FontInfo {
    /// Font family name (e.g. `"Roboto"`).
    pub family: String,
    /// Available style variants (e.g. `["Regular", "Bold", "Italic"]`).
    pub styles: Vec<String>,
    /// Classification category.
    pub category: FontCategory,
}

impl FontInfo {
    /// Create a new font info entry.
    pub fn new(family: impl Into<String>, styles: Vec<String>, category: FontCategory) -> Self {
        Self {
            family: family.into(),
            styles,
            category,
        }
    }
}

/// A user-defined collection (subset) of fonts.
#[derive(Debug, Clone, PartialEq)]
pub struct FontCollection {
    /// Collection display name.
    pub name: String,
    /// Font family names in this collection.
    pub font_families: Vec<String>,
}

impl FontCollection {
    /// Create a new empty collection.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            font_families: Vec::new(),
        }
    }

    /// Return `true` if the collection contains the given family.
    pub fn contains(&self, family: &str) -> bool {
        self.font_families.iter().any(|f| f == family)
    }
}

/// State for the Font Collections panel.
#[derive(Debug, Clone, PartialEq)]
pub struct FontCollectionsPanel {
    /// All system / available fonts.
    pub system_fonts: Vec<FontInfo>,
    /// User-defined font collections.
    pub collections: Vec<FontCollection>,
    /// Current search string.
    pub search_filter: String,
    /// Optional category filter.
    pub category_filter: Option<FontCategory>,
}

impl Default for FontCollectionsPanel {
    fn default() -> Self {
        Self {
            system_fonts: Vec::new(),
            collections: Vec::new(),
            search_filter: String::new(),
            category_filter: None,
        }
    }
}

impl FontCollectionsPanel {
    /// Search system fonts by family name (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&FontInfo> {
        let q = query.to_lowercase();
        self.system_fonts
            .iter()
            .filter(|f| f.family.to_lowercase().contains(&q))
            .collect()
    }

    /// Return fonts matching the current category filter.
    pub fn filter_by_category(&self) -> Vec<&FontInfo> {
        match self.category_filter {
            Some(cat) => self
                .system_fonts
                .iter()
                .filter(|f| f.category == cat)
                .collect(),
            None => self.system_fonts.iter().collect(),
        }
    }

    /// Add a font family to a collection by index.
    /// Returns `true` if the family was added (not already present).
    pub fn add_to_collection(&mut self, collection_index: usize, family: &str) -> bool {
        if let Some(col) = self.collections.get_mut(collection_index) {
            if col.contains(family) {
                return false;
            }
            col.font_families.push(family.to_string());
            true
        } else {
            false
        }
    }

    /// Create a new empty collection and return its index.
    pub fn create_collection(&mut self, name: impl Into<String>) -> usize {
        self.collections.push(FontCollection::new(name));
        self.collections.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fonts() -> Vec<FontInfo> {
        vec![
            FontInfo::new(
                "Roboto",
                vec!["Regular".into(), "Bold".into()],
                FontCategory::Sans,
            ),
            FontInfo::new(
                "Times New Roman",
                vec!["Regular".into()],
                FontCategory::Serif,
            ),
            FontInfo::new("Courier New", vec!["Regular".into()], FontCategory::Mono),
            FontInfo::new("Lobster", vec!["Regular".into()], FontCategory::Display),
        ]
    }

    #[test]
    fn search_by_name() {
        let mut panel = FontCollectionsPanel::default();
        panel.system_fonts = sample_fonts();
        let results = panel.search("rob");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].family, "Roboto");
    }

    #[test]
    fn search_case_insensitive() {
        let mut panel = FontCollectionsPanel::default();
        panel.system_fonts = sample_fonts();
        let results = panel.search("TIMES");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn filter_by_category() {
        let mut panel = FontCollectionsPanel::default();
        panel.system_fonts = sample_fonts();
        panel.category_filter = Some(FontCategory::Sans);
        let results = panel.filter_by_category();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].family, "Roboto");
    }

    #[test]
    fn filter_no_category() {
        let mut panel = FontCollectionsPanel::default();
        panel.system_fonts = sample_fonts();
        panel.category_filter = None;
        let results = panel.filter_by_category();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn create_collection() {
        let mut panel = FontCollectionsPanel::default();
        let idx = panel.create_collection("Favorites");
        assert_eq!(idx, 0);
        assert_eq!(panel.collections[0].name, "Favorites");
    }

    #[test]
    fn add_to_collection() {
        let mut panel = FontCollectionsPanel::default();
        panel.create_collection("My Fonts");
        assert!(panel.add_to_collection(0, "Roboto"));
        assert!(!panel.add_to_collection(0, "Roboto")); // duplicate
        assert_eq!(panel.collections[0].font_families.len(), 1);
    }

    #[test]
    fn add_to_invalid_collection() {
        let mut panel = FontCollectionsPanel::default();
        assert!(!panel.add_to_collection(99, "Roboto"));
    }

    #[test]
    fn category_labels() {
        assert_eq!(FontCategory::Serif.label(), "Serif");
        assert_eq!(FontCategory::all().len(), 5);
    }
}
