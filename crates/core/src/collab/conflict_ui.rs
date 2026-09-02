//! Conflict resolution UI model.
//!
//! Provides data structures for presenting merge conflicts to the user and
//! recording their resolution choices.  The actual rendering is handled by the
//! UI layer; this module is the underlying model.

use super::crdt::OpId;
use super::ot::ConflictResolution;

// ---------------------------------------------------------------------------
// ConflictInfo
// ---------------------------------------------------------------------------

/// Describes a single conflict between a local and a remote change.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictInfo {
    /// The operation id at the centre of the conflict.
    pub op_id: OpId,
    /// Human-readable description of the local change.
    pub local_change: String,
    /// Human-readable description of the remote change.
    pub remote_change: String,
    /// Unix-epoch timestamp (seconds) when the conflict was detected.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// ConflictResolver
// ---------------------------------------------------------------------------

/// Collects conflicts and their resolutions so the UI can present them and the
/// user (or an auto-resolver) can decide the outcome.
#[derive(Debug)]
pub struct ConflictResolver {
    /// Pending (unresolved) conflicts.
    pub conflicts: Vec<ConflictInfo>,
    /// Already-resolved conflicts with their chosen resolution.
    pub resolved: Vec<(OpId, ConflictResolution)>,
}

impl ConflictResolver {
    /// Create an empty conflict resolver.
    pub fn new() -> Self {
        Self {
            conflicts: Vec::new(),
            resolved: Vec::new(),
        }
    }

    /// Register a new conflict.
    pub fn add_conflict(&mut self, info: ConflictInfo) {
        self.conflicts.push(info);
    }

    /// Resolve a specific conflict (identified by `op_id`) with the given
    /// resolution and move it from the pending list to the resolved list.
    ///
    /// Returns `true` if the conflict was found and resolved, `false` if no
    /// conflict with that `op_id` existed.
    pub fn resolve(&mut self, op_id: OpId, resolution: ConflictResolution) -> bool {
        if let Some(pos) = self.conflicts.iter().position(|c| c.op_id == op_id) {
            self.conflicts.remove(pos);
            self.resolved.push((op_id, resolution));
            true
        } else {
            false
        }
    }

    /// Automatically resolve all pending conflicts by keeping the local
    /// change.
    pub fn auto_resolve_all(&mut self) {
        let pending: Vec<ConflictInfo> = std::mem::take(&mut self.conflicts);
        for info in pending {
            self.resolved
                .push((info.op_id, ConflictResolution::KeepLocal));
        }
    }

    /// Return the number of pending (unresolved) conflicts.
    pub fn pending_count(&self) -> usize {
        self.conflicts.len()
    }
}

impl Default for ConflictResolver {
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

    fn make_conflict(site: u64, clock: u64) -> ConflictInfo {
        ConflictInfo {
            op_id: OpId::new(site, clock),
            local_change: format!("local change at ({}, {})", site, clock),
            remote_change: format!("remote change at ({}, {})", site, clock),
            timestamp: 1000,
        }
    }

    #[test]
    fn test_new_resolver_empty() {
        let cr = ConflictResolver::new();
        assert_eq!(cr.pending_count(), 0);
        assert!(cr.conflicts.is_empty());
        assert!(cr.resolved.is_empty());
    }

    #[test]
    fn test_add_conflict() {
        let mut cr = ConflictResolver::new();
        cr.add_conflict(make_conflict(1, 1));
        assert_eq!(cr.pending_count(), 1);
    }

    #[test]
    fn test_resolve_conflict() {
        let mut cr = ConflictResolver::new();
        let op_id = OpId::new(1, 1);
        cr.add_conflict(make_conflict(1, 1));

        let result = cr.resolve(op_id, ConflictResolution::KeepRemote);
        assert!(result);
        assert_eq!(cr.pending_count(), 0);
        assert_eq!(cr.resolved.len(), 1);
        assert_eq!(cr.resolved[0].0, op_id);
        assert_eq!(cr.resolved[0].1, ConflictResolution::KeepRemote);
    }

    #[test]
    fn test_resolve_nonexistent() {
        let mut cr = ConflictResolver::new();
        let result = cr.resolve(OpId::new(99, 99), ConflictResolution::KeepLocal);
        assert!(!result);
    }

    #[test]
    fn test_auto_resolve_all() {
        let mut cr = ConflictResolver::new();
        cr.add_conflict(make_conflict(1, 1));
        cr.add_conflict(make_conflict(2, 1));
        cr.add_conflict(make_conflict(3, 1));
        assert_eq!(cr.pending_count(), 3);

        cr.auto_resolve_all();
        assert_eq!(cr.pending_count(), 0);
        assert_eq!(cr.resolved.len(), 3);
        for (_, res) in &cr.resolved {
            assert_eq!(*res, ConflictResolution::KeepLocal);
        }
    }

    #[test]
    fn test_auto_resolve_empty() {
        let mut cr = ConflictResolver::new();
        cr.auto_resolve_all();
        assert_eq!(cr.pending_count(), 0);
        assert!(cr.resolved.is_empty());
    }

    #[test]
    fn test_multiple_resolve() {
        let mut cr = ConflictResolver::new();
        cr.add_conflict(make_conflict(1, 1));
        cr.add_conflict(make_conflict(2, 2));

        cr.resolve(OpId::new(1, 1), ConflictResolution::KeepLocal);
        assert_eq!(cr.pending_count(), 1);

        cr.resolve(OpId::new(2, 2), ConflictResolution::KeepRemote);
        assert_eq!(cr.pending_count(), 0);
        assert_eq!(cr.resolved.len(), 2);
    }

    #[test]
    fn test_default_resolver() {
        let cr = ConflictResolver::default();
        assert_eq!(cr.pending_count(), 0);
    }

    #[test]
    fn test_conflict_info_fields() {
        let info = ConflictInfo {
            op_id: OpId::new(5, 10),
            local_change: "set fill to red".into(),
            remote_change: "set fill to blue".into(),
            timestamp: 12345,
        };
        assert_eq!(info.op_id, OpId::new(5, 10));
        assert_eq!(info.local_change, "set fill to red");
        assert_eq!(info.remote_change, "set fill to blue");
        assert_eq!(info.timestamp, 12345);
    }
}
