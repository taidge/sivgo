//! `sivgo-ui` -- Shared Dioxus UI components for the Sivgo SVG editor.
//!
//! Provides reusable components used by both the desktop and web targets:
//!
//! * [`canvas::render_to_data_uri`] -- renders the document to a base64 PNG data URI
//! * [`canvas::CanvasView`] -- canvas component with event handlers and selection overlays
//! * [`canvas_state::CanvasState`] -- canvas dimensions and invalidation state
//! * [`toolbar::Toolbar`] -- vertical tool palette grouped by category
//! * [`toolbar_model`] -- toolbar layout data model with `ToolbarItem`, `ToolbarGroup`
//! * [`tool_controls::ToolControls`] -- dynamic per-tool controls bar
//! * [`command_palette::CommandPalette`] -- searchable command palette overlay
//! * [`menu_bar::MenuBar`] -- top menu bar with full dropdown menus
//! * [`menu_model`] -- menu data model with hierarchical `MenuItem` and `default_menu_bar()`
//! * [`status_bar::StatusBar`] -- bottom status bar with cursor, zoom, and selection info
//! * [`statusbar::StatusBarInfo`] -- status bar data model
//! * [`layers_panel::LayersPanel`] -- layer list with visibility, lock, opacity, blend mode
//! * [`objects_panel::ObjectsPanel`] -- full object tree view
//! * [`properties_panel::PropertiesPanel`] -- fill/stroke property editor with paint types
//! * [`align_panel::AlignPanel`] -- alignment and distribution controls
//! * [`transform_panel::TransformPanel`] -- position, scale, rotate, skew
//! * [`xml_editor::XmlEditorPanel`] -- raw XML tree and attribute editor
//! * [`text_panel::TextPanel`] -- text-specific property editor
//! * [`rulers::HorizontalRuler`] / [`rulers::VerticalRuler`] -- tick-mark rulers
//! * [`ruler_config`] -- ruler configuration, tick-mark computation
//! * [`scrollbars::HorizontalScrollbar`] / [`scrollbars::VerticalScrollbar`] -- viewport scrollbars
//! * [`color_bar::ColorBar`] -- quick fill/stroke color indicator
//! * [`color_picker`] -- color picker modes, state, and hex conversion utilities
//! * [`main_layout::MainLayout`] -- grid-based main window layout
//! * [`panels`] -- panel data models (layers, objects, fill_stroke, align, transform)
//! * [`signals::EditorSignals`] -- version-based state change tracking
//! * [`doc_properties::DocPropertiesPanel`] -- document properties (page size, background, metadata, grid)
//! * [`preferences_dialog::PreferencesDialog`] -- application preferences dialog
//! * [`symbols_panel::SymbolsPanel`] -- symbol library browser and insertion
//! * [`find_replace::FindReplacePanel`] -- find & replace for SVG content
//! * [`undo_history::UndoHistoryPanel`] -- undo/redo history with jump-to
//! * [`swatches_panel::SwatchesPanel`] -- colour swatch palette grid
//! * [`font_collections::FontCollectionsPanel`] -- font browser with preview
//! * [`trace_dialog::TraceDialog`] -- bitmap tracing dialog
//! * [`filter_editor::FilterEditorPanel`] -- visual SVG filter chain editor
//! * [`lpe_panel::LpePanel`] -- live path effects manager
//! * [`export_panel::ExportPanel`] -- export to PNG/JPEG/SVG/PDF
//! * [`guides_dialog::GuidesDialog`] -- guide management dialog
//! * [`resources_panel::ResourcesPanel`] -- document resource listing and cleanup

// ---------------------------------------------------------------------------
// Dioxus component modules (existing)
// ---------------------------------------------------------------------------
pub mod align_panel;
pub mod canvas;
pub mod color_bar;
pub mod command_palette;
pub mod doc_properties;
pub mod export_panel;
pub mod filter_editor;
pub mod find_replace;
pub mod font_collections;
pub mod guides_dialog;
pub mod layers_panel;
pub mod lpe_panel;
pub mod main_layout;
pub mod menu_bar;
pub mod objects_panel;
pub mod preferences_dialog;
pub mod properties_panel;
pub mod resources_panel;
pub mod rulers;
pub mod scrollbars;
pub mod status_bar;
pub mod swatches_panel;
pub mod symbols_panel;
pub mod text_panel;
pub mod tool_controls;
pub mod toolbar;
pub mod trace_dialog;
pub mod transform_panel;
pub mod undo_history;
pub mod xml_editor;

