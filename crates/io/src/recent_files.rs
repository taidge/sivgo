//! Recent files manager for the Sivgo editor.
//!
//! Tracks recently opened files with persistence via a simple JSON file.

use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A single entry in the recent files list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentFileEntry {
    /// Absolute path to the file.
    pub path: String,
    /// Optional human-readable title (e.g. the SVG `<title>` element content).
    pub title: Option<String>,
    /// Unix timestamp (seconds since epoch) when the file was last opened.
    pub last_opened: u64,
}

/// Manages a bounded list of recently opened files, ordered by most recent first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    entries: Vec<RecentFileEntry>,
    max_entries: usize,
}

impl RecentFiles {
    /// Create a new, empty recent files list with the given maximum capacity.
    pub fn new(max: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max,
        }
    }

    /// Add (or re-open) a file.
    ///
    /// If a file with the same `path` already exists it is moved to the front
    /// with an updated timestamp and title.  Otherwise a new entry is prepended.
    /// When the list exceeds `max_entries` the oldest entry is dropped.
    pub fn add(&mut self, path: impl Into<String>, title: Option<String>) {
        let path = path.into();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Remove existing entry for the same path (if any).
        self.entries.retain(|e| e.path != path);

        // Prepend the new / updated entry.
        self.entries.insert(
            0,
            RecentFileEntry {
                path,
                title,
                last_opened: now,
            },
        );

        // Truncate to max capacity.
        self.entries.truncate(self.max_entries);
    }

    /// Remove the entry with the given path, if present.
    pub fn remove(&mut self, path: &str) {
        self.entries.retain(|e| e.path != path);
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return a slice of all entries, most-recently-opened first.
    pub fn entries(&self) -> &[RecentFileEntry] {
        &self.entries
    }

    /// Persist the list to `path` as JSON.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load a previously saved list from `path`.
    pub fn load_from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let rf: RecentFiles = serde_json::from_str(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(rf)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_retrieve_entries() {
        let mut rf = RecentFiles::new(5);
        rf.add("/tmp/a.svg", Some("Drawing A".into()));
        rf.add("/tmp/b.svg", None);

        assert_eq!(rf.entries().len(), 2);
        // Most recent first.
        assert_eq!(rf.entries()[0].path, "/tmp/b.svg");
        assert_eq!(rf.entries()[1].path, "/tmp/a.svg");
        assert_eq!(rf.entries()[1].title.as_deref(), Some("Drawing A"));
    }

    #[test]
    fn duplicate_path_moves_to_front() {
        let mut rf = RecentFiles::new(5);
        rf.add("/tmp/a.svg", Some("Old Title".into()));
        rf.add("/tmp/b.svg", None);
        rf.add("/tmp/a.svg", Some("New Title".into()));

        assert_eq!(rf.entries().len(), 2);
        assert_eq!(rf.entries()[0].path, "/tmp/a.svg");
        assert_eq!(rf.entries()[0].title.as_deref(), Some("New Title"));
    }

    #[test]
    fn max_entries_enforced() {
        let mut rf = RecentFiles::new(3);
        rf.add("/tmp/1.svg", None);
        rf.add("/tmp/2.svg", None);
        rf.add("/tmp/3.svg", None);
        rf.add("/tmp/4.svg", None);

        assert_eq!(rf.entries().len(), 3);
        // Oldest entry ("/tmp/1.svg") should have been dropped.
        assert!(!rf.entries().iter().any(|e| e.path == "/tmp/1.svg"));
    }

    #[test]
    fn remove_and_clear() {
        let mut rf = RecentFiles::new(10);
        rf.add("/a.svg", None);
        rf.add("/b.svg", None);

        rf.remove("/a.svg");
        assert_eq!(rf.entries().len(), 1);
        assert_eq!(rf.entries()[0].path, "/b.svg");

        rf.clear();
        assert!(rf.entries().is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("sivgo_recent_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recent.json");

        let mut rf = RecentFiles::new(5);
        rf.add("/tmp/test.svg", Some("Test".into()));
        rf.save_to_file(&file).unwrap();

        let loaded = RecentFiles::load_from_file(&file).unwrap();
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].path, "/tmp/test.svg");
        assert_eq!(loaded.entries()[0].title.as_deref(), Some("Test"));

        // Clean up.
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }
}
