//! Command: merge one layer into the layer below it.

use sivgo_dom::{NodeId, SvgDocument, SvgNode};

use crate::command::Command;

/// Moves all children from `source_layer` into `target_layer`, then removes the
/// now-empty source layer. Undo restores the source layer and its children.
#[derive(Debug)]
pub struct MergeLayerCommand {
    source_layer: NodeId,
    target_layer: NodeId,
    source_parent: Option<NodeId>,
    source_index: usize,
    source_node: Option<SvgNode>,
    moved_children: Vec<NodeId>,
}

impl MergeLayerCommand {
    pub fn new(source_layer: NodeId, target_layer: NodeId) -> Self {
        Self {
            source_layer,
            target_layer,
            source_parent: None,
            source_index: 0,
            source_node: None,
            moved_children: Vec::new(),
        }
    }
}

impl Command for MergeLayerCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let Some(source) = doc.get_node(self.source_layer).cloned() else {
            return;
        };
        if doc.get_node(self.target_layer).is_none() {
            return;
        }

        self.source_parent = doc.parent(self.source_layer);
        self.source_index = doc.child_index(self.source_layer).unwrap_or(0);
        self.moved_children = source.children.clone();
        self.source_node = Some(source);

        for child_id in &self.moved_children {
            doc.detach_node(*child_id);
            doc.append_child(self.target_layer, *child_id);
        }

        doc.detach_node(self.source_layer);
        doc.remove_node(self.source_layer);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        let Some(parent) = self.source_parent else {
            return;
        };
        let Some(source_node) = self.source_node.clone() else {
            return;
        };

        let mut restored = source_node;
        restored.children.clear();
        restored.parent = None;
        doc.insert_node(restored);
        doc.insert_child_at(parent, self.source_layer, self.source_index);

        for child_id in &self.moved_children {
            doc.detach_node(*child_id);
            doc.append_child(self.source_layer, *child_id);
        }
    }

    fn description(&self) -> &str {
        "Merge layer"
    }
}
