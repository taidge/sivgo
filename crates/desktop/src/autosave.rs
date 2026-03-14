//! Auto-save and crash recovery (12.9).
//!
//! Provides helpers to generate auto-save file paths, discover recovery
//! files, and clean up stale backups.  The actual periodic scheduling is
//! left to the caller (e.g. a Dioxus `use_future` or `tokio::interval`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Configuration for the auto-save subsystem.
#[derive(Debug, Clone)]
pub struct AutosaveManager {
    /// Seconds between auto-saves.
    pub interval_secs: u64,
    /// Directory where recovery files are written.
    pub save_dir: String,
    /// Maximum number of backup files to keep per document.
    pub max_backups: usize,
}

/// A single recovery file found on disk.
#[derive(Debug, Clone)]
pub struct RecoveryFile {
    /// Absolute path to the recovery file.
    pub path: String,
    /// The original document path that was being edited.
    pub original_path: String,
    /// Unix timestamp (seconds) when the recovery file was created.
    pub timestamp: u64,
}

impl AutosaveManager {
    /// Create a new `AutosaveManager` with the given settings.
    pub fn new(interval_secs: u64, save_dir: &str, max_backups: usize) -> Self {
        Self {
            interval_secs,
            save_dir: save_dir.to_string(),
            max_backups,
        }
    }

    /// Return the default auto-save directory (`<config>/sivgo/autosave`).
    ///
    /// Falls back to a temp directory if no config directory is detected.
    pub fn default_save_dir() -> String {
        if let Some(config) = dirs_fallback() {
            let dir = Path::new(&config).join("sivgo").join("autosave");
            dir.to_string_lossy().to_string()
        } else {
            std::env::temp_dir()
                .join("sivgo_autosave")
                .to_string_lossy()
                .to_string()
        }
    }
}

/// Best-effort config directory (no external crate required).
fn dirs_fallback() -> Option<String> {
    // Windows: %APPDATA%, Unix: $XDG_CONFIG_HOME or ~/.config
    std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{}/.config", h)))
}

// ---------------------------------------------------------------------------
// Path generation
// ---------------------------------------------------------------------------

/// Generate a deterministic recovery-file path for a given document path.
///
/// The name embeds a timestamp so multiple backups can coexist.
///
/// Format: `<save_dir>/<sanitised_name>.<timestamp>.sivgo-recovery`
pub fn generate_autosave_path(save_dir: &str, doc_path: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let stem = Path::new(doc_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    // Sanitise: keep only alphanumeric, dash, underscore.
    let safe: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let dir = PathBuf::from(save_dir);
    dir.join(format!("{safe}.{ts}.sivgo-recovery"))
        .to_string_lossy()
        .to_string()
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Scan `save_dir` for `*.sivgo-recovery` files and return them sorted by
/// timestamp (newest first).
pub fn list_recovery_files(save_dir: &str) -> Vec<RecoveryFile> {
    let dir = Path::new(save_dir);
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut files: Vec<RecoveryFile> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if n.ends_with(".sivgo-recovery") => n.to_string(),
                _ => continue,
            };

            // Parse timestamp from the name: <stem>.<ts>.sivgo-recovery
            let parts: Vec<&str> = name
                .trim_end_matches(".sivgo-recovery")
                .rsplitn(2, '.')
                .collect();
            let timestamp: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let original_stem = parts.get(1).unwrap_or(&"unknown");

            files.push(RecoveryFile {
                path: path.to_string_lossy().to_string(),
                original_path: original_stem.to_string(),
                timestamp,
            });
        }
    }

    files.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    files
}

// ---------------------------------------------------------------------------
// Cleanup
// ---------------------------------------------------------------------------

/// Remove recovery files in `save_dir` older than `max_age_secs` seconds.
pub fn cleanup_old(save_dir: &str, max_age_secs: u64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for file in list_recovery_files(save_dir) {
        if file.timestamp > 0 && now.saturating_sub(file.timestamp) > max_age_secs {
            let _ = fs::remove_file(&file.path);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_generate_autosave_path_format() {
        let path = generate_autosave_path("/tmp/sivgo_test", "/home/user/drawing.svg");
        assert!(path.starts_with("/tmp/sivgo_test"));
        assert!(path.ends_with(".sivgo-recovery"));
        assert!(path.contains("drawing"));
    }

    #[test]
    fn test_list_recovery_files_empty_dir() {
        // A non-existent directory should return an empty vec, not an error.
        let files = list_recovery_files("/nonexistent_dir_12345");
        assert!(files.is_empty());
    }

    #[test]
    fn test_list_and_cleanup_recovery_files() {
        let tmp = std::env::temp_dir().join("sivgo_autosave_test");
        let _ = fs::create_dir_all(&tmp);
        let save_dir = tmp.to_string_lossy().to_string();

        // Create a fake recovery file with a very old (but non-zero) timestamp.
        let fake = tmp.join("test.1000.sivgo-recovery");
        let _ = fs::write(&fake, "fake");

        let files = list_recovery_files(&save_dir);
        assert!(!files.is_empty());

        // Cleanup with max_age 1 should remove the file (timestamp 1000 is ancient).
        cleanup_old(&save_dir, 1);
        assert!(!fake.exists());

        // Tidy up the temp directory.
        let _ = fs::remove_dir_all(&tmp);
    }
}
