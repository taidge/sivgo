//! Document Resources panel data model (11.4.20).
//!
//! Inventories gradients, patterns, fonts, images, and filters used in the
//! document and supports cleanup of unused resources.

/// Information about a single reusable resource in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceInfo {
    /// Resource element id.
    pub id: String,
    /// Human-readable name (may equal `id` if no label is set).
    pub name: String,
    /// Number of elements referencing this resource.
    pub usage_count: usize,
    /// Resource category / type label (e.g. `"linearGradient"`, `"pattern"`).
    pub resource_type: String,
}

impl ResourceInfo {
    /// Create a new resource info entry.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        resource_type: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            usage_count: 0,
            resource_type: resource_type.into(),
        }
    }

    /// Return `true` if the resource is not referenced by any element.
    pub fn is_unused(&self) -> bool {
        self.usage_count == 0
    }
}

/// State for the Document Resources panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentResourcesPanel {
    /// Gradient definitions found in the document.
    pub gradients: Vec<ResourceInfo>,
    /// Pattern definitions found in the document.
    pub patterns: Vec<ResourceInfo>,
    /// Font families used in the document.
    pub fonts: Vec<String>,
    /// Linked / embedded images.
    pub images: Vec<ResourceInfo>,
    /// Filter definitions.
    pub filters: Vec<ResourceInfo>,
}

impl Default for DocumentResourcesPanel {
    fn default() -> Self {
        Self {
            gradients: Vec::new(),
            patterns: Vec::new(),
            fonts: Vec::new(),
            images: Vec::new(),
            filters: Vec::new(),
        }
    }
}

impl DocumentResourcesPanel {
    /// Clear all resource lists and re-populate from the document.
    ///
    /// The actual DOM walk is performed by the integration layer.  This method
    /// resets state so that the caller can push new entries.
    pub fn scan_document(&mut self) {
        self.gradients.clear();
        self.patterns.clear();
        self.fonts.clear();
        self.images.clear();
        self.filters.clear();
    }

    /// Return all resources (across every category) that have zero references.
    pub fn unused_resources(&self) -> Vec<&ResourceInfo> {
        self.gradients
            .iter()
            .chain(self.patterns.iter())
            .chain(self.images.iter())
            .chain(self.filters.iter())
            .filter(|r| r.is_unused())
            .collect()
    }

    /// Remove all unused resources from every category.
    /// Returns the number of resources removed.
    pub fn remove_unused(&mut self) -> usize {
        let before = self.total_count();
        self.gradients.retain(|r| !r.is_unused());
        self.patterns.retain(|r| !r.is_unused());
        self.images.retain(|r| !r.is_unused());
        self.filters.retain(|r| !r.is_unused());
        before - self.total_count()
    }

    /// Total number of non-font resources.
    pub fn total_count(&self) -> usize {
        self.gradients.len() + self.patterns.len() + self.images.len() + self.filters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_panel() -> DocumentResourcesPanel {
        let mut panel = DocumentResourcesPanel::default();
        let mut g1 = ResourceInfo::new("grad1", "Blue Gradient", "linearGradient");
        g1.usage_count = 3;
        let g2 = ResourceInfo::new("grad2", "Unused Gradient", "linearGradient");
        panel.gradients.push(g1);
        panel.gradients.push(g2);

        let mut p1 = ResourceInfo::new("pat1", "Dots", "pattern");
        p1.usage_count = 1;
        panel.patterns.push(p1);

        let img = ResourceInfo::new("img1", "logo.png", "image");
        panel.images.push(img);

        let mut f1 = ResourceInfo::new("filt1", "Blur", "filter");
        f1.usage_count = 2;
        panel.filters.push(f1);

        panel.fonts.push("Roboto".to_string());
        panel.fonts.push("Arial".to_string());
        panel
    }

    #[test]
    fn default_is_empty() {
        let panel = DocumentResourcesPanel::default();
        assert_eq!(panel.total_count(), 0);
        assert!(panel.fonts.is_empty());
    }

    #[test]
    fn unused_resources() {
        let panel = sample_panel();
        let unused = panel.unused_resources();
        assert_eq!(unused.len(), 2); // grad2 and img1
        let ids: Vec<&str> = unused.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"grad2"));
        assert!(ids.contains(&"img1"));
    }

    #[test]
    fn remove_unused() {
        let mut panel = sample_panel();
        let removed = panel.remove_unused();
        assert_eq!(removed, 2);
        assert_eq!(panel.gradients.len(), 1);
        assert_eq!(panel.images.len(), 0);
    }

    #[test]
    fn scan_document_clears() {
        let mut panel = sample_panel();
        panel.scan_document();
        assert_eq!(panel.total_count(), 0);
        assert!(panel.fonts.is_empty());
    }

    #[test]
    fn resource_info_is_unused() {
        let r = ResourceInfo::new("x", "X", "type");
        assert!(r.is_unused());
        let mut r2 = ResourceInfo::new("y", "Y", "type");
        r2.usage_count = 1;
        assert!(!r2.is_unused());
    }

    #[test]
    fn total_count() {
        let panel = sample_panel();
        assert_eq!(panel.total_count(), 5);
    }
}
