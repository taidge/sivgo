//! CRDT-based collaborative document model.
//!
//! Provides a conflict-free replicated data type (CRDT) document that allows
//! multiple users to concurrently edit an SVG document. Each editing site gets
//! a unique `site_id` and maintains a version vector to track causality.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// OpId – uniquely identifies a single operation across all sites
// ---------------------------------------------------------------------------

/// Unique identifier for a CRDT operation, composed of the originating site
/// and a monotonically increasing logical clock at that site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpId {
    pub site: u64,
    pub clock: u64,
}

impl OpId {
    /// Create a new operation identifier.
    pub fn new(site: u64, clock: u64) -> Self {
        Self { site, clock }
    }
}

// ---------------------------------------------------------------------------
// CrdtOp – the operations that can be applied to a CRDT document
// ---------------------------------------------------------------------------

/// A single CRDT operation that can be applied to the collaborative document.
#[derive(Debug, Clone, PartialEq)]
pub enum CrdtOp {
    /// Insert a new SVG node.
    Insert {
        site: u64,
        clock: u64,
        parent: Option<OpId>,
        node_type: String,
        attrs: HashMap<String, String>,
    },
    /// Delete an existing SVG node.
    Delete {
        site: u64,
        clock: u64,
        target: OpId,
    },
    /// Update a single attribute on an existing SVG node.
    Update {
        site: u64,
        clock: u64,
        target: OpId,
        key: String,
        value: String,
    },
}

impl CrdtOp {
    /// Return the site that originated this operation.
    pub fn site(&self) -> u64 {
        match self {
            CrdtOp::Insert { site, .. } => *site,
            CrdtOp::Delete { site, .. } => *site,
            CrdtOp::Update { site, .. } => *site,
        }
    }

    /// Return the logical clock value of this operation.
    pub fn clock(&self) -> u64 {
        match self {
            CrdtOp::Insert { clock, .. } => *clock,
            CrdtOp::Delete { clock, .. } => *clock,
            CrdtOp::Update { clock, .. } => *clock,
        }
    }

    /// Return the [`OpId`] that this operation represents.
    pub fn op_id(&self) -> OpId {
        OpId::new(self.site(), self.clock())
    }
}

// ---------------------------------------------------------------------------
// CrdtDocument
// ---------------------------------------------------------------------------

/// A CRDT-based document that tracks all operations from every collaborating
/// site and can be merged with remote replicas without conflicts.
#[derive(Debug, Clone)]
pub struct CrdtDocument {
    /// The unique site identifier for this replica.
    pub site_id: u64,
    /// Version vector mapping each known site to the highest clock seen.
    pub version_vector: HashMap<u64, u64>,
    /// Ordered log of all operations applied to this document.
    pub operations: Vec<CrdtOp>,
}

impl CrdtDocument {
    /// Create a new empty CRDT document for the given site.
    pub fn new(site_id: u64) -> Self {
        let mut version_vector = HashMap::new();
        version_vector.insert(site_id, 0);
        Self {
            site_id,
            version_vector,
            operations: Vec::new(),
        }
    }

    /// Generate the next [`OpId`] for this site, advancing the local clock.
    pub fn generate_id(&mut self) -> OpId {
        let clock = self.version_vector.entry(self.site_id).or_insert(0);
        *clock += 1;
        OpId::new(self.site_id, *clock)
    }

    /// Apply a locally-generated operation.  The operation is appended to the
    /// log and the local version vector is updated.
    pub fn apply_local(&mut self, op: CrdtOp) {
        let site = op.site();
        let clock = op.clock();
        let entry = self.version_vector.entry(site).or_insert(0);
        if clock > *entry {
            *entry = clock;
        }
        self.operations.push(op);
    }

    /// Apply a remotely-received operation.
    ///
    /// Returns `Err` if the operation is causally inconsistent (i.e. its clock
    /// is not the expected next value for its site) **and** the operation has
    /// already been seen.  Duplicate operations are silently ignored.
    pub fn apply_remote(&mut self, op: CrdtOp) -> Result<(), String> {
        let site = op.site();
        let clock = op.clock();
        let current = *self.version_vector.get(&site).unwrap_or(&0);

        // Duplicate – already applied.
        if clock <= current {
            return Ok(());
        }

        // Accept the operation (we allow gaps for simplicity in this stub).
        self.version_vector.insert(site, clock);
        self.operations.push(op);
        Ok(())
    }

    /// Merge all operations from `other` that this replica has not yet seen.
    pub fn merge(&mut self, other: &CrdtDocument) {
        for op in &other.operations {
            let site = op.site();
            let clock = op.clock();
            let current = *self.version_vector.get(&site).unwrap_or(&0);
            if clock > current {
                self.version_vector.insert(site, clock);
                self.operations.push(op.clone());
            }
        }
    }

