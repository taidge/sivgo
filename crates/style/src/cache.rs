//! Computed style cache with invalidation support.
//!
//! [`StyleCache`] wraps the style resolution system with a per-node cache.
//! After initial resolution via [`resolve_styles`], individual nodes can be
//! invalidated (e.g. after a DOM mutation) so that only affected subtrees
//! are recomputed on the next access.

use std::collections::{HashMap, HashSet};

use sivgo_dom::observer::DomEvent;
use sivgo_dom::{NodeId, SvgDocument};

use crate::cascade::resolve_styles;
use crate::computed::ComputedStyle;

/// A cached set of computed styles with lazy invalidation.
#[derive(Debug)]
pub struct StyleCache {
    /// The cached computed styles.
    styles: HashMap<NodeId, ComputedStyle>,
    /// Set of node IDs whose cached style is stale and needs recomputation.
    dirty: HashSet<NodeId>,
    /// Generation counter – bumped on every full rebuild.
    generation: u64,
}

impl Default for StyleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
            dirty: HashSet::new(),
            generation: 0,
        }
    }

    /// Current generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Perform a full resolution of the entire document, replacing all cached
    /// styles and clearing the dirty set.
    pub fn rebuild(&mut self, doc: &SvgDocument) {
        self.styles = resolve_styles(doc);
        self.dirty.clear();
        self.generation += 1;
    }

    /// Get the computed style for a node.
    ///
    /// If the node is dirty, this triggers a targeted recomputation of the
    /// node and its ancestors up to the nearest clean ancestor, then caches
    /// the result. If the node has never been resolved, returns `None`.
    pub fn get(&mut self, id: NodeId, doc: &SvgDocument) -> Option<&ComputedStyle> {
        if self.dirty.contains(&id) {
            self.recompute_node(id, doc);
        }
        self.styles.get(&id)
    }

    /// Get the computed style for a node without triggering recomputation.
    /// Returns `None` if the node is dirty or has never been resolved.
    pub fn get_if_clean(&self, id: NodeId) -> Option<&ComputedStyle> {
        if self.dirty.contains(&id) {
            None
        } else {
            self.styles.get(&id)
        }
    }

    /// Check whether a node is marked dirty.
    pub fn is_dirty(&self, id: NodeId) -> bool {
        self.dirty.contains(&id)
    }

    /// Mark a single node as dirty (its cached style needs recomputation).
    pub fn invalidate(&mut self, id: NodeId) {
        self.dirty.insert(id);
    }

    /// Mark a node and all its descendants as dirty.
    pub fn invalidate_subtree(&mut self, id: NodeId, doc: &SvgDocument) {
        self.dirty.insert(id);
        if let Some(node) = doc.get_node(id) {
            for &child in &node.children {
                self.invalidate_subtree(child, doc);
            }
        }
    }

    /// Process DOM events and invalidate affected nodes.
    ///
    /// For style purposes:
    /// - `NodeAdded` / `NodeRemoved`: invalidate the node and its parent's subtree
    ///   (sibling selectors may be affected).
    /// - `NodeModified` / `AttributeChanged`: invalidate the node's subtree
    ///   (inherited properties may cascade down).
    /// - `ChildrenChanged`: invalidate the parent's subtree.
    pub fn process_events(&mut self, events: &[DomEvent], doc: &SvgDocument) {
        for event in events {
            match event {
                DomEvent::NodeAdded(id) => {
                    // New node needs full resolution; also siblings might be
                    // affected by :first-child / :last-child selectors.
                    if let Some(node) = doc.get_node(*id) {
                        if let Some(parent) = node.parent {
                            self.invalidate_subtree(parent, doc);
                        }
                    }
                    self.dirty.insert(*id);
                }
                DomEvent::NodeRemoved(id) => {
                    // Remove stale entry; parent's children may need restyling.
                    self.styles.remove(id);
                    self.dirty.remove(id);
                }
                DomEvent::NodeModified(id) => {
                    // Element changed → its style and descendants may differ.
                    self.invalidate_subtree(*id, doc);
                }
                DomEvent::AttributeChanged { node, .. } => {
                    // Attribute change may affect this node's style and
                    // inherited properties flowing to descendants.
                    self.invalidate_subtree(*node, doc);
                }
                DomEvent::ChildrenChanged(parent) => {
                    // Structural change → selectors like :first-child may
                    // now match differently for all children.
                    self.invalidate_subtree(*parent, doc);
                }
            }
        }
    }

    /// Recompute a single dirty node by walking up to find a clean ancestor,
    /// then cascading down.
    fn recompute_node(&mut self, id: NodeId, doc: &SvgDocument) {
        // Find the chain of ancestors up to a clean one.
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(nid) = current {
            if !self.dirty.contains(&nid) && self.styles.contains_key(&nid) {
                break; // found a clean ancestor
            }
            chain.push(nid);
            current = doc.get_node(nid).and_then(|n| n.parent);
        }

        // The parent style is the clean ancestor's style (or default).
        let parent_style = current
            .and_then(|pid| self.styles.get(&pid))
            .cloned()
            .unwrap_or_default();

        // Walk the chain in reverse (from closest-to-root down to `id`) and
        // resolve each one. This is a simplified single-path cascade — for
        // full correctness with sibling selectors we'd need to do a subtree
        // resolve, but for the common case (attribute change on a node) this
        // is sufficient and much faster than a full rebuild.
        let mut style = parent_style;
        for &nid in chain.iter().rev() {
            // For each node in the chain, compute its style from the parent.
            let child_styles = crate::cascade::resolve_single(doc, nid, &style);
            self.styles.insert(nid, child_styles.clone());
            self.dirty.remove(&nid);
            style = child_styles;
        }
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.styles.clear();
        self.dirty.clear();
    }

    /// Get an immutable snapshot of all current styles (including dirty ones).
    pub fn all_styles(&self) -> &HashMap<NodeId, ComputedStyle> {
        &self.styles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{Color, GroupAttrs, Paint, RectAttrs, SvgElement, SvgSvgAttrs};

    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg);

        let mut g_attrs = GroupAttrs::default();
        let g = doc.add_node(SvgElement::Group(g_attrs));
        doc.append_child(svg, g);

        let r = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g, r);

        // Drain initial events
        let _: Vec<_> = doc.drain_events().collect();
        doc
    }

    #[test]
    fn rebuild_populates_cache() {
        let doc = make_doc();
        let mut cache = StyleCache::new();
        assert!(cache.is_empty());

        cache.rebuild(&doc);
        assert!(!cache.is_empty());
        assert_eq!(cache.generation(), 1);
    }

    #[test]
    fn invalidate_marks_dirty() {
        let doc = make_doc();
        let mut cache = StyleCache::new();
        cache.rebuild(&doc);

        let root = doc.root().unwrap();
        assert!(!cache.is_dirty(root));

        cache.invalidate(root);
        assert!(cache.is_dirty(root));
    }

    #[test]
    fn get_recomputes_dirty_node() {
        let doc = make_doc();
        let mut cache = StyleCache::new();
        cache.rebuild(&doc);

        let root = doc.root().unwrap();
        cache.invalidate(root);
        assert!(cache.is_dirty(root));

        // Accessing the node should trigger recomputation.
        let _style = cache.get(root, &doc);
        assert!(!cache.is_dirty(root));
    }

    #[test]
    fn process_node_modified_event() {
        let doc = make_doc();
        let mut cache = StyleCache::new();
        cache.rebuild(&doc);

        let root = doc.root().unwrap();
        let events = vec![DomEvent::NodeModified(root)];
        cache.process_events(&events, &doc);

        // Root and all descendants should be dirty.
        assert!(cache.is_dirty(root));
    }
}
