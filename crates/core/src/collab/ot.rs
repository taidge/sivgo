//! Operational transformation for concurrent CRDT operations.
//!
//! When two sites produce operations concurrently the OT layer transforms them
//! so they can both be applied in a consistent order.  The functions here work
//! on [`CrdtOp`] values from the [`crdt`](super::crdt) module.

use std::collections::HashMap;

use super::crdt::{CrdtOp, OpId};

// ---------------------------------------------------------------------------
// ConflictResolution
// ---------------------------------------------------------------------------

/// The outcome of a conflict between two concurrent operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// Keep the locally-generated operation and discard the remote one.
    KeepLocal,
    /// Keep the remotely-generated operation and discard the local one.
    KeepRemote,
    /// Merge the two operations into a single combined operation.
    Merge(CrdtOp),
}

// ---------------------------------------------------------------------------
// transform_ops
// ---------------------------------------------------------------------------

/// Transform two concurrent operations so they can be applied in either order
/// and yield the same result.
///
/// Returns `(op1', op2')` where `op1'` is `op1` transformed against `op2` and
/// vice-versa.
pub fn transform_ops(op1: &CrdtOp, op2: &CrdtOp) -> (CrdtOp, CrdtOp) {
    match (op1, op2) {
        (CrdtOp::NoOp { .. }, _) | (_, CrdtOp::NoOp { .. }) => (op1.clone(), op2.clone()),

        // Two inserts – both survive, order by site id.
        (CrdtOp::Insert { .. }, CrdtOp::Insert { .. }) => {
            (op1.clone(), op2.clone())
        }

        // Insert vs Delete – insert wins, delete unaffected.
        (CrdtOp::Insert { .. }, CrdtOp::Delete { .. }) => {
            (op1.clone(), op2.clone())
        }
        (CrdtOp::Delete { .. }, CrdtOp::Insert { .. }) => {
            (op1.clone(), op2.clone())
        }

        (CrdtOp::Delete { target: t1, .. }, CrdtOp::Delete { target: t2, .. }) if t1 == t2 => {
            if op_wins(op1, op2) {
                (op1.clone(), noop_like(op2, "duplicate delete"))
            } else {
                (noop_like(op1, "duplicate delete"), op2.clone())
            }
        }

        // Two updates to the same target & key – higher site wins, the other
        // is transformed to a no-op (we clone as-is for the stub; a real
        // implementation would apply last-writer-wins logic in `apply`).
        (
            CrdtOp::Update {
                target: t1, key: k1, ..
            },
            CrdtOp::Update {
                target: t2, key: k2, ..
            },
        ) if t1 == t2 && k1 == k2 => {
            if op_wins(op1, op2) {
                (op1.clone(), noop_like(op2, "same attribute superseded"))
            } else {
                (noop_like(op1, "same attribute superseded"), op2.clone())
            }
        }

        // Update vs Delete on the same target – delete wins.
        (CrdtOp::Update { target, .. }, CrdtOp::Delete { target: dt, .. })
            if target == dt =>
        {
            (noop_like(op1, "target deleted"), op2.clone())
        }
        (CrdtOp::Delete { target: dt, .. }, CrdtOp::Update { target, .. })
            if dt == target =>
        {
            (op1.clone(), noop_like(op2, "target deleted"))
        }

        // All other combinations – operations are independent, pass through.
        _ => (op1.clone(), op2.clone()),
    }
}

/// Transform a batch of local operations against a batch of remote operations.
pub fn transform_batch(local: &[CrdtOp], remote: &[CrdtOp]) -> (Vec<CrdtOp>, Vec<CrdtOp>) {
    let mut transformed_local = local.to_vec();
    let mut transformed_remote = remote.to_vec();

    for local_op in &mut transformed_local {
        for remote_op in &mut transformed_remote {
            let (left, right) = transform_ops(local_op, remote_op);
            *local_op = left;
            *remote_op = right;
        }
    }

    (transformed_local, transformed_remote)
}

fn op_wins(left: &CrdtOp, right: &CrdtOp) -> bool {
    left.clock() > right.clock() || (left.clock() == right.clock() && left.site() >= right.site())
}

