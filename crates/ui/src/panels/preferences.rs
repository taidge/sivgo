//! Preferences dialog data model (11.4.9).
//!
//! Stores application-wide preferences organised by category: interface,
//! I/O, rendering, tool defaults, and snap defaults.

use std::collections::HashMap;

/// UI theme selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl Theme {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "System",
        }
    }

    /// All themes in display order.
    pub fn all() -> &'static [Theme] {
        &[Theme::Light, Theme::Dark, Theme::System]
    }
}

/// Icon size preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    Small,
    Medium,
    Large,
}

impl IconSize {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            IconSize::Small => "Small",
            IconSize::Medium => "Medium",
            IconSize::Large => "Large",
        }
    }
}

/// Panel layout mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLayout {
    Default,
    Compact,
    Wide,
}

impl PanelLayout {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            PanelLayout::Default => "Default",
            PanelLayout::Compact => "Compact",
            PanelLayout::Wide => "Wide",
        }
    }
}

/// Display mode for on-canvas rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Normal,
    Outline,
    NoFilters,
    EnhancedThinLine,
    PrintPreview,
    ProofColors,
}

impl DisplayMode {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            DisplayMode::Normal => "Normal",
            DisplayMode::Outline => "Outline",
            DisplayMode::NoFilters => "No Filters",
            DisplayMode::EnhancedThinLine => "Enhanced Thin Line",
            DisplayMode::PrintPreview => "Print Preview",
            DisplayMode::ProofColors => "Proof Colors",
        }
    }
}

/// Default export / save format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultFormat {
    Svg,
    InkscapeSvg,
    Pdf,
    Png,
}

impl DefaultFormat {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            DefaultFormat::Svg => "Plain SVG",
            DefaultFormat::InkscapeSvg => "Inkscape SVG",
            DefaultFormat::Pdf => "PDF",
            DefaultFormat::Png => "PNG",
        }
    }
}

/// Interface-related preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfacePrefs {
    /// Colour theme.
    pub theme: Theme,
    /// Toolbar / panel icon size.
    pub icon_size: IconSize,
    /// Panel arrangement mode.
    pub panel_layout: PanelLayout,
}

impl Default for InterfacePrefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            icon_size: IconSize::Medium,
            panel_layout: PanelLayout::Default,
        }
    }
}

/// I/O-related preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct IoPrefs {
    /// Default file format for saving.
    pub default_format: DefaultFormat,
    /// Auto-save interval in seconds; 0 = disabled.
    pub auto_save_interval: u32,
    /// Maximum number of recent files to remember.
    pub recent_count: usize,
}

impl Default for IoPrefs {
    fn default() -> Self {
        Self {
            default_format: DefaultFormat::Svg,
            auto_save_interval: 120,
            recent_count: 10,
        }
    }
}

/// Rendering preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderingPrefs {
    /// Display mode for the canvas.
    pub display_mode: DisplayMode,
    /// Enable anti-aliasing.
    pub anti_alias: bool,
    /// Use GPU acceleration when available.
    pub gpu_enabled: bool,
}

impl Default for RenderingPrefs {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::Normal,
            anti_alias: true,
            gpu_enabled: true,
        }
    }
}

/// Snap-related default preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapDefaults {
    /// Global snap enabled by default.
    pub enabled: bool,
    /// Default snap distance in pixels.
    pub snap_distance: f64,
    /// Snap to grid by default.
    pub snap_to_grid: bool,
    /// Snap to guides by default.
    pub snap_to_guides: bool,
}

impl Default for SnapDefaults {
    fn default() -> Self {
        Self {
            enabled: true,
            snap_distance: 10.0,
            snap_to_grid: true,
            snap_to_guides: true,
        }
    }
}

/// Complete application preferences state.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    /// Per-tool default settings.  Outer key = tool name, inner map = param
    /// name -> value.
    pub tool_defaults: HashMap<String, HashMap<String, String>>,
    /// Interface preferences.
    pub interface: InterfacePrefs,
    /// I/O preferences.
    pub io: IoPrefs,
    /// Rendering preferences.
    pub rendering: RenderingPrefs,
    /// Snap defaults.
    pub snap_defaults: SnapDefaults,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            tool_defaults: HashMap::new(),
            interface: InterfacePrefs::default(),
            io: IoPrefs::default(),
            rendering: RenderingPrefs::default(),
            snap_defaults: SnapDefaults::default(),
        }
    }
}

impl PreferencesState {
    // -- Interface accessors --

    /// Get the current theme.
    pub fn theme(&self) -> Theme {
        self.interface.theme
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: Theme) {
        self.interface.theme = theme;
    }

    /// Get the icon size.
    pub fn icon_size(&self) -> IconSize {
        self.interface.icon_size
    }

    /// Set the icon size.
    pub fn set_icon_size(&mut self, size: IconSize) {
        self.interface.icon_size = size;
    }

    /// Get the panel layout.
    pub fn panel_layout(&self) -> PanelLayout {
        self.interface.panel_layout
    }

    /// Set the panel layout.
    pub fn set_panel_layout(&mut self, layout: PanelLayout) {
        self.interface.panel_layout = layout;
    }

    // -- I/O accessors --