// ---------------------------------------------------------------------------
// Data-model / logic modules (new)
// ---------------------------------------------------------------------------
pub mod canvas_state;
pub mod color_picker;
pub mod i18n;
pub mod menu_model;
pub mod panels;
pub mod ruler_config;
pub mod signals;
pub mod statusbar;
pub mod toolbar_model;

// ---------------------------------------------------------------------------
// Re-exports: Dioxus components
// ---------------------------------------------------------------------------
pub use align_panel::AlignPanel;
pub use canvas::{render_to_data_uri, CanvasView, SelectionHandle};
pub use color_bar::ColorBar;
pub use command_palette::{all_commands, CommandPalette, PaletteCommand};
pub use layers_panel::LayersPanel;
pub use main_layout::MainLayout;
pub use menu_bar::MenuBar;
pub use objects_panel::ObjectsPanel;
pub use properties_panel::PropertiesPanel;
pub use rulers::{HorizontalRuler, VerticalRuler};
pub use scrollbars::{HorizontalScrollbar, VerticalScrollbar};
pub use status_bar::StatusBar;
pub use text_panel::TextPanel;
pub use tool_controls::{
    GradientKind, NodeType, PenShape, SegmentType, ToolControlAction, ToolControls,
};
pub use toolbar::Toolbar;
pub use transform_panel::TransformPanel;
pub use xml_editor::XmlEditorPanel;

// New panel components
pub use doc_properties::DocPropertiesPanel;
pub use export_panel::ExportPanel;
pub use filter_editor::FilterEditorPanel;
pub use find_replace::FindReplacePanel;
pub use font_collections::FontCollectionsPanel;
pub use guides_dialog::GuidesDialog;
pub use lpe_panel::LpePanel;
pub use preferences_dialog::PreferencesDialog;
pub use resources_panel::ResourcesPanel;
pub use swatches_panel::SwatchesPanel;
pub use symbols_panel::SymbolsPanel;
pub use trace_dialog::TraceDialog;
pub use undo_history::UndoHistoryPanel;

// ---------------------------------------------------------------------------
// Re-exports: Data models
// ---------------------------------------------------------------------------
pub use canvas_state::CanvasState;
pub use color_picker::{color_to_hex, hex_to_color, ColorPickerMode, ColorPickerState};
pub use i18n::{build_chinese_simplified, build_japanese, I18n, Locale};
pub use menu_model::{default_menu_bar, MenuItem as MenuItemModel};
pub use panels::align::{AlignAction as AlignActionModel, DistributeAction, RelativeTo};
pub use panels::fill_stroke::{FillStrokeState, PaintType as PaintTypeModel};
pub use panels::layers::LayerEntry as LayerEntryModel;
pub use panels::objects::ObjectEntry as ObjectEntryModel;
pub use panels::transform::TransformState;
pub use ruler_config::{compute_tick_marks, Orientation, RulerConfig, RulerUnit, TickMark};
pub use signals::{EditorSignals, SignalChanges};
pub use statusbar::StatusBarInfo;
pub use toolbar_model::{
    default_toolbox, set_active_tool, ToolbarGroup as ToolbarGroupModel,
    ToolbarItem as ToolbarItemModel,
};

// New panel data models
pub use doc_properties::{DocumentMetadata, GridSettings, PageOrientation, PageSizePreset};
pub use export_panel::{ExportArea, ExportFormat, ExportSettings};
pub use filter_editor::{FilterPrimitive, FilterPrimitiveKind};
pub use find_replace::{FindQuery, FindResult, SearchType};
pub use font_collections::FontEntry;
pub use guides_dialog::GuideEntry;
pub use lpe_panel::{LpeEntry, LpeKind};
pub use preferences_dialog::{PreferencesSection, PreferencesState};
pub use resources_panel::{ResourceEntry, ResourceKind};
pub use swatches_panel::SwatchEntry;
pub use symbols_panel::SymbolEntry;
pub use trace_dialog::{TracingMode, TracingParams};
pub use undo_history::HistoryEntry;
