//! Version history timeline for collaborative documents.
//!
//! Records every meaningful change as a [`VersionEntry`] and allows querying,
//! filtering, and (logically) reverting to a previous version.

use super::crdt::CrdtOp;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ChangeSummary
// ---------------------------------------------------------------------------

/// A brief statistical summary of what changed in a version.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeSummary {
    /// Number of nodes added.
    pub nodes_added: usize,
    /// Number of nodes removed.
    pub nodes_removed: usize,
    /// Number of nodes whose attributes were modified.
    pub nodes_modified: usize,
    /// Names of properties that were changed.
    pub properties_changed: Vec<String>,
}

impl ChangeSummary {
    /// Create an empty change summary.
    pub fn empty() -> Self {
        Self {
            nodes_added: 0,
            nodes_removed: 0,
            nodes_modified: 0,
            properties_changed: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// VersionEntry
// ---------------------------------------------------------------------------

/// A single entry in the version history timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionEntry {
    /// Unique version identifier (e.g. a UUID string).
    pub version_id: String,
    /// Identifier of the user who authored this version.
    pub author_id: String,
    /// Display name of the author.
    pub author_name: String,
    /// Unix-epoch timestamp (seconds) of when this version was created.
    pub timestamp: u64,
    /// Human-readable description of the change.
    pub description: String,
    /// Statistical summary of the change.
    pub change_summary: ChangeSummary,
}

// ---------------------------------------------------------------------------
// VersionTimeline
// ---------------------------------------------------------------------------

/// An ordered timeline of version entries with navigation and filtering.
#[derive(Debug)]
pub struct VersionTimeline {
    /// All version entries, ordered chronologically.
    pub entries: Vec<VersionEntry>,
    /// Index into `entries` pointing at the current "head" version.
    pub current_index: usize,
    /// Exact CRDT operations applied by each version id.
    pub operation_log: HashMap<String, Vec<CrdtOp>>,
    /// Exact inverse operations for rollback by version id.
    pub inverse_log: HashMap<String, Vec<CrdtOp>>,
}

impl VersionTimeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            current_index: 0,
            operation_log: HashMap::new(),
            inverse_log: HashMap::new(),
        }
    }

    /// Append a new entry to the timeline.
    pub fn add_entry(&mut self, entry: VersionEntry) {
        self.operation_log
            .entry(entry.version_id.clone())
            .or_default();
        self.inverse_log
            .entry(entry.version_id.clone())
            .or_default();
        self.entries.push(entry);
        self.current_index = self.entries.len().saturating_sub(1);
    }

    /// Append a new entry with exact forward and inverse operations.
    pub fn add_entry_with_ops(
        &mut self,
        entry: VersionEntry,
        operations: Vec<CrdtOp>,
        inverse_operations: Vec<CrdtOp>,
    ) {
        self.operation_log
            .insert(entry.version_id.clone(), operations);
        self.inverse_log
            .insert(entry.version_id.clone(), inverse_operations);
        self.entries.push(entry);
        self.current_index = self.entries.len().saturating_sub(1);
    }

    /// Return a slice of entries in the given half-open index range
    /// `[start, end)`.  Out-of-bounds indices are clamped.
    pub fn get_range(&self, start: usize, end: usize) -> &[VersionEntry] {
        let s = start.min(self.entries.len());
        let e = end.min(self.entries.len());
        if s >= e {
            &[]
        } else {
            &self.entries[s..e]
        }
    }

    /// Return all entries authored by the given user.
    pub fn get_by_author(&self, author_id: &str) -> Vec<&VersionEntry> {
        self.entries
            .iter()
            .filter(|e| e.author_id == author_id)
            .collect()
    }

    /// Compute the CRDT operations needed to revert to the version with the
    /// given `version_id`.
    ///
    pub fn revert_to(&mut self, version_id: &str) -> Vec<CrdtOp> {
        let target_index = self.entries.iter().position(|e| e.version_id == version_id);

        let target_index = match target_index {
            Some(i) => i,
            None => return Vec::new(),
        };

        // If the target is at or ahead of the current index, nothing to undo.
        if target_index >= self.current_index {
            self.current_index = target_index;
            return Vec::new();
        }

        let mut inverse_ops = Vec::new();
        for entry in self.entries[target_index + 1..=self.current_index]
            .iter()
            .rev()
        {
            if let Some(ops) = self.inverse_log.get(&entry.version_id) {
                inverse_ops.extend(ops.iter().cloned());
            }
        }

        self.current_index = target_index;
        inverse_ops
    }
}

impl Default for VersionTimeline {
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

    fn make_entry(id: &str, author: &str, added: usize, removed: usize) -> VersionEntry {
        VersionEntry {
            version_id: id.to_string(),
            author_id: author.to_string(),
            author_name: author.to_string(),
            timestamp: 1000,
            description: format!("Version {}", id),
            change_summary: ChangeSummary {
                nodes_added: added,
                nodes_removed: removed,
                nodes_modified: 0,
                properties_changed: Vec::new(),
            },
        }
    }

