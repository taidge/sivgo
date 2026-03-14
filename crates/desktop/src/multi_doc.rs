//! Multi-window / multi-document tab management (12.6).
//!
//! Provides a lightweight `TabManager` that tracks multiple open documents,
//! each represented by a [`DocumentTab`].

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single document tab in the editor.
#[derive(Debug, Clone)]
pub struct DocumentTab {
    /// Unique identifier for this tab.
    pub id: usize,
    /// Display title (usually the file name or "Untitled").
    pub title: String,
    /// Full path on disk, if the document has been saved before.
    pub path: Option<PathBuf>,
    /// Whether the document has unsaved modifications.
    pub modified: bool,
}

/// Manages a set of open document tabs.
#[derive(Debug)]
pub struct TabManager {
    tabs: Vec<DocumentTab>,
    active_tab: usize,
    next_id: usize,
}

impl TabManager {
    /// Create an empty `TabManager` with no tabs.
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_id: 0,
        }
    }

    /// Open a new untitled tab and make it active.
    ///
    /// Returns the id of the newly created tab.
    pub fn new_tab(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(DocumentTab {
            id,
            title: "Untitled".to_string(),
            path: None,
            modified: false,
        });
        self.active_tab = id;
        id
    }

    /// Close the tab with the given `id`.
    ///
    /// Returns `true` if the tab was found and removed, `false` otherwise.
    /// If the closed tab was active, the active tab switches to the nearest
    /// remaining tab.
    pub fn close_tab(&mut self, id: usize) -> bool {
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        self.tabs.remove(pos);

        // Adjust active tab.
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab == id {
            // Pick the tab at the same position, or the last one.
            let fallback_pos = if pos < self.tabs.len() { pos } else { self.tabs.len() - 1 };
            self.active_tab = self.tabs[fallback_pos].id;
        }
        true
    }

    /// Switch the active tab to the one with the given `id`.
    ///
    /// Returns `true` if the tab exists, `false` if not found.
    pub fn switch_tab(&mut self, id: usize) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab = id;
            true
        } else {
            false
        }
    }

    /// Rename the tab with the given `id`.
    ///
    /// Returns `true` if the tab was found, `false` otherwise.
    pub fn rename_tab(&mut self, id: usize, title: String) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.title = title;
            true
        } else {
            false
        }
    }

    /// Mark a tab as modified or unmodified.
    ///
    /// Returns `true` if the tab was found, `false` otherwise.
    pub fn mark_modified(&mut self, id: usize, modified: bool) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.modified = modified;
            true
        } else {
            false
        }
    }

    /// Return a reference to the currently active tab, if any.
    pub fn get_active(&self) -> Option<&DocumentTab> {
        self.tabs.iter().find(|t| t.id == self.active_tab)
    }

    /// Return the total number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Return a slice of all tabs.
    pub fn tabs(&self) -> &[DocumentTab] {
        &self.tabs
    }

    /// Return the id of the active tab.
    pub fn active_tab_id(&self) -> usize {
        self.active_tab
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tab() {
        let mut mgr = TabManager::new();
        assert_eq!(mgr.tab_count(), 0);

        let id = mgr.new_tab();
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_tab_id(), id);

        let tab = mgr.get_active().unwrap();
        assert_eq!(tab.title, "Untitled");
        assert!(tab.path.is_none());
        assert!(!tab.modified);
    }

    #[test]
    fn test_close_tab() {
        let mut mgr = TabManager::new();
        let id1 = mgr.new_tab();
        let id2 = mgr.new_tab();
        assert_eq!(mgr.tab_count(), 2);

        assert!(mgr.close_tab(id1));
        assert_eq!(mgr.tab_count(), 1);
        // Active tab should now be id2.
        assert_eq!(mgr.active_tab_id(), id2);
    }

    #[test]
    fn test_close_active_tab_switches() {
        let mut mgr = TabManager::new();
        let _id1 = mgr.new_tab();
        let id2 = mgr.new_tab();
        let id3 = mgr.new_tab();

        // Active is id3. Close it -> should fall back.
        assert!(mgr.close_tab(id3));
        assert_eq!(mgr.tab_count(), 2);
        // Should switch to a remaining tab.
        assert!(mgr.get_active().is_some());
    }

    #[test]
    fn test_close_nonexistent() {
        let mut mgr = TabManager::new();
        mgr.new_tab();
        assert!(!mgr.close_tab(9999));
    }

    #[test]
    fn test_switch_tab() {
        let mut mgr = TabManager::new();
        let id1 = mgr.new_tab();
        let id2 = mgr.new_tab();

        assert_eq!(mgr.active_tab_id(), id2);
        assert!(mgr.switch_tab(id1));
        assert_eq!(mgr.active_tab_id(), id1);
    }

    #[test]
    fn test_switch_nonexistent() {
        let mut mgr = TabManager::new();
        mgr.new_tab();
        assert!(!mgr.switch_tab(9999));
    }

    #[test]
    fn test_rename_tab() {
        let mut mgr = TabManager::new();
        let id = mgr.new_tab();
        assert!(mgr.rename_tab(id, "My Drawing".to_string()));
        assert_eq!(mgr.get_active().unwrap().title, "My Drawing");
    }

    #[test]
    fn test_rename_nonexistent() {
        let mut mgr = TabManager::new();
        assert!(!mgr.rename_tab(9999, "Gone".to_string()));
    }

    #[test]
    fn test_mark_modified() {
        let mut mgr = TabManager::new();
        let id = mgr.new_tab();
        assert!(!mgr.get_active().unwrap().modified);

        assert!(mgr.mark_modified(id, true));
        assert!(mgr.get_active().unwrap().modified);

        assert!(mgr.mark_modified(id, false));
        assert!(!mgr.get_active().unwrap().modified);
    }

    #[test]
    fn test_mark_modified_nonexistent() {
        let mut mgr = TabManager::new();
        assert!(!mgr.mark_modified(9999, true));
    }

    #[test]
    fn test_tab_count() {
        let mut mgr = TabManager::new();
        assert_eq!(mgr.tab_count(), 0);
        let id1 = mgr.new_tab();
        assert_eq!(mgr.tab_count(), 1);
        mgr.new_tab();
        assert_eq!(mgr.tab_count(), 2);
        mgr.close_tab(id1);
        assert_eq!(mgr.tab_count(), 1);
    }

    #[test]
    fn test_get_active_empty() {
        let mgr = TabManager::new();
        assert!(mgr.get_active().is_none());
    }

    #[test]
    fn test_tabs_slice() {
        let mut mgr = TabManager::new();
        mgr.new_tab();
        mgr.new_tab();
        assert_eq!(mgr.tabs().len(), 2);
    }

    #[test]
    fn test_unique_ids() {
        let mut mgr = TabManager::new();
        let id1 = mgr.new_tab();
        let id2 = mgr.new_tab();
        let id3 = mgr.new_tab();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }
}