fn noop_like(op: &CrdtOp, reason: &str) -> CrdtOp {
    CrdtOp::NoOp {
        site: op.site(),
        clock: op.clock(),
        reason: reason.to_string(),
    }
}

// ---------------------------------------------------------------------------
// resolve_conflict
// ---------------------------------------------------------------------------

/// Decide how to resolve a conflict between a local and a remote operation.
///
/// Uses a deterministic rule: the operation with the higher `site` id wins.
/// When sites are equal, the higher `clock` wins.
pub fn resolve_conflict(local: &CrdtOp, remote: &CrdtOp) -> ConflictResolution {
    match (local, remote) {
        // Same-key updates on the same target: merge by picking the higher
        // site-id value, falling back to higher clock.
        (
            CrdtOp::Update {
                target: lt,
                key: lk,
                ..
            },
            CrdtOp::Update {
                target: rt,
                key: rk,
                ..
            },
        ) if lt == rt && lk == rk => {
            if local.site() > remote.site() {
                ConflictResolution::KeepLocal
            } else if remote.site() > local.site() {
                ConflictResolution::KeepRemote
            } else if local.clock() >= remote.clock() {
                ConflictResolution::KeepLocal
            } else {
                ConflictResolution::KeepRemote
            }
        }

        // Delete vs Update on same target – delete wins.
        (CrdtOp::Delete { target: dt, .. }, CrdtOp::Update { target: ut, .. })
            if dt == ut =>
        {
            ConflictResolution::KeepLocal
        }
        (CrdtOp::Update { target: ut, .. }, CrdtOp::Delete { target: dt, .. })
            if ut == dt =>
        {
            ConflictResolution::KeepRemote
        }

        // Two updates on same target but different keys – merge both.
        (
            CrdtOp::Update {
                site: ls,
                clock: lc,
                target: lt,
                key: lk,
                value: lv,
            },
            CrdtOp::Update {
                target: rt,
                key: rk,
                value: rv,
                ..
            },
        ) if lt == rt && lk != rk => {
            // Create a merged update that combines both key changes.  We
            // arbitrarily use the local op's metadata and the remote key/value
            // as a second attribute – in a real system this would modify the
            // node to apply both changes.  Here we produce a single Update
            // with a merged value for demonstration purposes.
            let merged_value = format!("{}={};{}={}", lk, lv, rk, rv);
            ConflictResolution::Merge(CrdtOp::Update {
                site: *ls,
                clock: *lc,
                target: *lt,
                key: lk.clone(),
                value: merged_value,
            })
        }

        // Default: higher site wins.
        _ => {
            if local.site() >= remote.site() {
                ConflictResolution::KeepLocal
            } else {
                ConflictResolution::KeepRemote
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_insert(site: u64, clock: u64) -> CrdtOp {
        CrdtOp::Insert {
            site,
            clock,
            parent: None,
            node_type: "rect".into(),
            attrs: HashMap::new(),
        }
    }

    fn make_delete(site: u64, clock: u64, target: OpId) -> CrdtOp {
        CrdtOp::Delete {
            site,
            clock,
            target,
        }
    }

    fn make_update(site: u64, clock: u64, target: OpId, key: &str, value: &str) -> CrdtOp {
        CrdtOp::Update {
            site,
            clock,
            target,
            key: key.into(),
            value: value.into(),
        }
    }

    #[test]
    fn test_transform_two_inserts() {
        let op1 = make_insert(1, 1);
        let op2 = make_insert(2, 1);
        let (t1, t2) = transform_ops(&op1, &op2);
        assert_eq!(t1, op1);
        assert_eq!(t2, op2);
    }

    #[test]
    fn test_transform_insert_delete() {
        let op1 = make_insert(1, 1);
        let op2 = make_delete(2, 1, OpId::new(3, 1));
        let (t1, t2) = transform_ops(&op1, &op2);
        assert_eq!(t1, op1);
        assert_eq!(t2, op2);
    }

    #[test]
    fn test_transform_two_deletes() {
        let target = OpId::new(5, 1);
        let op1 = make_delete(1, 2, target);
        let op2 = make_delete(2, 2, target);
        let (t1, t2) = transform_ops(&op1, &op2);
        assert!(matches!(t1, CrdtOp::NoOp { .. }));
        assert_eq!(t2, op2);
    }

    #[test]
    fn test_transform_same_key_updates_higher_site_wins() {
        let target = OpId::new(5, 1);
        let op1 = make_update(1, 3, target, "fill", "#ff0000");
        let op2 = make_update(2, 3, target, "fill", "#00ff00");
        let (t1, t2) = transform_ops(&op1, &op2);
        // Site 2 is higher, so op2 wins.
        assert!(matches!(t1, CrdtOp::NoOp { .. }));
        assert_eq!(t2, op2);
    }

    #[test]
    fn test_transform_update_delete_same_target() {
        let target = OpId::new(5, 1);
        let op1 = make_update(1, 3, target, "fill", "red");
        let op2 = make_delete(2, 3, target);
        let (t1, t2) = transform_ops(&op1, &op2);
        // Delete wins.
        assert!(matches!(t1, CrdtOp::NoOp { .. }));
        assert_eq!(t2, op2);
    }

    #[test]
    fn test_transform_independent_updates() {
        let t1 = OpId::new(5, 1);
        let t2 = OpId::new(6, 1);
        let op1 = make_update(1, 3, t1, "fill", "red");
        let op2 = make_update(2, 3, t2, "stroke", "blue");
        let (r1, r2) = transform_ops(&op1, &op2);
        assert_eq!(r1, op1);
        assert_eq!(r2, op2);
    }

    #[test]
    fn test_resolve_same_key_update_local_wins() {
        let target = OpId::new(5, 1);
        let local = make_update(10, 3, target, "fill", "red");
        let remote = make_update(2, 3, target, "fill", "blue");
        assert_eq!(
            resolve_conflict(&local, &remote),
            ConflictResolution::KeepLocal
        );
    }

    #[test]
    fn test_resolve_same_key_update_remote_wins() {
        let target = OpId::new(5, 1);
        let local = make_update(1, 3, target, "fill", "red");
        let remote = make_update(10, 3, target, "fill", "blue");
        assert_eq!(
            resolve_conflict(&local, &remote),
            ConflictResolution::KeepRemote
        );
    }

    #[test]
    fn test_resolve_delete_vs_update() {
        let target = OpId::new(5, 1);
        let local = make_delete(1, 3, target);
        let remote = make_update(2, 3, target, "fill", "red");
        assert_eq!(
            resolve_conflict(&local, &remote),
            ConflictResolution::KeepLocal
        );
    }

    #[test]
    fn test_resolve_update_vs_delete() {
        let target = OpId::new(5, 1);
        let local = make_update(1, 3, target, "fill", "red");
        let remote = make_delete(2, 3, target);
        assert_eq!(
            resolve_conflict(&local, &remote),
            ConflictResolution::KeepRemote
        );
    }

    #[test]
    fn test_resolve_different_key_updates_merge() {
        let target = OpId::new(5, 1);
        let local = make_update(1, 3, target, "fill", "red");
        let remote = make_update(2, 3, target, "stroke", "blue");
        let result = resolve_conflict(&local, &remote);
        match result {
            ConflictResolution::Merge(op) => {
                assert_eq!(op.site(), 1);
            }
            other => panic!("Expected Merge, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_default_higher_site_wins() {
        let op1 = make_insert(10, 1);
        let op2 = make_insert(2, 1);
        assert_eq!(
            resolve_conflict(&op1, &op2),
            ConflictResolution::KeepLocal
        );
    }

    #[test]
    fn test_transform_batch_suppresses_conflicting_local_ops() {
        let target = OpId::new(5, 1);
        let local = vec![make_update(1, 2, target, "fill", "red")];
        let remote = vec![make_delete(2, 2, target)];
        let (local, remote) = transform_batch(&local, &remote);
        assert!(matches!(local[0], CrdtOp::NoOp { .. }));
        assert!(matches!(remote[0], CrdtOp::Delete { .. }));
    }
}