    /// Get the default file format.
    pub fn default_format(&self) -> DefaultFormat {
        self.io.default_format
    }

    /// Set the default file format.
    pub fn set_default_format(&mut self, format: DefaultFormat) {
        self.io.default_format = format;
    }

    /// Get the auto-save interval in seconds.
    pub fn auto_save_interval(&self) -> u32 {
        self.io.auto_save_interval
    }

    /// Set the auto-save interval in seconds.
    pub fn set_auto_save_interval(&mut self, seconds: u32) {
        self.io.auto_save_interval = seconds;
    }

    /// Get the recent-files count.
    pub fn recent_count(&self) -> usize {
        self.io.recent_count
    }

    /// Set the recent-files count.
    pub fn set_recent_count(&mut self, count: usize) {
        self.io.recent_count = count;
    }

    // -- Rendering accessors --

    /// Get the display mode.
    pub fn display_mode(&self) -> DisplayMode {
        self.rendering.display_mode
    }

    /// Set the display mode.
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.rendering.display_mode = mode;
    }

    /// Check whether anti-aliasing is enabled.
    pub fn anti_alias(&self) -> bool {
        self.rendering.anti_alias
    }

    /// Set anti-aliasing.
    pub fn set_anti_alias(&mut self, enabled: bool) {
        self.rendering.anti_alias = enabled;
    }

    /// Check whether GPU acceleration is enabled.
    pub fn gpu_enabled(&self) -> bool {
        self.rendering.gpu_enabled
    }

    /// Set GPU acceleration.
    pub fn set_gpu_enabled(&mut self, enabled: bool) {
        self.rendering.gpu_enabled = enabled;
    }

    // -- Tool defaults --

    /// Get a tool default value.
    pub fn get_tool_default(&self, tool: &str, param: &str) -> Option<&str> {
        self.tool_defaults
            .get(tool)
            .and_then(|m| m.get(param).map(|s| s.as_str()))
    }

    /// Set a tool default value.
    pub fn set_tool_default(
        &mut self,
        tool: impl Into<String>,
        param: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.tool_defaults
            .entry(tool.into())
            .or_default()
            .insert(param.into(), value.into());
    }

    // -- Bulk operations --

    /// Reset all preferences to their defaults.
    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }

    /// Simulate loading preferences from a file path.
    ///
    /// Returns `Ok(())` on success or an error description.
    /// Actual file I/O is handled at a higher layer; this method is a
    /// placeholder for the serialisation boundary.
    pub fn load(&mut self, _path: &str) -> Result<(), String> {
        // TODO: deserialise from file
        Ok(())
    }

    /// Simulate saving preferences to a file path.
    pub fn save(&self, _path: &str) -> Result<(), String> {
        // TODO: serialise to file
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let prefs = PreferencesState::default();
        assert_eq!(prefs.theme(), Theme::System);
        assert_eq!(prefs.icon_size(), IconSize::Medium);
        assert_eq!(prefs.panel_layout(), PanelLayout::Default);
        assert_eq!(prefs.default_format(), DefaultFormat::Svg);
        assert_eq!(prefs.auto_save_interval(), 120);
        assert_eq!(prefs.recent_count(), 10);
        assert!(prefs.anti_alias());
        assert!(prefs.gpu_enabled());
    }

    #[test]
    fn set_and_get_theme() {
        let mut prefs = PreferencesState::default();
        prefs.set_theme(Theme::Dark);
        assert_eq!(prefs.theme(), Theme::Dark);
    }

    #[test]
    fn tool_defaults_round_trip() {
        let mut prefs = PreferencesState::default();
        prefs.set_tool_default("rect", "rx", "5");
        assert_eq!(prefs.get_tool_default("rect", "rx"), Some("5"));
        assert_eq!(prefs.get_tool_default("rect", "ry"), None);
        assert_eq!(prefs.get_tool_default("circle", "r"), None);
    }

    #[test]
    fn reset_to_defaults_clears_changes() {
        let mut prefs = PreferencesState::default();
        prefs.set_theme(Theme::Light);
        prefs.set_auto_save_interval(30);
        prefs.set_tool_default("pen", "stroke", "#ff0000");
        prefs.reset_to_defaults();
        assert_eq!(prefs.theme(), Theme::System);
        assert_eq!(prefs.auto_save_interval(), 120);
        assert!(prefs.tool_defaults.is_empty());
    }

    #[test]
    fn load_save_placeholder() {
        let mut prefs = PreferencesState::default();
        assert!(prefs.load("/tmp/prefs.json").is_ok());
        assert!(prefs.save("/tmp/prefs.json").is_ok());
    }

    #[test]
    fn enum_labels() {
        assert_eq!(Theme::Dark.label(), "Dark");
        assert_eq!(IconSize::Small.label(), "Small");
        assert_eq!(PanelLayout::Compact.label(), "Compact");
        assert_eq!(DisplayMode::Outline.label(), "Outline");
        assert_eq!(DisplayMode::PrintPreview.label(), "Print Preview");
        assert_eq!(DisplayMode::ProofColors.label(), "Proof Colors");
        assert_eq!(DefaultFormat::Png.label(), "PNG");
    }
}
