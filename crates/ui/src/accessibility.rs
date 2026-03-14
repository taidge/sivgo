//! Screen reader and accessibility support (17.7.2).
//!
//! Provides ARIA attribute modelling, an accessibility announcement queue,
//! and helper methods for generating human-readable descriptions of
//! editor state for screen readers.

use std::collections::HashMap;

/// ARIA live region politeness level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveRegion {
    /// Updates are announced at the next graceful opportunity.
    Polite,
    /// Updates are announced immediately, interrupting any current speech.
    Assertive,
    /// The region is not live; updates are not announced.
    Off,
}

impl LiveRegion {
    /// Return the ARIA attribute value string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Polite => "polite",
            Self::Assertive => "assertive",
            Self::Off => "off",
        }
    }
}

/// ARIA attributes for a single UI element.
#[derive(Debug, Clone)]
pub struct AriaAttributes {
    /// The ARIA role (e.g. "button", "toolbar", "dialog").
    pub role: String,
    /// Human-readable label (`aria-label`).
    pub label: String,
    /// ID of the element that describes this one (`aria-describedby`).
    pub described_by: Option<String>,
    /// Live region setting (`aria-live`).
    pub live_region: Option<LiveRegion>,
    /// Whether a collapsible section is expanded (`aria-expanded`).
    pub expanded: Option<bool>,
    /// Whether this element is selected (`aria-selected`).
    pub selected: Option<bool>,
    /// Whether this element is disabled (`aria-disabled`).
    pub disabled: Option<bool>,
    /// Whether this element is hidden from the accessibility tree.
    pub hidden: bool,
}

impl AriaAttributes {
    /// Create a new set of ARIA attributes with the given role and label.
    pub fn new(role: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            label: label.into(),
            described_by: None,
            live_region: None,
            expanded: None,
            selected: None,
            disabled: None,
            hidden: false,
        }
    }
}

impl Default for AriaAttributes {
    fn default() -> Self {
        Self {
            role: String::new(),
            label: String::new(),
            described_by: None,
            live_region: None,
            expanded: None,
            selected: None,
            disabled: None,
            hidden: false,
        }
    }
}

/// Manages accessibility state for the editor.
///
/// Tracks ARIA attributes per element and maintains a queue of
/// announcements that should be read out by a screen reader.
#[derive(Debug, Clone, Default)]
pub struct AccessibilityManager {
    /// Queue of text announcements to be read by the screen reader.
    pub announcements: Vec<String>,
    /// ARIA attributes keyed by element ID.
    pub element_attrs: HashMap<String, AriaAttributes>,
}

impl AccessibilityManager {
    /// Create a new, empty accessibility manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a text announcement for the screen reader.
    pub fn announce(&mut self, text: &str) {
        self.announcements.push(text.to_string());
    }

