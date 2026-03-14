//! Objects panel data model (11.4.2).
//!
//! A hierarchical tree of SVG object entries for the objects panel.

/// A single entry in the object tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectEntry {
    /// Unique object identifier.
    pub id: String,
    /// Display name of the object.
    pub name: String,
    /// SVG element type name (e.g. `"rect"`, `"path"`, `"g"`).
    pub type_name: String,
    /// Whether the object is visible.
    pub visible: bool,
    /// Whether the object is locked.
    pub locked: bool,
    /// Nesting depth in the tree (0 = top-level).
    pub depth: usize,
    /// Child objects (for groups and container elements).
    pub children: Vec<ObjectEntry>,
}

impl ObjectEntry {
    /// Create a new visible, unlocked leaf object entry.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_name: type_name.into(),
            visible: true,
            locked: false,
            depth: 0,
            children: Vec::new(),
        }
    }

    /// Create a group entry with children.
    pub fn group(
        id: impl Into<String>,
        name: impl Into<String>,
        children: Vec<ObjectEntry>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_name: "g".to_string(),
            visible: true,
            locked: false,
            depth: 0,
            children,
        }
    }

    /// Return the total number of objects in this sub-tree (including self).
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(|c| c.count()).sum::<usize>()
    }

    /// Flatten the tree into a depth-annotated list suitable for rendering.
    pub fn flatten(&self, base_depth: usize) -> Vec<ObjectEntry> {
        let mut result = Vec::new();
        let mut entry = self.clone();
        entry.depth = base_depth;
        let children = std::mem::take(&mut entry.children);
        result.push(entry);
        for child in &children {
            result.extend(child.flatten(base_depth + 1));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_entry() {
        let obj = ObjectEntry::new("r1", "Rectangle", "rect");
        assert_eq!(obj.type_name, "rect");
        assert!(obj.children.is_empty());
        assert_eq!(obj.count(), 1);
    }

    #[test]
    fn group_count() {
        let group = ObjectEntry::group(
            "g1",
            "Group",
            vec![
                ObjectEntry::new("r1", "Rect", "rect"),
                ObjectEntry::new("c1", "Circle", "circle"),
            ],
        );
        assert_eq!(group.count(), 3);
    }

    #[test]
    fn flatten_tree() {
        let group = ObjectEntry::group(
            "g1",
            "Group",
            vec![
                ObjectEntry::new("r1", "Rect", "rect"),
                ObjectEntry::new("c1", "Circle", "circle"),
            ],
        );
        let flat = group.flatten(0);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[1].depth, 1);
        assert_eq!(flat[2].depth, 1);
    }
}
