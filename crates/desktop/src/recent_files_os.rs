//! OS-integrated recent files management (12.7).
//!
//! Wraps [`sivgo_io::recent_files::RecentFiles`] with platform-aware behaviour.
//! On Windows this could update the taskbar jump list; for now it provides a
//! cross-platform in-memory recent files list with optional JSON persistence.

use std::path::{Path, PathBuf};

use sivgo_io::recent_files::RecentFiles;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of entries kept in the recent files list.
const MAX_RECENT: usize = 20;

/// Name of the JSON persistence file.
const RECENT_FILE_NAME: &str = "recent_files.json";

// ---------------------------------------------------------------------------
// OsRecentFiles
// ---------------------------------------------------------------------------

/// OS-integrated recent files manager.
///
/// Internally delegates to [`sivgo_io::recent_files::RecentFiles`] for storage
/// and ordering, and adds optional disk persistence.
pub struct OsRecentFiles {
    inner: RecentFiles,
    /// Path to the JSON file used for persistence (if any).
    persist_path: Option<PathBuf>,
}

impl OsRecentFiles {
    /// Create a new `OsRecentFiles` manager.
    ///
    /// If `config_dir` is provided, the list will be persisted to
    /// `<config_dir>/recent_files.json` on every mutation.
    pub fn new(config_dir: Option<&Path>) -> Self {
        let persist_path = config_dir.map(|d| d.join(RECENT_FILE_NAME));

        let inner = persist_path
            .as_ref()
            .and_then(|p| RecentFiles::load_from_file(p).ok())
            .unwrap_or_else(|| RecentFiles::new(MAX_RECENT));

        Self {
            inner,
            persist_path,
        }
    }

    /// Record a file as recently opened.
    ///
    /// The entry is moved to the front of the list.  If a persistence path
    /// was configured, the updated list is written to disk.
    pub fn add_recent(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string();
        self.inner.add(path_str, None);
        self.persist();
    }

    /// Return the list of recent file paths, most-recently-opened first.
    pub fn get_recent(&self) -> Vec<PathBuf> {
        self.inner
            .entries()
            .iter()
            .map(|entry| PathBuf::from(&entry.path))
            .collect()
    }

    /// Remove a specific path from the recent list.
    pub fn remove(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string();
        self.inner.remove(&path_str);
        self.persist();
    }

    /// Clear all recent entries.
    pub fn clear(&mut self) {
        self.inner.clear();
        self.persist();
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.inner.entries().len()
    }

    /// Return `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.entries().is_empty()
    }

    // -- internal -----------------------------------------------------------

    /// Best-effort write to disk.
    fn persist(&self) {
        if let Some(ref path) = self.persist_path {
            // Ensure parent directory exists.
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = self.inner.save_to_file(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_recent() {
        let mut mgr = OsRecentFiles::new(None);
        mgr.add_recent(Path::new("/tmp/a.svg"));
        mgr.add_recent(Path::new("/tmp/b.svg"));

        let recent = mgr.get_recent();
        assert_eq!(recent.len(), 2);
        // Most recent first.
        assert_eq!(recent[0], PathBuf::from("/tmp/b.svg"));
        assert_eq!(recent[1], PathBuf::from("/tmp/a.svg"));
    }

    #[test]
    fn test_duplicate_moves_to_front() {
        let mut mgr = OsRecentFiles::new(None);
        mgr.add_recent(Path::new("/tmp/a.svg"));
        mgr.add_recent(Path::new("/tmp/b.svg"));
        mgr.add_recent(Path::new("/tmp/a.svg"));

        let recent = mgr.get_recent();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], PathBuf::from("/tmp/a.svg"));
    }

    #[test]
    fn test_remove() {
        let mut mgr = OsRecentFiles::new(None);
        mgr.add_recent(Path::new("/tmp/a.svg"));
        mgr.add_recent(Path::new("/tmp/b.svg"));
        mgr.remove(Path::new("/tmp/a.svg"));

        let recent = mgr.get_recent();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0], PathBuf::from("/tmp/b.svg"));
    }

    #[test]
    fn test_clear() {
        let mut mgr = OsRecentFiles::new(None);
        mgr.add_recent(Path::new("/tmp/a.svg"));
        mgr.clear();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut mgr = OsRecentFiles::new(None);
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);

        mgr.add_recent(Path::new("/tmp/a.svg"));
        assert!(!mgr.is_empty());
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_persistence_roundtrip() {
        let tmp = std::env::temp_dir().join("sivgo_recent_os_test");
        let _ = std::fs::create_dir_all(&tmp);

        {
            let mut mgr = OsRecentFiles::new(Some(&tmp));
            mgr.add_recent(Path::new("/tmp/persist.svg"));
            assert_eq!(mgr.len(), 1);
        }

        // Re-load from disk.
        {
            let mgr = OsRecentFiles::new(Some(&tmp));
            let recent = mgr.get_recent();
            assert_eq!(recent.len(), 1);
            assert_eq!(recent[0], PathBuf::from("/tmp/persist.svg"));
        }

        // Clean up.
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