    #[test]
    fn test_new_timeline_empty() {
        let tl = VersionTimeline::new();
        assert!(tl.entries.is_empty());
        assert_eq!(tl.current_index, 0);
    }

    #[test]
    fn test_add_entry() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        assert_eq!(tl.entries.len(), 1);
        assert_eq!(tl.current_index, 0);

        tl.add_entry(make_entry("v2", "bob", 0, 1));
        assert_eq!(tl.entries.len(), 2);
        assert_eq!(tl.current_index, 1);
    }

    #[test]
    fn test_get_range() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        tl.add_entry(make_entry("v2", "bob", 2, 0));
        tl.add_entry(make_entry("v3", "alice", 0, 1));

        let range = tl.get_range(0, 2);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].version_id, "v1");
        assert_eq!(range[1].version_id, "v2");
    }

    #[test]
    fn test_get_range_clamped() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));

        let range = tl.get_range(0, 100);
        assert_eq!(range.len(), 1);

        let range = tl.get_range(5, 10);
        assert!(range.is_empty());
    }

    #[test]
    fn test_get_by_author() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        tl.add_entry(make_entry("v2", "bob", 2, 0));
        tl.add_entry(make_entry("v3", "alice", 0, 1));

        let alice_entries = tl.get_by_author("alice");
        assert_eq!(alice_entries.len(), 2);
        assert_eq!(alice_entries[0].version_id, "v1");
        assert_eq!(alice_entries[1].version_id, "v3");

        let bob_entries = tl.get_by_author("bob");
        assert_eq!(bob_entries.len(), 1);

        let nobody = tl.get_by_author("nobody");
        assert!(nobody.is_empty());
    }

    #[test]
    fn test_revert_to() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        tl.add_entry_with_ops(
            make_entry("v2", "bob", 2, 0),
            vec![CrdtOp::Insert {
                site: 2,
                clock: 1,
                parent: None,
                node_type: "rect".into(),
                attrs: HashMap::new(),
            }],
            vec![CrdtOp::Delete {
                site: 2,
                clock: 2,
                target: crate::collab::crdt::OpId::new(2, 1),
            }],
        );
        tl.add_entry_with_ops(
            make_entry("v3", "alice", 0, 3),
            vec![CrdtOp::Update {
                site: 1,
                clock: 2,
                target: crate::collab::crdt::OpId::new(2, 1),
                key: "fill".into(),
                value: "blue".into(),
            }],
            vec![CrdtOp::Update {
                site: 1,
                clock: 3,
                target: crate::collab::crdt::OpId::new(2, 1),
                key: "fill".into(),
                value: "red".into(),
            }],
        );
        assert_eq!(tl.current_index, 2);

        let ops = tl.revert_to("v1");
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], CrdtOp::Update { ref value, .. } if value == "red"));
        assert!(matches!(ops[1], CrdtOp::Delete { .. }));
        assert_eq!(tl.current_index, 0);
    }

    #[test]
    fn test_revert_to_nonexistent() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        let ops = tl.revert_to("no-such-version");
        assert!(ops.is_empty());
        assert_eq!(tl.current_index, 0); // unchanged
    }

    #[test]
    fn test_revert_to_current_or_ahead() {
        let mut tl = VersionTimeline::new();
        tl.add_entry(make_entry("v1", "alice", 1, 0));
        tl.add_entry(make_entry("v2", "bob", 2, 0));
        assert_eq!(tl.current_index, 1);

        // Revert to current version – no ops needed.
        let ops = tl.revert_to("v2");
        assert!(ops.is_empty());
        assert_eq!(tl.current_index, 1);
    }

    #[test]
    fn test_change_summary_empty() {
        let cs = ChangeSummary::empty();
        assert_eq!(cs.nodes_added, 0);
        assert_eq!(cs.nodes_removed, 0);
        assert_eq!(cs.nodes_modified, 0);
        assert!(cs.properties_changed.is_empty());
    }

    #[test]
    fn test_default_timeline() {
        let tl = VersionTimeline::default();
        assert!(tl.entries.is_empty());
    }

    #[test]
    fn test_version_entry_fields() {
        let entry = VersionEntry {
            version_id: "abc".into(),
            author_id: "user1".into(),
            author_name: "User One".into(),
            timestamp: 42,
            description: "Initial".into(),
            change_summary: ChangeSummary {
                nodes_added: 5,
                nodes_removed: 2,
                nodes_modified: 3,
                properties_changed: vec!["fill".into(), "stroke".into()],
            },
        };
        assert_eq!(entry.version_id, "abc");
        assert_eq!(entry.timestamp, 42);
        assert_eq!(entry.change_summary.nodes_added, 5);
        assert_eq!(entry.change_summary.properties_changed.len(), 2);
    }
}
