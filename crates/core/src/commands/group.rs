//! Command: group selected nodes into a `<g>`.

use sivgo_dom::{GroupAttrs, NodeId, PresentationAttrs, SvgDocument, SvgElement, SvgNode};

use crate::command::Command;

/// Groups a set of nodes under a new `<g>` element.
///
/// The group is inserted into the parent of the first node, at the position
/// of the first node.
#[derive(Debug)]
pub struct GroupCommand {
    /// The nodes to group (in document order).
    node_ids: Vec<NodeId>,
    /// The created group node id (filled on first execute).
    group_id: Option<NodeId>,
    /// Parent and original indices of the nodes (for undo).
    original_positions: Vec<(NodeId, usize)>,
}

impl GroupCommand {
    pub fn new(node_ids: Vec<NodeId>) -> Self {
        Self {
            node_ids,
            group_id: None,
            original_positions: Vec::new(),
        }
    }

    pub fn group_id(&self) -> Option<NodeId> {
        self.group_id
    }
}

impl Command for GroupCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if self.node_ids.is_empty() {
            return;
        }

        // Find the parent and index of the first node.
        let first_parent = match doc.parent(self.node_ids[0]) {
            Some(p) => p,
            None => return,
        };
        let first_index = doc.child_index(self.node_ids[0]).unwrap_or(0);

        // Record original positions (before any detaching).
        self.original_positions.clear();
        for &id in &self.node_ids {
            let parent = doc.parent(id).unwrap_or(first_parent);
            let idx = doc.child_index(id).unwrap_or(0);
            self.original_positions.push((parent, idx));
        }

        // Create or re-use the group node.
        let group_id = if let Some(gid) = self.group_id {
            let node = SvgNode::new(gid, SvgElement::Group(GroupAttrs::default()));
            doc.insert_node(node);
            gid
        } else {
            let gid = doc.add_node(SvgElement::Group(GroupAttrs::default()));
            self.group_id = Some(gid);
            gid
        };

        // Insert group at first_index in first_parent.
        doc.insert_child_at(first_parent, group_id, first_index);

        // Detach each node from its current parent and reparent to group.
        for &id in &self.node_ids {
            doc.detach_node(id);
            doc.append_child(group_id, id);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        let group_id = match self.group_id {
            Some(id) => id,
            None => return,
        };

        // Move children back to their original parents and positions.
        // We process in reverse to keep indices stable.
        for (i, &id) in self.node_ids.iter().enumerate().rev() {
            doc.detach_node(id);
            if let Some(&(parent, idx)) = self.original_positions.get(i) {
                doc.insert_child_at(parent, id, idx);
            }
        }

        // Remove the group node.
        doc.remove_node(group_id);
    }

    fn description(&self) -> &str {
        "Group"
    }
}

/// Ungroups a `<g>` element, moving its children to the group's parent.
#[derive(Debug)]
pub struct UngroupCommand {
    group_id: NodeId,
    /// Parent of the group and its index (for undo).
    group_parent: Option<NodeId>,
    group_index: usize,
    /// The children that were in the group.
    children: Vec<NodeId>,
    group_element: Option<SvgElement>,
    group_presentation: Option<PresentationAttrs>,
    group_xml_id: Option<String>,
    group_attributes: Option<std::collections::HashMap<String, String>>,
}

impl UngroupCommand {
    pub fn new(group_id: NodeId) -> Self {
        Self {
            group_id,
            group_parent: None,
            group_index: 0,
            children: Vec::new(),
            group_element: None,
            group_presentation: None,
            group_xml_id: None,
            group_attributes: None,
        }
    }
}

impl Command for UngroupCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        // Record group's parent and position.
        self.group_parent = doc.parent(self.group_id);
        self.group_index = doc.child_index(self.group_id).unwrap_or(0);
        if let Some(group) = doc.get_node(self.group_id) {
            self.children = group.children.clone();
            self.group_element = Some(group.element.clone());
            self.group_presentation = Some(group.presentation.clone());
            self.group_xml_id = group.xml_id.clone();
            self.group_attributes = Some(group.attributes.clone());
        }

        let parent = match self.group_parent {
            Some(p) => p,
            None => return,
        };

        // Move each child from the group to the parent, maintaining order.
        for (insert_at, &child_id) in (self.group_index..).zip(self.children.iter()) {
            doc.detach_node(child_id);
            doc.insert_child_at(parent, child_id, insert_at);
        }

        // Remove the now-empty group.
        doc.remove_node(self.group_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        let parent = match self.group_parent {
            Some(p) => p,
            None => return,
        };

        // Re-create the group.
        let mut node = SvgNode::new(
            self.group_id,
            self.group_element
                .clone()
                .unwrap_or_else(|| SvgElement::Group(GroupAttrs::default())),
        );
        if let Some(presentation) = &self.group_presentation {
            node.presentation = presentation.clone();
        }
        node.xml_id = self.group_xml_id.clone();
        if let Some(attributes) = &self.group_attributes {
            node.attributes = attributes.clone();
        }
        doc.insert_node(node);
        doc.insert_child_at(parent, self.group_id, self.group_index);

        // Move children back into the group.
        for &child_id in &self.children {
            doc.detach_node(child_id);
            doc.append_child(self.group_id, child_id);
        }
    }

    fn description(&self) -> &str {
        "Ungroup"
    }
}
