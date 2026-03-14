use sivgo_dom::{NodeId, PresentationAttrs, SvgElement};
use sivgo_math::Transform2D;

/// Actions that a plugin can request the editor to perform.
///
/// After a plugin hook returns a `Vec<PluginAction>`, the host (typically
/// the `EditorEngine`) inspects each action and applies it to the document.
#[derive(Debug, Clone)]
pub enum PluginAction {
    /// Add a new element as a child of `parent_id`.
    AddNode {
        parent_id: NodeId,
        element: SvgElement,
    },
    /// Remove a node from the document.
    RemoveNode(NodeId),
    /// Replace the presentation attributes of a node.
    ModifyAttribute {
        node_id: NodeId,
        attrs: PresentationAttrs,
    },
    /// Set (or replace) the transform on a node.
    SetTransform {
        node_id: NodeId,
        transform: Transform2D,
    },
    /// Display a transient message to the user (e.g. toast / status bar).
    ShowMessage(String),
    /// Write a message to the editor log.
    Log(String),
}
