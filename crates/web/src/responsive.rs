//! Responsive layout management.
//!
//! Adapts the editor UI to different screen sizes using breakpoints,
//! and tracks the visibility of sidebars and collapsible panels.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Screen-width breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutBreakpoint {
    /// Width < 768 CSS pixels.
    Mobile,
    /// 768 <= width <= 1024 CSS pixels.
    Tablet,
    /// Width > 1024 CSS pixels.
    Desktop,
}

impl LayoutBreakpoint {
    /// Determine the breakpoint for a given width in CSS pixels.
    pub fn from_width(width: f64) -> Self {
        if width < 768.0 {
            Self::Mobile
        } else if width <= 1024.0 {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }
}

impl Default for LayoutBreakpoint {
    fn default() -> Self {
        Self::Desktop
    }
}

/// A snapshot of layout parameters derived from the current breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    pub breakpoint: LayoutBreakpoint,
    pub sidebar_visible: bool,
    pub sidebar_width: f64,
    pub toolbar_position: String,
}

/// Responsive layout state.
#[derive(Debug, Clone)]
pub struct ResponsiveLayout {
    /// Current breakpoint based on viewport width.
    pub breakpoint: LayoutBreakpoint,
    /// Whether the sidebar is currently visible.
    pub sidebar_visible: bool,
    /// Width of the sidebar in CSS pixels.
    pub sidebar_width: f64,
    /// Map of panel name to collapsed state.
    pub panel_collapsed: HashMap<String, bool>,
}

impl Default for ResponsiveLayout {
    fn default() -> Self {
        Self {
            breakpoint: LayoutBreakpoint::Desktop,
            sidebar_visible: true,
            sidebar_width: 260.0,
            panel_collapsed: HashMap::new(),
        }
    }
}

impl ResponsiveLayout {
    /// Create a new layout with desktop defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the layout based on the new viewport width.
    ///
    /// On mobile, the sidebar is hidden and its width reduced. On tablet
    /// the sidebar is narrower. On desktop the full sidebar is shown.
    pub fn update_width(&mut self, width: f64) {
        self.breakpoint = LayoutBreakpoint::from_width(width);
        match self.breakpoint {
            LayoutBreakpoint::Mobile => {
                self.sidebar_visible = false;
                self.sidebar_width = 0.0;
            }
            LayoutBreakpoint::Tablet => {
                self.sidebar_visible = true;
                self.sidebar_width = 200.0;
            }
            LayoutBreakpoint::Desktop => {
                self.sidebar_visible = true;
                self.sidebar_width = 260.0;
            }
        }
    }

    /// Toggle the sidebar between visible and hidden.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible {
            self.sidebar_width = 0.0;
        } else {
            // Restore to the breakpoint default.
            match self.breakpoint {
                LayoutBreakpoint::Mobile => self.sidebar_width = 240.0, // overlay
                LayoutBreakpoint::Tablet => self.sidebar_width = 200.0,
                LayoutBreakpoint::Desktop => self.sidebar_width = 260.0,
            }
        }
    }

    /// Collapse or expand a named panel.
    pub fn collapse_panel(&mut self, name: &str) {
        let entry = self.panel_collapsed.entry(name.to_string()).or_insert(false);
        *entry = !*entry;
    }

    /// Query whether a panel is collapsed.
    pub fn is_panel_collapsed(&self, name: &str) -> bool {
        self.panel_collapsed.get(name).copied().unwrap_or(false)
    }

    /// Generate a `LayoutConfig` snapshot of the current state.
    pub fn get_layout(&self) -> LayoutConfig {
        LayoutConfig {
            breakpoint: self.breakpoint,
            sidebar_visible: self.sidebar_visible,
            sidebar_width: self.sidebar_width,
            toolbar_position: match self.breakpoint {
                LayoutBreakpoint::Mobile => "bottom".to_string(),
                _ => "top".to_string(),
            },
        }
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_from_width() {
        assert_eq!(LayoutBreakpoint::from_width(320.0), LayoutBreakpoint::Mobile);
        assert_eq!(LayoutBreakpoint::from_width(767.9), LayoutBreakpoint::Mobile);
        assert_eq!(LayoutBreakpoint::from_width(768.0), LayoutBreakpoint::Tablet);
        assert_eq!(
            LayoutBreakpoint::from_width(1024.0),
            LayoutBreakpoint::Tablet
        );
        assert_eq!(
            LayoutBreakpoint::from_width(1025.0),
            LayoutBreakpoint::Desktop
        );
        assert_eq!(
            LayoutBreakpoint::from_width(1920.0),
            LayoutBreakpoint::Desktop
        );
    }

    #[test]
    fn default_layout_is_desktop() {
        let layout = ResponsiveLayout::new();
        assert_eq!(layout.breakpoint, LayoutBreakpoint::Desktop);
        assert!(layout.sidebar_visible);
        assert!((layout.sidebar_width - 260.0).abs() < f64::EPSILON);
    }

    #[test]
    fn update_width_mobile() {
        let mut layout = ResponsiveLayout::new();
        layout.update_width(375.0);
        assert_eq!(layout.breakpoint, LayoutBreakpoint::Mobile);
        assert!(!layout.sidebar_visible);
        assert!((layout.sidebar_width).abs() < f64::EPSILON);
    }

    #[test]
    fn update_width_tablet() {
        let mut layout = ResponsiveLayout::new();
        layout.update_width(800.0);
        assert_eq!(layout.breakpoint, LayoutBreakpoint::Tablet);
        assert!(layout.sidebar_visible);
        assert!((layout.sidebar_width - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn toggle_sidebar() {
        let mut layout = ResponsiveLayout::new();
        assert!(layout.sidebar_visible);

        layout.toggle_sidebar();
        assert!(!layout.sidebar_visible);
        assert!((layout.sidebar_width).abs() < f64::EPSILON);

        layout.toggle_sidebar();
        assert!(layout.sidebar_visible);
        assert!((layout.sidebar_width - 260.0).abs() < f64::EPSILON);
    }

    #[test]
    fn collapse_panel_toggles() {
        let mut layout = ResponsiveLayout::new();
        assert!(!layout.is_panel_collapsed("layers"));

        layout.collapse_panel("layers");
        assert!(layout.is_panel_collapsed("layers"));

        layout.collapse_panel("layers");
        assert!(!layout.is_panel_collapsed("layers"));
    }

    #[test]
    fn get_layout_config() {
        let layout = ResponsiveLayout::new();
        let config = layout.get_layout();
        assert_eq!(config.breakpoint, LayoutBreakpoint::Desktop);
        assert!(config.sidebar_visible);
        assert_eq!(config.toolbar_position, "top");
    }

    #[test]
    fn mobile_toolbar_at_bottom() {
        let mut layout = ResponsiveLayout::new();
        layout.update_width(375.0);
        let config = layout.get_layout();
        assert_eq!(config.toolbar_position, "bottom");
    }
}
