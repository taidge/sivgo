//! Layers panel data model (11.4.1).
//!
//! Provides a plain data representation of a layer entry used by the layers
//! panel UI.  This is a pure data struct with no Dioxus dependency.

/// A single layer entry in the layers panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerEntry {
    /// Unique identifier for this layer.
    pub id: String,
    /// Human-readable layer name.
    pub name: String,
    /// Whether the layer is visible.
    pub visible: bool,
    /// Whether the layer is locked (non-editable).
    pub locked: bool,
    /// Layer opacity in the range `[0.0, 1.0]`.
    pub opacity: f64,
    /// CSS blend mode name (e.g. `"Normal"`, `"Multiply"`).
    pub blend_mode: String,
    /// Nesting depth (0 = top-level layer).
    pub depth: usize,
}

impl LayerEntry {
    /// Create a new visible, unlocked layer at full opacity with normal blending.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: "Normal".to_string(),
            depth: 0,
        }
    }

    /// Toggle the visibility flag and return the new state.
    pub fn toggle_visible(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }

    /// Toggle the locked flag and return the new state.
    pub fn toggle_locked(&mut self) -> bool {
        self.locked = !self.locked;
        self.locked
    }
}

impl Default for LayerEntry {
    fn default() -> Self {
        Self::new("layer-0", "Layer 1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_layer_defaults() {
        let layer = LayerEntry::new("l1", "Background");
        assert_eq!(layer.id, "l1");
        assert_eq!(layer.name, "Background");
        assert!(layer.visible);
        assert!(!layer.locked);
        assert!((layer.opacity - 1.0).abs() < f64::EPSILON);
        assert_eq!(layer.blend_mode, "Normal");
        assert_eq!(layer.depth, 0);
    }

    #[test]
    fn toggle_visibility() {
        let mut layer = LayerEntry::default();
        assert!(layer.visible);
        let new = layer.toggle_visible();
        assert!(!new);
        assert!(!layer.visible);
        let new = layer.toggle_visible();
        assert!(new);
    }

    #[test]
    fn toggle_lock() {
        let mut layer = LayerEntry::default();
        assert!(!layer.locked);
        layer.toggle_locked();
        assert!(layer.locked);
    }
}