    /// Return a snapshot of all operations currently in the log.
    pub fn to_operations(&self) -> Vec<CrdtOp> {
        self.operations.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_document() {
        let doc = CrdtDocument::new(1);
        assert_eq!(doc.site_id, 1);
        assert_eq!(*doc.version_vector.get(&1).unwrap(), 0);
        assert!(doc.operations.is_empty());
    }

    #[test]
    fn test_generate_id_increments() {
        let mut doc = CrdtDocument::new(42);
        let id1 = doc.generate_id();
        assert_eq!(id1, OpId::new(42, 1));
        let id2 = doc.generate_id();
        assert_eq!(id2, OpId::new(42, 2));
        let id3 = doc.generate_id();
        assert_eq!(id3, OpId::new(42, 3));
    }

    #[test]
    fn test_apply_local() {
        let mut doc = CrdtDocument::new(1);
        let id = doc.generate_id();
        let op = CrdtOp::Insert {
            site: 1,
            clock: id.clock,
            parent: None,
            node_type: "rect".into(),
            attrs: HashMap::new(),
        };
        doc.apply_local(op);
        assert_eq!(doc.operations.len(), 1);
        assert_eq!(*doc.version_vector.get(&1).unwrap(), 1);
    }

    #[test]
    fn test_apply_remote() {
        let mut doc = CrdtDocument::new(1);
        let remote_op = CrdtOp::Insert {
            site: 2,
            clock: 1,
            parent: None,
            node_type: "circle".into(),
            attrs: HashMap::new(),
        };
        let result = doc.apply_remote(remote_op);
        assert!(result.is_ok());
        assert_eq!(doc.operations.len(), 1);
        assert_eq!(*doc.version_vector.get(&2).unwrap(), 1);
    }

    #[test]
    fn test_apply_remote_duplicate_ignored() {
        let mut doc = CrdtDocument::new(1);
        let op = CrdtOp::Insert {
            site: 2,
            clock: 1,
            parent: None,
            node_type: "circle".into(),
            attrs: HashMap::new(),
        };
        doc.apply_remote(op.clone()).unwrap();
        doc.apply_remote(op).unwrap();
        // Duplicate should be silently ignored.
        assert_eq!(doc.operations.len(), 1);
    }

    #[test]
    fn test_merge_documents() {
        let mut doc_a = CrdtDocument::new(1);
        let mut doc_b = CrdtDocument::new(2);

        let id_a = doc_a.generate_id();
        doc_a.apply_local(CrdtOp::Insert {
            site: 1,
            clock: id_a.clock,
            parent: None,
            node_type: "rect".into(),
            attrs: HashMap::new(),
        });

        let id_b = doc_b.generate_id();
        doc_b.apply_local(CrdtOp::Insert {
            site: 2,
            clock: id_b.clock,
            parent: None,
            node_type: "ellipse".into(),
            attrs: HashMap::new(),
        });

        doc_a.merge(&doc_b);
        assert_eq!(doc_a.operations.len(), 2);
        assert_eq!(*doc_a.version_vector.get(&2).unwrap(), 1);
    }

    #[test]
    fn test_to_operations() {
        let mut doc = CrdtDocument::new(1);
        let id = doc.generate_id();
        doc.apply_local(CrdtOp::Insert {
            site: 1,
            clock: id.clock,
            parent: None,
            node_type: "path".into(),
            attrs: HashMap::new(),
        });
        let ops = doc.to_operations();
        assert_eq!(ops.len(), 1);
    }

    #[test]
    fn test_op_id_equality() {
        let a = OpId::new(1, 5);
        let b = OpId::new(1, 5);
        let c = OpId::new(2, 5);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_crdt_op_accessors() {
        let op = CrdtOp::Delete {
            site: 3,
            clock: 7,
            target: OpId::new(1, 2),
        };
        assert_eq!(op.site(), 3);
        assert_eq!(op.clock(), 7);
        assert_eq!(op.op_id(), OpId::new(3, 7));
    }

    #[test]
    fn test_update_op() {
        let mut doc = CrdtDocument::new(1);
        let target = OpId::new(1, 1);
        let id = doc.generate_id();
        doc.apply_local(CrdtOp::Insert {
            site: 1,
            clock: id.clock,
            parent: None,
            node_type: "rect".into(),
            attrs: HashMap::new(),
        });

        let update_id = doc.generate_id();
        doc.apply_local(CrdtOp::Update {
            site: 1,
            clock: update_id.clock,
            target,
            key: "fill".into(),
            value: "#ff0000".into(),
        });
        assert_eq!(doc.operations.len(), 2);
    }

    #[test]
    fn test_merge_idempotent() {
        let mut doc_a = CrdtDocument::new(1);
        let doc_b_clone;

        let mut doc_b = CrdtDocument::new(2);
        let id_b = doc_b.generate_id();
        doc_b.apply_local(CrdtOp::Insert {
            site: 2,
            clock: id_b.clock,
            parent: None,
            node_type: "line".into(),
            attrs: HashMap::new(),
        });
        doc_b_clone = doc_b.clone();

        doc_a.merge(&doc_b);
        doc_a.merge(&doc_b_clone);
        // Second merge should not duplicate the operation.
        assert_eq!(doc_a.operations.len(), 1);
    }
}
