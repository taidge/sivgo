//! Bounding box cache with invalidation support.
//!
//! Caches per-node bounding boxes and invalidates them when DOM events indicate
//! a node has changed. Group bounding boxes are the union of their children.

use std::collections::{HashMap, HashSet};

use sivgo_dom::observer::DomEvent;
use sivgo_dom::{NodeId, SvgDocument};
use sivgo_math::Rect;

use crate::selection::element_bounding_box;

/// A cached mapping from `NodeId` to its bounding box.
#[derive(Debug, Default)]
pub struct BBoxCache {
    cache: HashMap<NodeId, Option<Rect>>,
    dirty: HashSet<NodeId>,
}

impl BBoxCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the cached bounding box for a node, recomputing if dirty.
    ///
    /// For leaf elements this is computed from the element geometry.
    /// For groups this is the union of all child bounding boxes.
    pub fn get(&mut self, id: NodeId, doc: &SvgDocument) -> Option<Rect> {
        if self.dirty.contains(&id) || !self.cache.contains_key(&id) {
            let bb = self.compute(id, doc);
            self.cache.insert(id, bb);
            self.dirty.remove(&id);
            bb
        } else {
            *self.cache.get(&id).unwrap()
        }
    }

    /// Get without triggering recomputation.
    pub fn get_if_clean(&self, id: NodeId) -> Option<Option<Rect>> {
        if self.dirty.contains(&id) {
            None
        } else {
            self.cache.get(&id).copied()
        }
    }

    /// Mark a node as dirty.
    pub fn invalidate(&mut self, id: NodeId) {
        self.dirty.insert(id);
    }

    /// Mark a node and all ancestors as dirty (because a change in a child
    /// affects the group's bounding box).
    pub fn invalidate_with_ancestors(&mut self, id: NodeId, doc: &SvgDocument) {
        self.dirty.insert(id);
        if let Some(node) = doc.get_node(id) {
            if let Some(parent) = node.parent {
                self.invalidate_with_ancestors(parent, doc);
            }
        }
    }

    /// Process DOM events and invalidate affected cache entries.
    pub fn process_events(&mut self, events: &[DomEvent], doc: &SvgDocument) {
        for event in events {
            match event {
                DomEvent::NodeAdded(id) | DomEvent::NodeModified(id) => {
                    self.invalidate_with_ancestors(*id, doc);
                }
                DomEvent::NodeRemoved(id) => {
                    self.cache.remove(id);
                    self.dirty.remove(id);
                }
                DomEvent::AttributeChanged { node, .. } => {
                    self.invalidate_with_ancestors(*node, doc);
                }
                DomEvent::ChildrenChanged(parent) => {
                    self.invalidate_with_ancestors(*parent, doc);
                }
            }
        }
    }

    /// Rebuild the entire cache by walking the document tree.
    pub fn rebuild(&mut self, doc: &SvgDocument) {
        self.cache.clear();
        self.dirty.clear();
        if let Some(root) = doc.root() {
            self.compute_recursive(root, doc);
        }
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
    }

    fn compute(&mut self, id: NodeId, doc: &SvgDocument) -> Option<Rect> {
        let node = doc.get_node(id)?;

        // Try element-level bounding box first (leaf shapes).
        if let Some(bb) = element_bounding_box(&node.element) {
            return Some(bb);
        }

        // For groups and other container elements, compute as union of children.
        let children = &node.children;
        if children.is_empty() {
            return None;
        }

        let mut result: Option<Rect> = None;
        for &child in children {
            if let Some(child_bb) = self.get(child, doc) {
                result = Some(match result {
                    Some(r) => r.union(child_bb),
                    None => child_bb,
                });
            }
        }
        result
    }

    fn compute_recursive(&mut self, id: NodeId, doc: &SvgDocument) {
        if let Some(node) = doc.get_node(id) {
            let children: Vec<NodeId> = node.children.clone();
            for child in &children {
                self.compute_recursive(*child, doc);
            }
        }
        let bb = self.compute(id, doc);
        self.cache.insert(id, bb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{GroupAttrs, RectAttrs, SvgElement, SvgLength, SvgSvgAttrs};

    fn make_doc() -> (SvgDocument, NodeId, NodeId) {
        let mut doc = SvgDocument::new();
        let svg = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg);

        let rect_attrs = RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(20.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(50.0),
            ..Default::default()
        };
        let rect = doc.add_node(SvgElement::Rect(rect_attrs));
        doc.append_child(svg, rect);

        let _: Vec<_> = doc.drain_events().collect();
        (doc, svg, rect)
    }

    #[test]
    fn cache_computes_rect_bbox() {
        let (doc, _, rect) = make_doc();
        let mut cache = BBoxCache::new();
        let bb = cache.get(rect, &doc).unwrap();
        assert!((bb.x0 - 10.0).abs() < 1e-10);
        assert!((bb.y0 - 20.0).abs() < 1e-10);
        assert!((bb.x1 - 110.0).abs() < 1e-10);
        assert!((bb.y1 - 70.0).abs() < 1e-10);
    }

    #[test]
    fn cache_returns_cached_value() {
        let (doc, _, rect) = make_doc();
        let mut cache = BBoxCache::new();
        let _ = cache.get(rect, &doc);
        // Second call should return cached value.
        let bb = cache.get(rect, &doc).unwrap();
        assert!((bb.x0 - 10.0).abs() < 1e-10);
    }

    #[test]
    fn invalidation_triggers_recompute() {
        let (doc, _, rect) = make_doc();
        let mut cache = BBoxCache::new();
        let _ = cache.get(rect, &doc);
        cache.invalidate(rect);
        assert!(cache.get_if_clean(rect).is_none());
        // After get, it should be clean again.
        let _ = cache.get(rect, &doc);
        assert!(cache.get_if_clean(rect).is_some());
    }

    #[test]
    fn group_bbox_is_union_of_children() {
        let mut doc = SvgDocument::new();
        let svg = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg);

        let g = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg, g);

        let r1 = RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(50.0),
            height: SvgLength::px(50.0),
            ..Default::default()
        };
        let rect1 = doc.add_node(SvgElement::Rect(r1));
        doc.append_child(g, rect1);

        let r2 = RectAttrs {
            x: SvgLength::px(100.0),
            y: SvgLength::px(100.0),
            width: SvgLength::px(50.0),
            height: SvgLength::px(50.0),
            ..Default::default()
        };
        let rect2 = doc.add_node(SvgElement::Rect(r2));
        doc.append_child(g, rect2);

        let _: Vec<_> = doc.drain_events().collect();

        let mut cache = BBoxCache::new();
        let bb = cache.get(g, &doc).unwrap();
        assert!((bb.x0 - 0.0).abs() < 1e-10);
        assert!((bb.y0 - 0.0).abs() < 1e-10);
        assert!((bb.x1 - 150.0).abs() < 1e-10);
        assert!((bb.y1 - 150.0).abs() < 1e-10);
    }
}
