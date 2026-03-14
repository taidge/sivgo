//! Filter Editor panel data model (11.4.16).
//!
//! A node-graph style editor for constructing SVG filter chains from
//! individual filter primitives.

use std::collections::HashMap;

/// A single filter primitive node in the editor graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterNode {
    /// SVG filter primitive type (e.g. `"feGaussianBlur"`, `"feColorMatrix"`).
    pub primitive_type: String,
    /// Parameters for this primitive (key -> value as strings).
    pub params: HashMap<String, String>,
    /// Horizontal position in the editor canvas.
    pub x: f64,
    /// Vertical position in the editor canvas.
    pub y: f64,
}

impl FilterNode {
    /// Create a new filter node at the given position.
    pub fn new(primitive_type: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            primitive_type: primitive_type.into(),
            params: HashMap::new(),
            x,
            y,
        }
    }
}

/// A directional connection between two filter nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterConnection {
    /// Source node index.
    pub src: usize,
    /// Destination node index.
    pub dst: usize,
}

/// State for the Filter Editor panel.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterEditorState {
    /// Ordered list of filter primitive nodes.
    pub filter_chain: Vec<FilterNode>,
    /// Index of the currently selected node, if any.
    pub selected_node: Option<usize>,
    /// Connections between nodes (src index -> dst index).
    pub connections: Vec<(usize, usize)>,
}

impl Default for FilterEditorState {
    fn default() -> Self {
        Self {
            filter_chain: Vec::new(),
            selected_node: None,
            connections: Vec::new(),
        }
    }
}

impl FilterEditorState {
    /// Add a new filter node of the given type at position `(x, y)`.
    /// Returns the index of the new node.
    pub fn add_node(&mut self, primitive_type: impl Into<String>, x: f64, y: f64) -> usize {
        self.filter_chain
            .push(FilterNode::new(primitive_type, x, y));
        self.filter_chain.len() - 1
    }

    /// Remove a node by index, adjusting connections.
    /// Returns `true` if the node was removed.
    pub fn remove_node(&mut self, idx: usize) -> bool {
        if idx >= self.filter_chain.len() {
            return false;
        }
        self.filter_chain.remove(idx);
        // Remove connections involving the deleted node and remap indices.
        self.connections.retain(|(s, d)| *s != idx && *d != idx);
        for (s, d) in &mut self.connections {
            if *s > idx {
                *s -= 1;
            }
            if *d > idx {
                *d -= 1;
            }
        }
        // Adjust selection.
        match self.selected_node {
            Some(sel) if sel == idx => self.selected_node = None,
            Some(sel) if sel > idx => self.selected_node = Some(sel - 1),
            _ => {}
        }
        true
    }

    /// Connect the output of node `src` to the input of node `dst`.
    /// Returns `true` if the connection was added (no duplicate).
    pub fn connect(&mut self, src: usize, dst: usize) -> bool {
        if src >= self.filter_chain.len() || dst >= self.filter_chain.len() || src == dst {
            return false;
        }
        let conn = (src, dst);
        if self.connections.contains(&conn) {
            return false;
        }
        self.connections.push(conn);
        true
    }

    /// Set a parameter on a node.
    /// Returns `true` if the node index was valid.
    pub fn set_param(
        &mut self,
        node_idx: usize,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> bool {
        if let Some(node) = self.filter_chain.get_mut(node_idx) {
            node.params.insert(key.into(), value.into());
            true
        } else {
            false
        }
    }

    /// Return the number of nodes in the filter chain.
    pub fn node_count(&self) -> usize {
        self.filter_chain.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_nodes() {
        let mut state = FilterEditorState::default();
        let i0 = state.add_node("feGaussianBlur", 10.0, 20.0);
        let i1 = state.add_node("feColorMatrix", 100.0, 20.0);
        assert_eq!(i0, 0);
        assert_eq!(i1, 1);
        assert_eq!(state.node_count(), 2);
    }

    #[test]
    fn remove_node_adjusts_connections() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        state.add_node("B", 0.0, 0.0);
        state.add_node("C", 0.0, 0.0);
        state.connect(0, 1);
        state.connect(1, 2);
        // Remove B (index 1).
        assert!(state.remove_node(1));
        assert_eq!(state.node_count(), 2);
        // Connection 0->1 is removed (involved deleted node).
        // Connection 1->2 was (old 1->2), also removed because src=1 was the
        // deleted node.
        assert!(state.connections.is_empty());
    }

    #[test]
    fn remove_node_adjusts_selection() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        state.add_node("B", 0.0, 0.0);
        state.add_node("C", 0.0, 0.0);
        state.selected_node = Some(2);
        state.remove_node(1);
        assert_eq!(state.selected_node, Some(1)); // shifted down
    }

    #[test]
    fn remove_selected_node() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        state.selected_node = Some(0);
        state.remove_node(0);
        assert_eq!(state.selected_node, None);
    }

    #[test]
    fn connect_valid() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        state.add_node("B", 0.0, 0.0);
        assert!(state.connect(0, 1));
        assert!(!state.connect(0, 1)); // duplicate
    }

    #[test]
    fn connect_self_loop() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        assert!(!state.connect(0, 0));
    }

    #[test]
    fn connect_out_of_bounds() {
        let mut state = FilterEditorState::default();
        state.add_node("A", 0.0, 0.0);
        assert!(!state.connect(0, 5));
    }

    #[test]
    fn set_param() {
        let mut state = FilterEditorState::default();
        state.add_node("feGaussianBlur", 0.0, 0.0);
        assert!(state.set_param(0, "stdDeviation", "5"));
        assert_eq!(
            state.filter_chain[0].params.get("stdDeviation"),
            Some(&"5".to_string())
        );
    }

    #[test]
    fn set_param_invalid_index() {
        let mut state = FilterEditorState::default();
        assert!(!state.set_param(0, "key", "value"));
    }
}
