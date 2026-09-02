//! Replicas that have seen the same operations must agree, whatever order
//! those operations arrived in.
//!
//! The existing unit tests only merged two replicas in one direction, which
//! never exercises the case that actually diverged: an update made on one site
//! to a node inserted on another, delivered to a third site before the insert
//! it depends on. Per-site clocks do not order those two operations, so the
//! update used to be dropped and that replica kept a different document
//! forever.

use std::collections::{BTreeMap, HashMap};

use sivgo_core::collab::crdt::{CrdtDocument, CrdtOp, OpId};

/// Apply a local insert and return the new node's id.
fn insert(doc: &mut CrdtDocument, node_type: &str) -> OpId {
    let id = doc.generate_id();
    doc.apply_local(CrdtOp::Insert {
        site: id.site,
        clock: id.clock,
        parent: None,
        node_type: node_type.to_string(),
        attrs: HashMap::new(),
    });
    id
}

/// Apply a local attribute update.
fn update(doc: &mut CrdtDocument, target: OpId, key: &str, value: &str) {
    let id = doc.generate_id();
    doc.apply_local(CrdtOp::Update {
        site: id.site,
        clock: id.clock,
        target,
        key: key.to_string(),
        value: value.to_string(),
    });
}

/// Apply a local delete.
fn delete(doc: &mut CrdtDocument, target: OpId) {
    let id = doc.generate_id();
    doc.apply_local(CrdtOp::Delete {
        site: id.site,
        clock: id.clock,
        target,
    });
}

/// A comparable snapshot of what a replica currently shows.
fn snapshot(doc: &CrdtDocument) -> Vec<(String, BTreeMap<String, String>)> {
    doc.visible_nodes()
        .into_iter()
        .map(|node| {
            let id = format!("{}:{}", node.id.site, node.id.clock);
            let attrs: BTreeMap<String, String> = node
                .attrs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            (id, attrs)
        })
        .collect()
}

#[track_caller]
fn assert_converged(replicas: &[(&str, &CrdtDocument)]) {
    let (first_name, first) = replicas[0];
    let expected = snapshot(first);
    for (name, doc) in &replicas[1..] {
        assert_eq!(snapshot(doc), expected, "{name} diverged from {first_name}");
    }
}

/// Deliver `ops` to a fresh replica in the given order.
fn replica_seeing(ops: &[CrdtOp]) -> CrdtDocument {
    let mut doc = CrdtDocument::new(99);
    for op in ops {
        let _ = doc.apply_remote(op.clone());
    }
    doc
}

#[test]
fn an_update_delivered_before_its_target_is_not_lost() {
    // Site 2 inserts a node; site 1 updates it after merging.
    let mut site2 = CrdtDocument::new(2);
    let node = insert(&mut site2, "rect");

    let mut site1 = CrdtDocument::new(1);
    site1.merge(&site2);
    update(&mut site1, node, "fill", "red");

    let insert_ops: Vec<CrdtOp> = site2.to_operations();
    let update_ops: Vec<CrdtOp> = site1
        .to_operations()
        .into_iter()
        .filter(|op| op.site() == 1)
        .collect();
    assert!(
        !update_ops.is_empty(),
        "site 1 should have produced an update"
    );

    // A third replica receives the update first.
    let mut out_of_order: Vec<CrdtOp> = update_ops.clone();
    out_of_order.extend(insert_ops.clone());
    let late = replica_seeing(&out_of_order);

    // And another receives them in causal order.
    let mut in_order: Vec<CrdtOp> = insert_ops;
    in_order.extend(update_ops);
    let early = replica_seeing(&in_order);

    assert_converged(&[("in-order", &early), ("out-of-order", &late)]);

    let attrs = &snapshot(&late)[0].1;
    assert_eq!(
        attrs.get("fill").map(String::as_str),
        Some("red"),
        "the update must survive arriving before its target"
    );
}

