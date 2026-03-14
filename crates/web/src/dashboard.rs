//! Document dashboard state and data models.
//!
//! Manages the list of documents shown on the user's dashboard, including
//! sorting, filtering, and view mode (grid vs list).

use serde::{Deserialize, Serialize};

/// Sort field for the document list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Title,
    Modified,
    Created,
}

impl Default for SortField {
    fn default() -> Self {
        Self::Modified
    }
}

/// View mode for the document list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Grid,
    List,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Grid
    }
}

/// Summary of a document for display on the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub preview_url: Option<String>,
    /// ISO 8601 timestamp of the last update.
    pub updated_at: String,
    /// Whether the document is shared with other users.
    pub shared: bool,
    /// Display name of the document owner.
    pub owner: String,
}

/// State of the document dashboard.
#[derive(Debug, Clone)]
pub struct DashboardState {
    /// All documents available to the user.
    pub documents: Vec<DocumentSummary>,
    /// Current sort field.
    pub sort_by: SortField,
    /// Current text filter (matched against title and owner).
    pub filter: String,
    /// Whether to display documents as a grid or a list.
    pub view_mode: ViewMode,
    /// The id of the currently selected document, if any.
    pub selected: Option<String>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            sort_by: SortField::default(),
            filter: String::new(),
            view_mode: ViewMode::default(),
            selected: None,
        }
    }
}

impl DashboardState {
    /// Create a new, empty dashboard state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sort the documents in place by the current `sort_by` field.
    pub fn sort(&mut self) {
        match self.sort_by {
            SortField::Title => {
                self.documents
                    .sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            }
            SortField::Modified => {
                self.documents
                    .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            }
            SortField::Created => {
                // For summaries we only have `updated_at`; treat it as
                // a proxy for created order (newest first).
                self.documents
                    .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            }
        }
    }

    /// Return documents whose title or owner matches the given query
    /// (case-insensitive substring match).
    pub fn filter_by(&self, query: &str) -> Vec<&DocumentSummary> {
        let q = query.to_lowercase();
        self.documents
            .iter()
            .filter(|d| {
                d.title.to_lowercase().contains(&q) || d.owner.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Select a document by id.
    pub fn select(&mut self, id: &str) {
        self.selected = Some(id.to_string());
    }

    /// Remove a document from the list by id.
    ///
    /// If the deleted document was selected, the selection is cleared.
    pub fn delete(&mut self, id: &str) {
        self.documents.retain(|d| d.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_docs() -> Vec<DocumentSummary> {
        vec![
            DocumentSummary {
                id: "1".into(),
                title: "Zebra Logo".into(),
                preview_url: Some("/previews/1.png".into()),
                updated_at: "2025-12-01T10:00:00Z".into(),
                shared: false,
                owner: "Alice".into(),
            },
            DocumentSummary {
                id: "2".into(),
                title: "Alpha Icon".into(),
                preview_url: None,
                updated_at: "2025-12-03T10:00:00Z".into(),
                shared: true,
                owner: "Bob".into(),
            },
            DocumentSummary {
                id: "3".into(),
                title: "Beta Banner".into(),
                preview_url: None,
                updated_at: "2025-12-02T10:00:00Z".into(),
                shared: false,
                owner: "Alice".into(),
            },
        ]
    }

    #[test]
    fn default_state() {
        let state = DashboardState::new();
        assert!(state.documents.is_empty());
        assert_eq!(state.sort_by, SortField::Modified);
        assert_eq!(state.view_mode, ViewMode::Grid);
        assert!(state.selected.is_none());
    }

    #[test]
    fn sort_by_title() {
        let mut state = DashboardState {
            documents: sample_docs(),
            sort_by: SortField::Title,
            ..Default::default()
        };
        state.sort();
        assert_eq!(state.documents[0].title, "Alpha Icon");
        assert_eq!(state.documents[1].title, "Beta Banner");
        assert_eq!(state.documents[2].title, "Zebra Logo");
    }

    #[test]
    fn sort_by_modified_newest_first() {
        let mut state = DashboardState {
            documents: sample_docs(),
            sort_by: SortField::Modified,
            ..Default::default()
        };
        state.sort();
        assert_eq!(state.documents[0].id, "2"); // Dec 3
        assert_eq!(state.documents[1].id, "3"); // Dec 2
        assert_eq!(state.documents[2].id, "1"); // Dec 1
    }

    #[test]
    fn filter_by_title() {
        let state = DashboardState {
            documents: sample_docs(),
            ..Default::default()
        };
        let results = state.filter_by("alpha");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "2");
    }

    #[test]
    fn filter_by_owner() {
        let state = DashboardState {
            documents: sample_docs(),
            ..Default::default()
        };
        let results = state.filter_by("alice");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filter_by_empty_returns_all() {
        let state = DashboardState {
            documents: sample_docs(),
            ..Default::default()
        };
        let results = state.filter_by("");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn select_and_delete() {
        let mut state = DashboardState {
            documents: sample_docs(),
            ..Default::default()
        };
        state.select("2");
        assert_eq!(state.selected.as_deref(), Some("2"));

        state.delete("2");
        assert_eq!(state.documents.len(), 2);
        assert!(state.selected.is_none()); // cleared because "2" was selected
    }

    #[test]
    fn delete_non_selected_keeps_selection() {
        let mut state = DashboardState {
            documents: sample_docs(),
            ..Default::default()
        };
        state.select("1");
        state.delete("2");
        assert_eq!(state.documents.len(), 2);
        assert_eq!(state.selected.as_deref(), Some("1"));
    }
}
