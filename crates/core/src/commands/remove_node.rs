//! Command: remove a node from the document.

use sivgo_dom::{NodeId, SvgDocument, SvgNode};

use crate::command::Command;

/// Removes a node and everything under it, storing the whole subtree for undo.
///
/// The document's `remove_node` deletes descendants as well, so saving only the
/// node itself is not enough to put it back: undo would restore a node whose
/// child list pointed at entries that no longer exist. Deleting a group, a
/// layer or a gradient with stops and then undoing used to lose the contents
/// permanently and leave dangling references behind.
#[derive(Debug)]
pub struct RemoveNodeCommand {
    node_id: NodeId,
    /// Where the node sat in its parent, for undo.
    parent_id: Option<NodeId>,
    index: usize,
    /// The removed node and all its descendants, in pre-order.
    subtree: Vec<SvgNode>,
}

impl RemoveNodeCommand {
    /// Create a command that will remove the node with `node_id`.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            parent_id: None,
            index: 0,
            subtree: Vec::new(),
        }
    }
}

impl Command for RemoveNodeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        // Snapshot the whole subtree before anything is detached. Nodes keep
        // their ids, so references elsewhere in the document stay valid once
        // the subtree is restored.
        self.subtree = doc
            .traverse_pre_order(self.node_id)
            .into_iter()
            .filter_map(|id| doc.get_node(id).cloned())
            .collect();

        // Detach from parent, remembering the slot to restore into.
        if let Some((parent, idx)) = doc.detach_node(self.node_id) {
            self.parent_id = Some(parent);
            self.index = idx;
        }

        doc.remove_node(self.node_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if self.subtree.is_empty() {
            return;
        }

        // Re-insert every node, keeping the parent and child links each one
        // carried when it was captured.
        for node in &self.subtree {
            doc.insert_node(node.clone());
        }

        // Re-attach the root to its original parent and position. Descendants
        // are already linked to each other through the restored child lists.
        if let Some(parent_id) = self.parent_id {
            doc.insert_child_at(parent_id, self.node_id, self.index);
        }
    }

    fn description(&self) -> &str {
        "Delete element"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{GroupAttrs, RectAttrs, SvgElement};

    /// Build `<svg><g><rect/><rect/></g></svg>` and return (doc, group, rects).
    fn document_with_group() -> (SvgDocument, NodeId, Vec<NodeId>) {
        let mut doc = SvgDocument::new();
        let root = doc.add_node(SvgElement::Svg(Default::default()));
        doc.set_root(root);

        let group = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(root, group);

        let mut rects = Vec::new();
        for _ in 0..2 {
            let rect = doc.add_node(SvgElement::Rect(RectAttrs::default()));
            doc.append_child(group, rect);
            rects.push(rect);
        }

        (doc, group, rects)
    }

    #[test]
    fn undo_restores_the_entire_subtree() {
        let (mut doc, group, rects) = document_with_group();
        let before = doc.len();

        let mut cmd = RemoveNodeCommand::new(group);
        cmd.execute(&mut doc);

        assert!(doc.get_node(group).is_none(), "group must be gone");
        for &rect in &rects {
            assert!(doc.get_node(rect).is_none(), "children must be gone too");
        }

        cmd.undo(&mut doc);

        assert_eq!(doc.len(), before, "undo must restore every node");
        assert!(doc.get_node(group).is_some(), "group must come back");
        for &rect in &rects {
            let child = doc.get_node(rect).expect("child must come back");
            assert_eq!(child.parent, Some(group), "child must be re-parented");
        }
        assert_eq!(doc.children(group), rects.as_slice(), "order must survive");
    }

    #[test]
    fn redo_after_undo_removes_the_subtree_again() {
        let (mut doc, group, rects) = document_with_group();

        let mut cmd = RemoveNodeCommand::new(group);
        cmd.execute(&mut doc);
        cmd.undo(&mut doc);
        cmd.execute(&mut doc);

        assert!(doc.get_node(group).is_none());
        for &rect in &rects {
            assert!(doc.get_node(rect).is_none());
        }

        cmd.undo(&mut doc);
        assert_eq!(doc.children(group), rects.as_slice());
    }

    #[test]
    fn undo_restores_position_among_siblings() {
        let mut doc = SvgDocument::new();
        let root = doc.add_node(SvgElement::Svg(Default::default()));
        doc.set_root(root);

        let first = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        let middle = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        let last = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        for id in [first, middle, last] {
            doc.append_child(root, id);
        }
        let inner = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(middle, inner);

        let mut cmd = RemoveNodeCommand::new(middle);
        cmd.execute(&mut doc);
        assert_eq!(doc.children(root), &[first, last]);

        cmd.undo(&mut doc);
        assert_eq!(doc.children(root), &[first, middle, last]);
        assert_eq!(doc.children(middle), &[inner]);
    }

    #[test]
    fn deeply_nested_subtree_survives_a_round_trip() {
        let mut doc = SvgDocument::new();
        let root = doc.add_node(SvgElement::Svg(Default::default()));
        doc.set_root(root);

        let outer = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(root, outer);

        let mut parent = outer;
        let mut chain = vec![outer];
        for _ in 0..10 {
            let child = doc.add_node(SvgElement::Group(GroupAttrs::default()));
            doc.append_child(parent, child);
            chain.push(child);
            parent = child;
        }
        let leaf = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(parent, leaf);
        chain.push(leaf);

        let before = doc.len();
        let mut cmd = RemoveNodeCommand::new(outer);
        cmd.execute(&mut doc);
        cmd.undo(&mut doc);

        assert_eq!(doc.len(), before);
        for pair in chain.windows(2) {
            assert_eq!(
                doc.children(pair[0]),
                &[pair[1]],
                "link between nested nodes must survive"
            );
        }
    }
}
