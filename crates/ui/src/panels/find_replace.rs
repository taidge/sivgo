//! Find & Replace panel data model (11.4.11).
//!
//! Search and replace across text content, attribute values, element types,
//! and style properties within the SVG document.

/// Scope of the search operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    /// Search the entire document.
    All,
    /// Search only within the current selection.
    Selection,
    /// Search only within the active layer.
    Layer,
}

impl SearchScope {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SearchScope::All => "All",
            SearchScope::Selection => "Selection",
            SearchScope::Layer => "Current Layer",
        }
    }

    /// All scopes in display order.
    pub fn all() -> &'static [SearchScope] {
        &[SearchScope::All, SearchScope::Selection, SearchScope::Layer]
    }
}

/// What kind of content to search within.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    /// Search text content of elements.
    TextContent,
    /// Search attribute values.
    AttributeValue,
    /// Search element tag names.
    ElementType,
    /// Search CSS / style property values.
    StyleProperty,
}

impl SearchType {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SearchType::TextContent => "Text Content",
            SearchType::AttributeValue => "Attribute Value",
            SearchType::ElementType => "Element Type",
            SearchType::StyleProperty => "Style Property",
        }
    }

    /// All search types in display order.
    pub fn all() -> &'static [SearchType] {
        &[
            SearchType::TextContent,
            SearchType::AttributeValue,
            SearchType::ElementType,
            SearchType::StyleProperty,
        ]
    }
}

/// A single match found during a search.
#[derive(Debug, Clone, PartialEq)]
pub struct FindResult {
    /// Identifier of the node containing the match.
    pub node_id: String,
    /// The matched text.
    pub match_text: String,
    /// Surrounding context (e.g. the full attribute or text run).
    pub context: String,
}

/// Complete state for the Find & Replace panel.
#[derive(Debug, Clone, PartialEq)]
pub struct FindReplaceState {
    /// Text to search for.
    pub search_text: String,
    /// Replacement text.
    pub replace_text: String,
    /// Where to search.
    pub search_scope: SearchScope,
    /// What kind of content to match.
    pub search_type: SearchType,
    /// Whether matching is case-sensitive.
    pub case_sensitive: bool,
    /// Accumulated search results.
    pub results: Vec<FindResult>,
    /// Index of the currently highlighted result, if any.
    pub current_result: Option<usize>,
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self {
            search_text: String::new(),
            replace_text: String::new(),
            search_scope: SearchScope::All,
            search_type: SearchType::TextContent,
            case_sensitive: false,
            results: Vec::new(),
            current_result: None,
        }
    }
}

impl FindReplaceState {
    /// Execute a full search and populate `results`.
    ///
    /// In this data-model layer the method clears old results and prepares
    /// state; the actual DOM traversal is performed by the integration layer.
    pub fn find_all(&mut self) {
        self.results.clear();
        self.current_result = None;
        // Actual search logic is implemented by the integration layer that
        // walks the DOM and calls `push_result`.
    }

    /// Advance to the next result.  Returns the new current index, if any.
    pub fn find_next(&mut self) -> Option<usize> {
        if self.results.is_empty() {
            return None;
        }
        let next = match self.current_result {
            Some(idx) => (idx + 1) % self.results.len(),
            None => 0,
        };
        self.current_result = Some(next);
        Some(next)
    }

    /// Replace the currently highlighted result.  Returns the replaced node id
    /// if a replacement was made.
    pub fn replace_one(&mut self) -> Option<String> {
        let idx = self.current_result?;
        if idx >= self.results.len() {
            return None;
        }
        let node_id = self.results[idx].node_id.clone();
        self.results.remove(idx);
        // Adjust current index.
        if self.results.is_empty() {
            self.current_result = None;
        } else if idx >= self.results.len() {
            self.current_result = Some(0);
        }
        Some(node_id)
    }

    /// Replace all matches.  Returns the number of replacements.
    pub fn replace_all(&mut self) -> usize {
        let count = self.results.len();
        self.results.clear();
        self.current_result = None;
        count
    }

    /// Push a result from an external search pass.
    pub fn push_result(&mut self, result: FindResult) {
        self.results.push(result);
    }

    /// Return the number of results.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results() -> Vec<FindResult> {
        vec![
            FindResult {
                node_id: "t1".into(),
                match_text: "hello".into(),
                context: "hello world".into(),
            },
            FindResult {
                node_id: "t2".into(),
                match_text: "hello".into(),
                context: "say hello".into(),
            },
            FindResult {
                node_id: "t3".into(),
                match_text: "hello".into(),
                context: "hello again".into(),
            },
        ]
    }

    #[test]
    fn default_state() {
        let state = FindReplaceState::default();
        assert!(state.search_text.is_empty());
        assert!(state.results.is_empty());
        assert!(!state.case_sensitive);
        assert_eq!(state.search_scope, SearchScope::All);
        assert_eq!(state.search_type, SearchType::TextContent);
    }

    #[test]
    fn find_next_cycles() {
        let mut state = FindReplaceState::default();
        for r in sample_results() {
            state.push_result(r);
        }
        assert_eq!(state.find_next(), Some(0));
        assert_eq!(state.find_next(), Some(1));
        assert_eq!(state.find_next(), Some(2));
        assert_eq!(state.find_next(), Some(0)); // wraps
    }

    #[test]
    fn find_next_empty() {
        let mut state = FindReplaceState::default();
        assert_eq!(state.find_next(), None);
    }

    #[test]
    fn replace_one() {
        let mut state = FindReplaceState::default();
        for r in sample_results() {
            state.push_result(r);
        }
        state.current_result = Some(1);
        let replaced = state.replace_one();
        assert_eq!(replaced, Some("t2".to_string()));
        assert_eq!(state.result_count(), 2);
    }

    #[test]
    fn replace_all() {
        let mut state = FindReplaceState::default();
        for r in sample_results() {
            state.push_result(r);
        }
        let count = state.replace_all();
        assert_eq!(count, 3);
        assert!(state.results.is_empty());
        assert_eq!(state.current_result, None);
    }

    #[test]
    fn find_all_clears() {
        let mut state = FindReplaceState::default();
        state.push_result(FindResult {
            node_id: "x".into(),
            match_text: "a".into(),
            context: "a".into(),
        });
        state.current_result = Some(0);
        state.find_all();
        assert!(state.results.is_empty());
        assert_eq!(state.current_result, None);
    }

    #[test]
    fn scope_labels() {
        assert_eq!(SearchScope::All.label(), "All");
        assert_eq!(SearchScope::all().len(), 3);
    }

    #[test]
    fn search_type_labels() {
        assert_eq!(SearchType::TextContent.label(), "Text Content");
        assert_eq!(SearchType::all().len(), 4);
    }
}
