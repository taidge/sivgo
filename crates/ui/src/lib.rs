//! `sivgo-ui` -- Shared Glory and Sapid UI components for the Sivgo SVG editor.
//!
//! Provides the reusable components used by the desktop editor:
//!
//! * [`canvas::render_to_data_uri`] -- renders the document to a base64 PNG data URI
//! * [`toolbar::Toolbar`] -- vertical tool palette grouped by category
//! * [`tool_controls::ToolControls`] -- dynamic per-tool controls bar
//! * [`menu_model`] -- menu data model with hierarchical `MenuItem` and `default_menu_bar()`
//! * [`status_bar::StatusBar`] -- bottom status bar with cursor, zoom, and selection info
//! * [`layers_panel::LayersPanel`] -- layer list with visibility, lock, opacity, blend mode
//! * [`objects_panel::ObjectsPanel`] -- full object tree view
//! * [`pages_panel::PagesPanel`] -- multi-page manager
//! * [`properties_panel::PropertiesPanel`] -- fill/stroke property editor with paint types
//! * [`align_panel::AlignPanel`] -- alignment and distribution controls
//! * [`transform_panel::TransformPanel`] -- position, scale, rotate, skew
//! * [`text_panel::TextPanel`] -- text-specific property editor
//! * [`rulers::HorizontalRuler`] / [`rulers::VerticalRuler`] -- tick-mark rulers
//! * [`scrollbars::HorizontalScrollbar`] / [`scrollbars::VerticalScrollbar`] -- viewport scrollbars
//! * [`color_bar::ColorBar`] -- quick fill/stroke color indicator
//! * [`doc_properties::DocPropertiesPanel`] -- document properties (page size, background, metadata, grid)
//! * [`preferences_dialog::PreferencesDialog`] -- application preferences dialog
//! * [`filter_editor::FilterEditorPanel`] -- visual SVG filter chain editor
//! * [`lpe_panel::LpePanel`] -- live path effects manager
//! * [`resources_panel::ResourcesPanel`] -- document resource listing and cleanup

pub mod align_panel;
pub mod canvas;
pub mod color_bar;
pub(crate) mod controls;
pub mod doc_properties;
pub mod filter_editor;
pub mod layers_panel;
pub mod lpe_panel;
pub mod menu_model;
pub mod objects_panel;
pub mod pages_panel;
pub mod preferences_dialog;
pub mod properties_panel;
pub mod resources_panel;
pub mod rulers;
pub mod scrollbars;
pub mod status_bar;
pub mod text_panel;
pub mod tool_controls;
pub mod toolbar;
pub mod transform_panel;

/// Cloneable event callback shared by the semantic editor widgets.
pub struct Callback<T>(std::rc::Rc<dyn Fn(T)>);

impl<T> Callback<T> {
    pub fn new(handler: impl Fn(T) + 'static) -> Self {
        Self(std::rc::Rc::new(handler))
    }

    pub fn call(&self, value: T) {
        (self.0)(value);
    }
}

impl<T> Clone for Callback<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Callback<T> {
    fn default() -> Self {
        Self::new(|_| {})
    }
}

// ---------------------------------------------------------------------------
// Re-exports: Glory widgets
// ---------------------------------------------------------------------------
pub use align_panel::AlignPanel;
pub use canvas::render_to_data_uri;
pub use color_bar::ColorBar;
pub use doc_properties::DocPropertiesPanel;
pub use filter_editor::FilterEditorPanel;
pub use layers_panel::LayersPanel;
pub use lpe_panel::LpePanel;
pub use objects_panel::ObjectsPanel;
pub use pages_panel::PagesPanel;
pub use preferences_dialog::PreferencesDialog;
pub use properties_panel::PropertiesPanel;
pub use resources_panel::ResourcesPanel;
pub use rulers::{HorizontalRuler, VerticalRuler};
pub use scrollbars::{HorizontalScrollbar, VerticalScrollbar};
pub use status_bar::StatusBar;
pub use text_panel::{TextAction, TextPanel};
pub use tool_controls::{
    GradientKind, NodeType, PenShape, SegmentType, ToolControlAction, ToolControls,
};
pub use toolbar::Toolbar;
pub use transform_panel::TransformPanel;

// ---------------------------------------------------------------------------
// Re-exports: Data models
// ---------------------------------------------------------------------------
pub use doc_properties::{DocumentMetadata, GridSettings, PageOrientation, PageSizePreset};
pub use filter_editor::{FilterEditorAction, FilterParam, FilterPrimitive, FilterPrimitiveKind};
pub use lpe_panel::{LpeAction, LpeEntry, LpeKind, LpeParam};
pub use preferences_dialog::{PreferencesSection, PreferencesState};
pub use resources_panel::{ResourceEntry, ResourceKind, ResourcesAction};
