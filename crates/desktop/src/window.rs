//! Window management and multi-document tab support (12.6, 12.8).
//!
//! Provides lightweight state types for the title bar and a simple tab
//! manager that tracks multiple open documents.

use std::path::Path;

// ---------------------------------------------------------------------------
// Window title
// ---------------------------------------------------------------------------

/// Describes the current state of the main window for title-bar rendering.
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Display title (usually the file name or "Untitled").
    pub title: String,
    /// Whether the document has unsaved modifications.
    pub modified: bool,
    /// Full path on disk, if the document has been saved before.
    pub document_path: Option<String>,
}

impl WindowState {
    /// Convenience constructor for a brand-new, untitled document.
    pub fn untitled() -> Self {
        Self {
            title: "Untitled".to_string(),
            modified: false,
            document_path: None,
        }
    }
}

/// Format a human-friendly window title.
///
/// * Unmodified: `"filename - Sivgo"`
/// * Modified:   `"filename* - Sivgo"`
pub fn format_title(state: &WindowState) -> String {
    let name = if let Some(ref path) = state.document_path {
        Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| state.title.clone())
    } else {
        state.title.clone()
    };

    if state.modified {
        format!("{}* - Sivgo", name)
    } else {
        format!("{} - Sivgo", name)
    }
}

// ---------------------------------------------------------------------------
// Multi-document tabs
// ---------------------------------------------------------------------------

/// Metadata for a single document tab.
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Short display title for the tab.
    pub title: String,
    /// Full path on disk, if known.
    pub path: Option<String>,
    /// Whether the tab has unsaved changes.
    pub modified: bool,
}

impl TabInfo {
    /// Create a new untitled tab.
    pub fn untitled() -> Self {
        Self {
            title: "Untitled".to_string(),
            path: None,
            modified: false,
        }
    }

    /// Create a tab from a file path.
    pub fn from_path(path: &str) -> Self {
        let title = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        Self {
            title,
            path: Some(path.to_string()),
            modified: false,
        }
    }
}

/// Manages a set of open document tabs.
#[derive(Debug)]
pub struct TabManager {
    tabs: Vec<TabInfo>,
    active: usize,
}

impl TabManager {
    /// Create a `TabManager` with a single untitled tab.
    pub fn new() -> Self {
        Self {
            tabs: vec![TabInfo::untitled()],
            active: 0,
        }
    }

    /// Return a slice of all tabs.
    pub fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    /// Return the index of the currently active tab.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Return the currently active `TabInfo`, if any.
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.tabs.get(self.active)
    }

    /// Switch to the tab at `index`. Returns `false` if out of range.
    pub fn switch_to(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active = index;
            true
        } else {
            false
        }
    }

    /// Add a new tab and make it active. Returns its index.
    pub fn add(&mut self, tab: TabInfo) -> usize {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Close the tab at `index`. The active index is adjusted so it stays
    /// valid. Returns `false` if there is only one tab (we never close the
    /// last tab -- the caller should replace it with an untitled tab instead).
    pub fn close(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
        true
    }

    /// Mark the tab at `index` as modified (or not).
    pub fn set_modified(&mut self, index: usize, modified: bool) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.modified = modified;
        }
    }

    /// Return `true` if any tab has unsaved changes.
    pub fn any_modified(&self) -> bool {
        self.tabs.iter().any(|t| t.modified)
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
    fn test_format_title_unmodified() {
        let state = WindowState {
            title: "drawing".to_string(),
            modified: false,
            document_path: Some("/home/user/drawing.svg".to_string()),
        };
        assert_eq!(format_title(&state), "drawing.svg - Sivgo");
    }

    #[test]
    fn test_format_title_modified() {
        let state = WindowState {
            title: "Untitled".to_string(),
            modified: true,
            document_path: None,
        };
        assert_eq!(format_title(&state), "Untitled* - Sivgo");
    }

    #[test]
    fn test_tab_manager_add_and_close() {
        let mut mgr = TabManager::new();
        assert_eq!(mgr.tabs().len(), 1);
        assert_eq!(mgr.active(), 0);

        let idx = mgr.add(TabInfo::from_path("/tmp/a.svg"));
        assert_eq!(idx, 1);
        assert_eq!(mgr.active(), 1);

        // Cannot close the last remaining tab.
        assert!(mgr.close(1)); // close "a.svg"
        assert_eq!(mgr.tabs().len(), 1);
        assert!(!mgr.close(0)); // only one tab left
    }
}