#[test]
fn three_sites_converge_however_the_operations_interleave() {
    let mut a = CrdtDocument::new(1);
    let mut b = CrdtDocument::new(2);
    let mut c = CrdtDocument::new(3);

    let from_a = insert(&mut a, "rect");
    let from_b = insert(&mut b, "circle");

    // Each site learns about the others and then edits what it learned.
    a.merge(&b);
    b.merge(&a);
    update(&mut a, from_b, "fill", "blue");
    update(&mut b, from_a, "stroke", "black");
    insert(&mut c, "path");

    // Collect every operation and deliver it in several different orders.
    let mut all: Vec<CrdtOp> = Vec::new();
    for doc in [&a, &b, &c] {
        for op in doc.to_operations() {
            if !all.iter().any(|seen| seen.op_id() == op.op_id()) {
                all.push(op);
            }
        }
    }

    let forward = replica_seeing(&all);

    let mut reversed = all.clone();
    reversed.reverse();
    let backward = replica_seeing(&reversed);

    // A deterministic shuffle: every third operation first.
    let mut shuffled: Vec<CrdtOp> = Vec::new();
    for start in 0..3 {
        for (i, op) in all.iter().enumerate() {
            if i % 3 == start {
                shuffled.push(op.clone());
            }
        }
    }
    let scrambled = replica_seeing(&shuffled);

    assert_converged(&[
        ("forward", &forward),
        ("backward", &backward),
        ("scrambled", &scrambled),
    ]);
}

#[test]
fn merging_is_idempotent_and_order_independent() {
    let mut a = CrdtDocument::new(1);
    let mut b = CrdtDocument::new(2);
    let node = insert(&mut a, "rect");
    update(&mut a, node, "fill", "red");
    insert(&mut b, "circle");

    let mut ab = CrdtDocument::new(3);
    ab.merge(&a);
    ab.merge(&b);
    // Merging the same state again changes nothing.
    ab.merge(&a);
    ab.merge(&b);

    let mut ba = CrdtDocument::new(4);
    ba.merge(&b);
    ba.merge(&a);

    assert_converged(&[("a-then-b", &ab), ("b-then-a", &ba)]);
}

#[test]
fn a_delete_arriving_before_its_target_still_removes_it() {
    let mut site1 = CrdtDocument::new(1);
    let node = insert(&mut site1, "rect");
    let insert_ops = site1.to_operations();

    let mut site2 = CrdtDocument::new(2);
    site2.merge(&site1);
    delete(&mut site2, node);
    let delete_ops: Vec<CrdtOp> = site2
        .to_operations()
        .into_iter()
        .filter(|op| op.site() == 2)
        .collect();

    let mut out_of_order = delete_ops;
    out_of_order.extend(insert_ops);
    let replica = replica_seeing(&out_of_order);

    assert!(
        snapshot(&replica).is_empty(),
        "a delete must still apply when it arrives first"
    );
}

#[test]
fn concurrent_updates_to_one_attribute_settle_on_one_value() {
    let mut origin = CrdtDocument::new(1);
    let node = insert(&mut origin, "rect");

    let mut a = CrdtDocument::new(2);
    a.merge(&origin);
    let mut b = CrdtDocument::new(3);
    b.merge(&origin);

    // Both edit the same attribute without seeing each other.
    update(&mut a, node, "fill", "red");
    update(&mut b, node, "fill", "blue");

    let mut ab = CrdtDocument::new(4);
    ab.merge(&a);
    ab.merge(&b);

    let mut ba = CrdtDocument::new(5);
    ba.merge(&b);
    ba.merge(&a);

    assert_converged(&[("a-then-b", &ab), ("b-then-a", &ba)]);
    let value = snapshot(&ab)[0].1.get("fill").cloned();
    assert!(
        value.as_deref() == Some("red") || value.as_deref() == Some("blue"),
        "the winner must be one of the two writes, got {value:?}"
    );
}

#[test]
fn operations_survive_a_round_trip_through_json() {
    // Operations are useless for collaboration if they cannot cross a network,
    // so they have to serialize and come back unchanged.
    let mut origin = CrdtDocument::new(1);
    let node = insert(&mut origin, "rect");
    update(&mut origin, node, "fill", "red");
    delete(&mut origin, node);

    let wire = serde_json::to_string(&origin.to_operations()).expect("serialize");
    let decoded: Vec<CrdtOp> = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(decoded, origin.to_operations());

    // A replica built from the decoded stream matches one built directly.
    let direct = replica_seeing(&origin.to_operations());
    let from_wire = replica_seeing(&decoded);
    assert_converged(&[("direct", &direct), ("from-wire", &from_wire)]);
}
