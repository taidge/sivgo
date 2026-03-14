//! Editor page state for the web client.
//!
//! Tracks loading, saving, collaboration, and dirty state for the
//! document currently open in the editor.

use serde::{Deserialize, Serialize};

/// State of the web editor page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebEditorState {
    /// The id of the currently loaded document, if any.
    pub document_id: Option<String>,
    /// Whether the document is currently being loaded from the server.
    pub loading: bool,
    /// Whether a save operation is in progress.
    pub saving: bool,
    /// ISO 8601 timestamp of the last successful save.
    pub last_saved: Option<String>,
    /// Whether the editor is connected to the collaboration server.
    pub connected: bool,
    /// Display names or ids of other users editing the same document.
    pub collaborators: Vec<String>,
    /// Whether the document has unsaved changes.
    dirty: bool,
}

impl Default for WebEditorState {
    fn default() -> Self {
        Self {
            document_id: None,
            loading: false,
            saving: false,
            last_saved: None,
            connected: false,
            collaborators: Vec::new(),
            dirty: false,
        }
    }
}

impl WebEditorState {
    /// Create a new, idle editor state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin loading a document by id.
    ///
    /// Sets `loading` to `true` and records the document id.
    pub fn load_document(&mut self, id: &str) {
        self.document_id = Some(id.to_string());
        self.loading = true;
        self.saving = false;
        self.dirty = false;
        self.last_saved = None;
        self.collaborators.clear();
    }

    /// Mark the document load as complete.
    pub fn on_load_complete(&mut self) {
        self.loading = false;
    }

    /// Begin saving the current document.
    ///
    /// Sets `saving` to `true`.
    pub fn save_document(&mut self) {
        if self.document_id.is_some() {
            self.saving = true;
        }
    }

    /// Mark a save as complete, recording the timestamp and clearing dirty.
    pub fn on_save_complete(&mut self, timestamp: &str) {
        self.saving = false;
        self.dirty = false;
        self.last_saved = Some(timestamp.to_string());
    }

    /// Trigger an auto-save if the document is dirty and not currently saving.
    ///
    /// Returns `true` if a save was initiated.
    pub fn auto_save(&mut self) -> bool {
        if self.dirty && !self.saving && self.document_id.is_some() {
            self.saving = true;
            true
        } else {
            false
        }
    }

    /// Mark the document as having unsaved changes.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns `true` if the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Add a collaborator to the session.
    pub fn add_collaborator(&mut self, name: &str) {
        if !self.collaborators.contains(&name.to_string()) {
            self.collaborators.push(name.to_string());
        }
    }

    /// Remove a collaborator from the session.
    pub fn remove_collaborator(&mut self, name: &str) {
        self.collaborators.retain(|c| c != name);
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_idle() {
        let state = WebEditorState::new();
        assert!(state.document_id.is_none());
        assert!(!state.loading);
        assert!(!state.saving);
        assert!(!state.is_dirty());
        assert!(!state.connected);
        assert!(state.collaborators.is_empty());
    }

    #[test]
    fn load_document_sets_loading() {
        let mut state = WebEditorState::new();
        state.load_document("doc-42");
        assert_eq!(state.document_id.as_deref(), Some("doc-42"));
        assert!(state.loading);
        assert!(!state.is_dirty());
    }

    #[test]
    fn on_load_complete_clears_loading() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        assert!(!state.loading);
    }

    #[test]
    fn save_document_sets_saving() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        state.save_document();
        assert!(state.saving);
    }

    #[test]
    fn save_document_noop_without_document() {
        let mut state = WebEditorState::new();
        state.save_document();
        assert!(!state.saving);
    }

    #[test]
    fn on_save_complete_clears_dirty() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        state.mark_dirty();
        assert!(state.is_dirty());

        state.save_document();
        state.on_save_complete("2025-12-01T10:00:00Z");
        assert!(!state.saving);
        assert!(!state.is_dirty());
        assert_eq!(state.last_saved.as_deref(), Some("2025-12-01T10:00:00Z"));
    }

    #[test]
    fn auto_save_when_dirty() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        state.mark_dirty();
        assert!(state.auto_save());
        assert!(state.saving);
    }

    #[test]
    fn auto_save_noop_when_clean() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        assert!(!state.auto_save());
    }

    #[test]
    fn auto_save_noop_when_already_saving() {
        let mut state = WebEditorState::new();
        state.load_document("doc-1");
        state.on_load_complete();
        state.mark_dirty();
        state.save_document();
        assert!(!state.auto_save()); // already saving
    }

    #[test]
    fn collaborators() {
        let mut state = WebEditorState::new();
        state.add_collaborator("Alice");
        state.add_collaborator("Bob");
        state.add_collaborator("Alice"); // duplicate
        assert_eq!(state.collaborators.len(), 2);

        state.remove_collaborator("Alice");
        assert_eq!(state.collaborators, vec!["Bob"]);
    }
}
