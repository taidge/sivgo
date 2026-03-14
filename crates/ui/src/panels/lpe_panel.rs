//! Live Path Effects (LPE) panel data model (11.4.17).
//!
//! Manages a stack of live path effects applied to the selected path,
//! with per-effect parameters and enable/disable toggling.

/// A single live path effect entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LpeEntry {
    /// Effect type name (e.g. `"Pattern Along Path"`, `"Bend"`).
    pub effect_name: String,
    /// Whether this effect is currently applied.
    pub enabled: bool,
    /// Named numeric parameters for the effect.
    pub params: Vec<(String, f64)>,
}

impl LpeEntry {
    /// Create a new enabled effect with no parameters.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            effect_name: name.into(),
            enabled: true,
            params: Vec::new(),
        }
    }

    /// Create a new effect with the given parameters.
    pub fn with_params(name: impl Into<String>, params: Vec<(String, f64)>) -> Self {
        Self {
            effect_name: name.into(),
            enabled: true,
            params,
        }
    }

    /// Get a parameter value by name.
    pub fn get_param(&self, name: &str) -> Option<f64> {
        self.params.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }
}

/// State for the Live Path Effects panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LpePanel {
    /// Ordered stack of effects (applied top to bottom).
    pub effects: Vec<LpeEntry>,
    /// Index of the currently selected effect, if any.
    pub selected_index: Option<usize>,
}

impl Default for LpePanel {
    fn default() -> Self {
        Self {
            effects: Vec::new(),
            selected_index: None,
        }
    }
}

impl LpePanel {
    /// Add a new effect to the end of the stack.  Returns the index of the
    /// new entry.
    pub fn add_effect(&mut self, name: impl Into<String>) -> usize {
        self.effects.push(LpeEntry::new(name));
        let idx = self.effects.len() - 1;
        self.selected_index = Some(idx);
        idx
    }

    /// Remove an effect by index.  Returns `true` if the index was valid.
    pub fn remove_effect(&mut self, idx: usize) -> bool {
        if idx >= self.effects.len() {
            return false;
        }
        self.effects.remove(idx);
        // Adjust selection.
        match self.selected_index {
            Some(sel) if sel == idx => {
                self.selected_index = if self.effects.is_empty() {
                    None
                } else {
                    Some(idx.min(self.effects.len() - 1))
                };
            }
            Some(sel) if sel > idx => {
                self.selected_index = Some(sel - 1);
            }
            _ => {}
        }
        true
    }

    /// Move an effect from index `from` to index `to`.
    /// Returns `true` if the reorder was valid.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.effects.len() || to >= self.effects.len() || from == to {
            return false;
        }
        let entry = self.effects.remove(from);
        self.effects.insert(to, entry);
        // Update selection to follow the moved item if it was selected.
        if self.selected_index == Some(from) {
            self.selected_index = Some(to);
        }
        true
    }

    /// Toggle the enabled state of an effect.  Returns the new state, or
    /// `None` if the index was invalid.
    pub fn toggle(&mut self, idx: usize) -> Option<bool> {
        if let Some(entry) = self.effects.get_mut(idx) {
            entry.enabled = !entry.enabled;
            Some(entry.enabled)
        } else {
            None
        }
    }

    /// Set a parameter on an effect.  If the parameter already exists its value
    /// is updated; otherwise a new parameter is appended.
    /// Returns `true` if the effect index was valid.
    pub fn set_param(&mut self, idx: usize, name: impl Into<String>, value: f64) -> bool {
        if let Some(entry) = self.effects.get_mut(idx) {
            let name = name.into();
            if let Some(p) = entry.params.iter_mut().find(|(n, _)| *n == name) {
                p.1 = value;
            } else {
                entry.params.push((name, value));
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_effect() {
        let mut panel = LpePanel::default();
        let idx = panel.add_effect("Bend");
        assert_eq!(idx, 0);
        assert_eq!(panel.selected_index, Some(0));
        assert_eq!(panel.effects[0].effect_name, "Bend");
        assert!(panel.effects[0].enabled);
    }

    #[test]
    fn remove_effect() {
        let mut panel = LpePanel::default();
        panel.add_effect("A");
        panel.add_effect("B");
        panel.add_effect("C");
        panel.selected_index = Some(1);
        assert!(panel.remove_effect(1));
        assert_eq!(panel.effects.len(), 2);
        // Selection should clamp to valid range.
        assert_eq!(panel.selected_index, Some(1)); // now points to "C"
        assert_eq!(panel.effects[1].effect_name, "C");
    }

    #[test]
    fn remove_last_effect() {
        let mut panel = LpePanel::default();
        panel.add_effect("A");
        panel.selected_index = Some(0);
        panel.remove_effect(0);
        assert!(panel.effects.is_empty());
        assert_eq!(panel.selected_index, None);
    }

    #[test]
    fn remove_invalid() {
        let mut panel = LpePanel::default();
        assert!(!panel.remove_effect(5));
    }

    #[test]
    fn reorder() {
        let mut panel = LpePanel::default();
        panel.add_effect("A");
        panel.add_effect("B");
        panel.add_effect("C");
        panel.selected_index = Some(0);
        assert!(panel.reorder(0, 2));
        assert_eq!(panel.effects[0].effect_name, "B");
        assert_eq!(panel.effects[2].effect_name, "A");
        assert_eq!(panel.selected_index, Some(2)); // follows moved item
    }

    #[test]
    fn reorder_invalid() {
        let mut panel = LpePanel::default();
        panel.add_effect("A");
        assert!(!panel.reorder(0, 5));
        assert!(!panel.reorder(0, 0));
    }

    #[test]
    fn toggle() {
        let mut panel = LpePanel::default();
        panel.add_effect("A");
        assert_eq!(panel.toggle(0), Some(false));
        assert!(!panel.effects[0].enabled);
        assert_eq!(panel.toggle(0), Some(true));
    }

    #[test]
    fn toggle_invalid() {
        let mut panel = LpePanel::default();
        assert_eq!(panel.toggle(0), None);
    }

    #[test]
    fn set_param_new() {
        let mut panel = LpePanel::default();
        panel.add_effect("Bend");
        assert!(panel.set_param(0, "angle", 45.0));
        assert_eq!(panel.effects[0].get_param("angle"), Some(45.0));
    }

    #[test]
    fn set_param_update() {
        let mut panel = LpePanel::default();
        panel.add_effect("Bend");
        panel.set_param(0, "angle", 45.0);
        panel.set_param(0, "angle", 90.0);
        assert_eq!(panel.effects[0].get_param("angle"), Some(90.0));
        assert_eq!(panel.effects[0].params.len(), 1);
    }

    #[test]
    fn set_param_invalid() {
        let mut panel = LpePanel::default();
        assert!(!panel.set_param(0, "x", 1.0));
    }
}
