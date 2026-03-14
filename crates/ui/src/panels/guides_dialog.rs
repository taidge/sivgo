//! Guides Dialog data model (11.4.19).
//!
//! Manages the set of guide lines (horizontal, vertical, and angled) on the
//! canvas.

/// Orientation / type of a guide line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideOrientation {
    /// Horizontal guide.
    Horizontal,
    /// Vertical guide.
    Vertical,
    /// Angled guide (arbitrary angle).
    Angled,
}

impl GuideOrientation {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            GuideOrientation::Horizontal => "Horizontal",
            GuideOrientation::Vertical => "Vertical",
            GuideOrientation::Angled => "Angled",
        }
    }
}

/// Information about a single guide line.
#[derive(Debug, Clone, PartialEq)]
pub struct GuideInfo {
    /// Unique identifier for the guide.
    pub id: String,
    /// Orientation of the guide.
    pub orientation: GuideOrientation,
    /// Position along the perpendicular axis (in user units).
    pub position: f64,
    /// Angle in degrees (only meaningful for `Angled` guides).
    pub angle: f64,
    /// Optional label displayed next to the guide.
    pub label: String,
    /// Guide colour as RGBA components in `[0.0, 1.0]`.
    pub color: [f64; 4],
}

impl GuideInfo {
    /// Create a horizontal guide at the given Y position.
    pub fn horizontal(id: impl Into<String>, y: f64) -> Self {
        Self {
            id: id.into(),
            orientation: GuideOrientation::Horizontal,
            position: y,
            angle: 0.0,
            label: String::new(),
            color: [0.0, 0.0, 1.0, 1.0], // blue default
        }
    }

    /// Create a vertical guide at the given X position.
    pub fn vertical(id: impl Into<String>, x: f64) -> Self {
        Self {
            id: id.into(),
            orientation: GuideOrientation::Vertical,
            position: x,
            angle: 90.0,
            label: String::new(),
            color: [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Create an angled guide.
    pub fn angled(id: impl Into<String>, position: f64, angle: f64) -> Self {
        Self {
            id: id.into(),
            orientation: GuideOrientation::Angled,
            position,
            angle,
            label: String::new(),
            color: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

/// State for the Guides Dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct GuidesDialogState {
    /// All guides in the document.
    pub guides: Vec<GuideInfo>,
}

impl Default for GuidesDialogState {
    fn default() -> Self {
        Self { guides: Vec::new() }
    }
}

impl GuidesDialogState {
    /// Add a new guide and return its index.
    pub fn add_guide(&mut self, guide: GuideInfo) -> usize {
        self.guides.push(guide);
        self.guides.len() - 1
    }

    /// Remove a guide by index.  Returns `true` if the index was valid.
    pub fn remove_guide(&mut self, idx: usize) -> bool {
        if idx < self.guides.len() {
            self.guides.remove(idx);
            true
        } else {
            false
        }
    }

    /// Update the guide at `idx` with new information.
    /// Returns `true` if the index was valid.
    pub fn update_guide(&mut self, idx: usize, info: GuideInfo) -> bool {
        if let Some(g) = self.guides.get_mut(idx) {
            *g = info;
            true
        } else {
            false
        }
    }

    /// Remove all guides.
    pub fn clear_all(&mut self) {
        self.guides.clear();
    }

    /// Return the number of guides.
    pub fn count(&self) -> usize {
        self.guides.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_horizontal_guide() {
        let mut state = GuidesDialogState::default();
        let idx = state.add_guide(GuideInfo::horizontal("h1", 100.0));
        assert_eq!(idx, 0);
        assert_eq!(state.count(), 1);
        assert_eq!(state.guides[0].orientation, GuideOrientation::Horizontal);
        assert!((state.guides[0].position - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn add_vertical_guide() {
        let mut state = GuidesDialogState::default();
        state.add_guide(GuideInfo::vertical("v1", 50.0));
        assert_eq!(state.guides[0].orientation, GuideOrientation::Vertical);
    }

    #[test]
    fn add_angled_guide() {
        let mut state = GuidesDialogState::default();
        state.add_guide(GuideInfo::angled("a1", 200.0, 45.0));
        assert_eq!(state.guides[0].orientation, GuideOrientation::Angled);
        assert!((state.guides[0].angle - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn remove_guide() {
        let mut state = GuidesDialogState::default();
        state.add_guide(GuideInfo::horizontal("h1", 10.0));
        state.add_guide(GuideInfo::vertical("v1", 20.0));
        assert!(state.remove_guide(0));
        assert_eq!(state.count(), 1);
        assert_eq!(state.guides[0].id, "v1");
    }

    #[test]
    fn remove_guide_invalid() {
        let mut state = GuidesDialogState::default();
        assert!(!state.remove_guide(0));
    }

    #[test]
    fn update_guide() {
        let mut state = GuidesDialogState::default();
        state.add_guide(GuideInfo::horizontal("h1", 10.0));
        let updated = GuideInfo::horizontal("h1", 200.0);
        assert!(state.update_guide(0, updated));
        assert!((state.guides[0].position - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn update_guide_invalid() {
        let mut state = GuidesDialogState::default();
        assert!(!state.update_guide(0, GuideInfo::horizontal("x", 0.0)));
    }

    #[test]
    fn clear_all() {
        let mut state = GuidesDialogState::default();
        state.add_guide(GuideInfo::horizontal("h1", 10.0));
        state.add_guide(GuideInfo::vertical("v1", 20.0));
        state.clear_all();
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn orientation_labels() {
        assert_eq!(GuideOrientation::Horizontal.label(), "Horizontal");
        assert_eq!(GuideOrientation::Vertical.label(), "Vertical");
        assert_eq!(GuideOrientation::Angled.label(), "Angled");
    }
}
