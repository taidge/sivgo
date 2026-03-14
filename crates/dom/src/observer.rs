//! DOM observer/event system for the SVG document.
//!
//! Provides a [`DomEvent`] enum describing mutations to the document tree and
//! a simple event-bus pattern on [`SvgDocument`] that accumulates events which
//! can be drained by consumers.  This avoids borrow-checker issues that arise
//! with trait-object observers while still supporting all the same use cases
//! (style invalidation, bounding-box cache invalidation, UI updates, etc.).
//!
//! # Example
//!
//! ```
//! use sivgo_dom::{SvgDocument, SvgElement, RectAttrs};
//! use sivgo_dom::observer::DomEvent;
//!
//! let mut doc = SvgDocument::new();
//! let node = doc.add_node(SvgElement::Rect(RectAttrs::default()));
//! // The add_node call pushed a NodeAdded event.
//! let events: Vec<DomEvent> = doc.drain_events().collect();
//! assert!(events.iter().any(|e| matches!(e, DomEvent::NodeAdded(_))));
//! ```

use crate::node::NodeId;

/// Describes a single mutation to the SVG document tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomEvent {
    /// A new node was inserted into the document's node map.
    NodeAdded(NodeId),

    /// A node (and its descendants) were removed from the document.
    NodeRemoved(NodeId),

    /// A node was modified in a general way (element data changed, etc.).
    NodeModified(NodeId),

    /// A specific attribute on a node was changed (set or removed).
    AttributeChanged { node: NodeId, attr: String },

    /// The children list of a node changed (child appended, inserted, removed,
    /// or reordered).
    ChildrenChanged(NodeId),
}