    /// Take (drain) all pending announcements.
    pub fn take_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.announcements)
    }

    /// Set or replace the ARIA attributes for an element.
    pub fn set_attrs(&mut self, element_id: &str, attrs: AriaAttributes) {
        self.element_attrs.insert(element_id.to_string(), attrs);
    }

    /// Get the ARIA attributes for an element, if set.
    pub fn get_attrs(&self, element_id: &str) -> Option<&AriaAttributes> {
        self.element_attrs.get(element_id)
    }

    /// Remove the ARIA attributes for an element.
    pub fn remove_attrs(&mut self, element_id: &str) {
        self.element_attrs.remove(element_id);
    }

    /// Generate a human-readable description of the current selection
    /// suitable for screen reader announcement.
    ///
    /// # Arguments
    /// * `count` -- number of selected objects
    /// * `types` -- element type names of the selected objects
    pub fn describe_selection(&self, count: usize, types: &[String]) -> String {
        if count == 0 {
            return "No objects selected".to_string();
        }
        if count == 1 {
            let type_name = types
                .first()
                .map(|s| s.as_str())
                .unwrap_or("object");
            return format!("1 {type_name} selected");
        }

        // Multiple objects: summarise types.
        let mut type_counts: HashMap<&str, usize> = HashMap::new();
        for t in types {
            *type_counts.entry(t.as_str()).or_insert(0) += 1;
        }

        if type_counts.len() == 1 {
            let (name, _) = type_counts.iter().next().unwrap();
            format!("{count} {name}s selected")
        } else {
            let mut parts: Vec<String> = type_counts
                .iter()
                .map(|(name, c)| {
                    if *c == 1 {
                        format!("{c} {name}")
                    } else {
                        format!("{c} {name}s")
                    }
                })
                .collect();
            parts.sort();
            format!("{count} objects selected: {}", parts.join(", "))
        }
    }

    /// Generate a human-readable description of a tool for screen reader
    /// announcement when the tool is activated.
    pub fn describe_tool(&self, name: &str) -> String {
        match name {
            "selector" | "select" => {
                "Selector tool active. Click to select objects, drag to move.".to_string()
            }
            "node" => {
                "Node tool active. Click nodes to select, drag to move.".to_string()
            }
            "rect" | "rectangle" => {
                "Rectangle tool active. Click and drag to draw a rectangle.".to_string()
            }
            "ellipse" | "circle" => {
                "Ellipse tool active. Click and drag to draw an ellipse.".to_string()
            }
            "pen" | "bezier" => {
                "Pen tool active. Click to place points, click first point to close path."
                    .to_string()
            }
            "pencil" | "freehand" => {
                "Pencil tool active. Click and drag to draw freehand.".to_string()
            }
            "text" => {
                "Text tool active. Click to create a text object.".to_string()
            }
            "gradient" => {
                "Gradient tool active. Drag on an object to adjust its gradient.".to_string()
            }
            "zoom" => {
                "Zoom tool active. Click to zoom in, Shift+click to zoom out.".to_string()
            }
            "dropper" | "eyedropper" => {
                "Colour picker tool active. Click an object to sample its colour.".to_string()
            }
            "star" => {
                "Star tool active. Click and drag to draw a star or polygon.".to_string()
            }
            "measure" => {
                "Measure tool active. Click and drag to measure distance.".to_string()
            }
            _ => format!("{name} tool active."),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_is_empty() {
        let mgr = AccessibilityManager::new();
        assert!(mgr.announcements.is_empty());
        assert!(mgr.element_attrs.is_empty());
    }

    #[test]
    fn announce_queues_text() {
        let mut mgr = AccessibilityManager::new();
        mgr.announce("File saved");
        mgr.announce("Undo: delete rectangle");
        assert_eq!(mgr.announcements.len(), 2);
        assert_eq!(mgr.announcements[0], "File saved");
    }

    #[test]
    fn take_announcements_drains() {
        let mut mgr = AccessibilityManager::new();
        mgr.announce("Hello");
        mgr.announce("World");
        let taken = mgr.take_announcements();
        assert_eq!(taken.len(), 2);
        assert!(mgr.announcements.is_empty());
    }

    #[test]
    fn set_and_get_attrs() {
        let mut mgr = AccessibilityManager::new();
        let attrs = AriaAttributes::new("button", "Save Document");
        mgr.set_attrs("save-btn", attrs);

        let retrieved = mgr.get_attrs("save-btn").unwrap();
        assert_eq!(retrieved.role, "button");
        assert_eq!(retrieved.label, "Save Document");
    }

    #[test]
    fn get_attrs_missing_returns_none() {
        let mgr = AccessibilityManager::new();
        assert!(mgr.get_attrs("nonexistent").is_none());
    }

    #[test]
    fn remove_attrs() {
        let mut mgr = AccessibilityManager::new();
        mgr.set_attrs("btn", AriaAttributes::new("button", "X"));
        mgr.remove_attrs("btn");
        assert!(mgr.get_attrs("btn").is_none());
    }

    #[test]
    fn describe_selection_empty() {
        let mgr = AccessibilityManager::new();
        assert_eq!(mgr.describe_selection(0, &[]), "No objects selected");
    }

    #[test]
    fn describe_selection_single() {
        let mgr = AccessibilityManager::new();
        let types = vec!["rectangle".to_string()];
        assert_eq!(mgr.describe_selection(1, &types), "1 rectangle selected");
    }

    #[test]
    fn describe_selection_single_no_type() {
        let mgr = AccessibilityManager::new();
        assert_eq!(mgr.describe_selection(1, &[]), "1 object selected");
    }

    #[test]
    fn describe_selection_multiple_same_type() {
        let mgr = AccessibilityManager::new();
        let types = vec!["circle".to_string(), "circle".to_string(), "circle".to_string()];
        assert_eq!(mgr.describe_selection(3, &types), "3 circles selected");
    }

    #[test]
    fn describe_selection_mixed_types() {
        let mgr = AccessibilityManager::new();
        let types = vec![
            "rectangle".to_string(),
            "circle".to_string(),
            "rectangle".to_string(),
        ];
        let result = mgr.describe_selection(3, &types);
        assert!(result.starts_with("3 objects selected:"));
        assert!(result.contains("circle"));
        assert!(result.contains("rectangle"));
    }

    #[test]
    fn describe_tool_known() {
        let mgr = AccessibilityManager::new();
        let desc = mgr.describe_tool("pen");
        assert!(desc.contains("Pen tool"));
        assert!(desc.contains("click"));
    }

    #[test]
    fn describe_tool_unknown() {
        let mgr = AccessibilityManager::new();
        let desc = mgr.describe_tool("custom_tool");
        assert_eq!(desc, "custom_tool tool active.");
    }

    #[test]
    fn live_region_as_str() {
        assert_eq!(LiveRegion::Polite.as_str(), "polite");
        assert_eq!(LiveRegion::Assertive.as_str(), "assertive");
        assert_eq!(LiveRegion::Off.as_str(), "off");
    }

    #[test]
    fn aria_attributes_defaults() {
        let attrs = AriaAttributes::default();
        assert!(attrs.role.is_empty());
        assert!(attrs.label.is_empty());
        assert!(attrs.described_by.is_none());
        assert!(attrs.live_region.is_none());
        assert!(attrs.expanded.is_none());
        assert!(attrs.selected.is_none());
        assert!(attrs.disabled.is_none());
        assert!(!attrs.hidden);
    }

    #[test]
    fn aria_attributes_with_all_fields() {
        let attrs = AriaAttributes {
            role: "dialog".to_string(),
            label: "Export Options".to_string(),
            described_by: Some("desc-1".to_string()),
            live_region: Some(LiveRegion::Polite),
            expanded: Some(true),
            selected: Some(false),
            disabled: Some(false),
            hidden: false,
        };
        assert_eq!(attrs.role, "dialog");
        assert_eq!(attrs.live_region, Some(LiveRegion::Polite));
        assert_eq!(attrs.expanded, Some(true));
    }
}
