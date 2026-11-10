//! `sivgo` -- Sivgo SVG vector graphics editor (desktop application).
//!
//! Supports two modes:
//!
//! * **GUI mode** (default, no arguments): launches the Glory + Sapid desktop editor.
//! * **CLI mode** (`render` / `info` subcommands): batch operations.

#![allow(clippy::items_after_test_module)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use log::info;
use rfd::{MessageButtons, MessageDialog, MessageLevel};
use serde::{Deserialize, Serialize};

use sivgo_core::selection::element_bounding_box;
use sivgo_core::{
    AddNodeCommand, AlignRelativeTo as CoreAlignRelativeTo, Alignment as CoreAlignment,
    DisplayMode, DistributeAxis as CoreDistributeAxis, EditorEngine, ModifyAttributeCommand,
    ToolId,
};
use sivgo_desktop::clipboard::{
    copy_image as copy_image_to_stub_clipboard, copy_svg as copy_svg_to_stub_clipboard,
    copy_text as copy_text_to_stub_clipboard, paste_image as paste_image_from_stub_clipboard,
    paste_svg as paste_svg_from_stub_clipboard, paste_text as paste_text_from_stub_clipboard,
    ClipboardManager, ClipboardPayload, ClipboardSnapshot,
};
use sivgo_dom::{
    Align, BlendMode, Color, ImageAttrs, LengthUnit, LineCap, LineJoin, MeetOrSlice, NodeId, Paint,
    PresentationAttrs, PreserveAspectRatio, SvgElement, SvgLength, TextAttrs, ViewBox,
};
use sivgo_math::{Point, Rect};
use sivgo_render::{DocumentRenderer, TinySkiaBackend};
use sivgo_ui::doc_properties::DocPropertiesAction;
use sivgo_ui::preferences_dialog::PreferencesAction;
use sivgo_ui::{
    align_panel::{
        AlignAction as UiAlignAction, AlignOp as UiAlignOp, AlignRelativeTo as UiAlignRelativeTo,
        DistributeOp as UiDistributeOp,
    },
    objects_panel::{ObjectAction, ObjectDetails, ObjectEntry},
    pages_panel::{PageAction, PageEntry, PageTemplate},
    transform_panel::TransformAction,
};
use sivgo_ui::{
    layers_panel::LayerEntry,
    lpe_panel::{LpeAction as UiLpeAction, LpeParam as UiLpeParam},
    menu_model::{default_menu_bar_with_state, MenuItem as MenuNode},
    AlignPanel, ColorBar, DocPropertiesPanel, FilterEditorAction as UiFilterEditorAction,
    FilterEditorPanel, FilterParam as UiFilterParam, FilterPrimitive as UiFilterPrimitive,
    FilterPrimitiveKind as UiFilterPrimitiveKind, HorizontalRuler, HorizontalScrollbar,
    LayersPanel, LpeEntry as UiLpeEntry, LpeKind as UiLpeKind, LpePanel, ObjectsPanel,
    PageOrientation, PageSizePreset, PagesPanel, PreferencesDialog, PreferencesState,
    PropertiesPanel, ResourceEntry as UiResourceEntry, ResourceKind as UiResourceKind,
    ResourcesAction as UiResourcesAction, ResourcesPanel, StatusBar, TextAction as UiTextAction,
    TextPanel, ToolControlAction, ToolControls, Toolbar, TransformPanel, VerticalRuler,
    VerticalScrollbar,
};

use glory::render::{Event as UiEvent, Query, QueryValue};
use glory::style::{Edges, Length as UiLength, Overflow, Position, Style as UiStyle};
use glory::ui::events::{focus_within, resize};
use glory::ui::{col, each, image, row, text, view, widget, Widget};
use glory::{Cage, Scope};
use sivgo_ui::icons::EditorIcon;
use sivgo_ui::Callback;
mod agent_panel;
mod canvas_assets;
mod file_session;
mod menu;
mod native_host;
mod recovery;
mod state;
use menu::editor_menu;
use native_host::{DesktopContext, NativeInput};
use state::Signal;
include!("editor_logic.rs");
include!("editor_panels.rs");

#[derive(Clone, Copy)]
struct AppState {
    engine: Signal<EditorEngine>,
    agent: agent_panel::AgentState,
    files: file_session::FileSession,
    render_version: Signal<u64>,
    canvas_pointer_down: Signal<bool>,
    canvas_drag_preview: Signal<Option<CanvasDragPreview>>,
    canvas_path_preview: Signal<Option<CanvasPathPreview>>,
    canvas_select_preview: Signal<Option<CanvasSelectPreview>>,
    canvas_tool_feedback: Signal<Option<&'static str>>,
    canvas_interaction_source_hit: Signal<Option<NodeId>>,
    file_path: Signal<Option<PathBuf>>,
    active_tool: Signal<ToolId>,
    is_fullscreen: Signal<bool>,
    inspector_panel: Signal<InspectorPanel>,
    document_defaults: Signal<DocumentDefaults>,
    preferences_state: Signal<PreferencesState>,
    shortcuts: Signal<sivgo_core::ShortcutManager>,
    layout_settings: Signal<LayoutSettings>,
    inspector_resize_drag: Signal<Option<InspectorResizeDrag>>,
    window_size: Signal<(f64, f64)>,
    align_relative_to: Signal<String>,
    transform_lock_aspect: Signal<bool>,
    transform_percent_mode: Signal<bool>,
    object_editing_id: Signal<Option<NodeId>>,
    object_edit_value: Signal<String>,
    object_xml_id_editing_id: Signal<Option<NodeId>>,
    object_xml_id_edit_value: Signal<String>,
    object_dragging_id: Signal<Option<NodeId>>,
    layer_editing_id: Signal<Option<NodeId>>,
    layer_edit_value: Signal<String>,
    lpe_selected_id: Signal<Option<String>>,
    filter_selected_id: Signal<Option<String>>,
    resource_kind_filter: Signal<Option<String>>,
    resource_selected_id: Signal<Option<String>>,
    cursor_position: Cage<(f64, f64)>,
    desktop: Cage<DesktopContext>,
    canvas_node: Cage<Option<glory::render::NodeId>>,
    canvas_bounds: Cage<Option<glory::render::Rect>>,
    editing: Cage<bool>,
    menu_open: Cage<Option<usize>>,
}
impl AppState {
    fn new(scope: &Scope, desktop: DesktopContext) -> Self {
        #[cfg(not(test))]
        let loaded = initial_desktop_settings();
        #[cfg(test)]
        let loaded = DesktopSettings::default();
        let mut engine = EditorEngine::new(
            loaded.document_defaults.width_px,
            loaded.document_defaults.height_px,
        );
        apply_document_defaults_to_engine(&mut engine, &loaded.document_defaults);
        apply_preferences_to_engine(&mut engine, &loaded.preferences);
        engine.mark_saved();
        Self {
            engine: Signal::new(scope, engine),
            agent: agent_panel::AgentState::new(scope),
            files: file_session::FileSession::new(
                scope,
                (!cfg!(test)).then(desktop_config_dir),
                loaded.preferences.recent_files_count as usize,
            ),
            render_version: Signal::new(scope, 0),
            canvas_pointer_down: Signal::new(scope, false),
            canvas_drag_preview: Signal::new(scope, None),
            canvas_path_preview: Signal::new(scope, None),
            canvas_select_preview: Signal::new(scope, None),
            canvas_tool_feedback: Signal::new(scope, None),
            canvas_interaction_source_hit: Signal::new(scope, None),
            file_path: Signal::new(scope, None),
            active_tool: Signal::new(scope, ToolId::Select),
            is_fullscreen: Signal::new(scope, false),
            inspector_panel: Signal::new(scope, InspectorPanel::Style),
            document_defaults: Signal::new(scope, loaded.document_defaults),
            preferences_state: Signal::new(scope, loaded.preferences),
            shortcuts: Signal::new(scope, sivgo_core::ShortcutManager::new()),
            layout_settings: Signal::new(scope, loaded.layout),
            inspector_resize_drag: Signal::new(scope, None),
            window_size: Signal::new(scope, desktop.logical_size()),
            align_relative_to: Signal::new(scope, "Selection".to_owned()),
            transform_lock_aspect: Signal::new(scope, true),
            transform_percent_mode: Signal::new(scope, false),
            object_editing_id: Signal::new(scope, None),
            object_edit_value: Signal::new(scope, String::new()),
            object_xml_id_editing_id: Signal::new(scope, None),
            object_xml_id_edit_value: Signal::new(scope, String::new()),
            object_dragging_id: Signal::new(scope, None),
            layer_editing_id: Signal::new(scope, None),
            layer_edit_value: Signal::new(scope, String::new()),
            lpe_selected_id: Signal::new(scope, None),
            filter_selected_id: Signal::new(scope, None),
            resource_kind_filter: Signal::new(scope, None),
            resource_selected_id: Signal::new(scope, None),
            cursor_position: scope.cage((0.0, 0.0)),
            desktop: scope.cage(desktop),
            canvas_node: scope.cage(None),
            canvas_bounds: scope.cage(None),
            editing: scope.cage(false),
            menu_open: scope.cage(None),
        }
    }
    fn action(self, action: &str) {
        if self.files.action(self, action) {
            return;
        }
        handle_editor_action(
            action,
            self.engine,
            self.render_version,
            self.is_fullscreen,
            self.inspector_panel,
            self.document_defaults,
            self.preferences_state,
            self.desktop.get(),
        );
        self.active_tool.set(self.engine.peek().tools.active_id());
    }
}
#[derive(Clone, Copy, Debug)]
enum EditorPart {
    Toolbar,
    ToolControls,
    HorizontalRuler,
    VerticalRuler,
    HorizontalScrollbar,
    VerticalScrollbar,
    ColorBar,
    PropertiesPanel,
    LayersPanel,
    TextPanel,
    ObjectsPanel,
    PagesPanel,
    AlignPanel,
    TransformPanel,
    LpePanel,
    FilterEditorPanel,
    ResourcesPanel,
    DocPropertiesPanel,
    PreferencesDialog,
    StatusBar,
}

fn editor_part(state: AppState, part: EditorPart) -> impl Widget {
    widget(move |scope| {
        // Retain an active editor while the user types. Refresh snapshots when
        // focus leaves the control, so document updates do not discard its caret.
        let previous = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let keys = scope.bond(move || {
            if state.editing.get() && !previous.borrow().is_empty() {
                return vec![previous.borrow().clone()];
            }
            let key = format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                *state.render_version.read(),
                *state.active_tool.read(),
                *state.inspector_panel.read(),
                *state.object_editing_id.read(),
                *state.layer_editing_id.read(),
                *state.lpe_selected_id.read(),
                *state.filter_selected_id.read(),
                *state.resource_kind_filter.read(),
                *state.resource_selected_id.read(),
                *state.transform_lock_aspect.read(),
                *state.transform_percent_mode.read(),
                *state.align_relative_to.read(),
                *state.window_size.read(),
                state.preferences_state.read().section,
                *state.object_dragging_id.read(),
                state.canvas_bounds.get()
            );
            *previous.borrow_mut() = key.clone();
            vec![key]
        });
        each(keys)
            .key(|key| key.clone())
            .seed(move |_| widget(move |scope| build_editor_part(scope, state, part)))
            .show_in(scope)
    })
}

fn menu_availability(state: AppState) -> HashMap<String, bool> {
    let engine = state.engine.peek();
    let tree = default_menu_bar_with_state(
        engine.history.can_undo(),
        engine.history.can_redo(),
        !engine.selection.is_empty(),
    );
    let mut values = HashMap::new();
    fn collect(items: &[MenuNode], values: &mut HashMap<String, bool>) {
        for item in items {
            if let Some(action) = item.action.as_ref() {
                values.insert(action.clone(), item.enabled);
            }
            collect(&item.children, values);
        }
    }
    collect(&tree, &mut values);
    values
}

fn action_button(
    label: &'static str,
    icon: EditorIcon,
    action: &'static str,
    state: AppState,
) -> sapid::ActionIcon {
    sapid::action_icon(label)
        .theme(sapid::Theme::dark())
        .size(sapid::Size::ExtraSmall)
        .icon(icon.widget())
        .enabled(state.render_version.map(move |_| match action {
            "edit.undo" => state.engine.peek().history.can_undo(),
            "edit.redo" => state.engine.peek().history.can_redo(),
            _ => true,
        }))
        .on_press(move || state.action(action))
}

fn inspector(state: AppState) -> impl Widget {
    widget(move |scope| {
        let panels = [
            ("Style", InspectorPanel::Style),
            ("Text", InspectorPanel::Text),
            ("Objects", InspectorPanel::Objects),
            ("Pages", InspectorPanel::Pages),
            ("Align", InspectorPanel::Align),
            ("Transform", InspectorPanel::Transform),
            ("Effects", InspectorPanel::Lpe),
            ("Filters", InspectorPanel::Filters),
            ("Resources", InspectorPanel::Resources),
            ("Document", InspectorPanel::Document),
            ("Preferences", InspectorPanel::Preferences),
        ];
        glory::ui::when(
            state
                .inspector_panel
                .map(|panel| *panel != InspectorPanel::Agent),
            move || {
                row()
                    .class("inspector-tabs")
                    .style(UiStyle {
                        ..row_style()
                            .position(Position::Relative)
                            .gap(4.0)
                            .flex_wrap(glory::style::FlexWrap::Wrap)
                    })
                    .fill(widget(move |scope| {
                        for (label, panel) in panels {
                            sapid::button()
                                .theme(sapid::Theme::dark())
                                .size(sapid::Size::ExtraSmall)
                                .label(label)
                                .name(format!("Show {label} panel"))
                                .variant(sapid::ButtonVariant::Secondary)
                                .style(state.inspector_panel.map(move |active| {
                                    let theme = sapid::Theme::dark();
                                    UiStyle {
                                        background: Some(if *active == panel {
                                            theme.accent_soft
                                        } else {
                                            theme.surface
                                        }),
                                        color: Some(if *active == panel {
                                            theme.accent_text
                                        } else {
                                            theme.foreground
                                        }),
                                        border_color: Some(if *active == panel {
                                            theme.accent
                                        } else {
                                            theme.border
                                        }),
                                        ..UiStyle::default()
                                    }
                                }))
                                .on_press(move || {
                                    state.editing.revise(|v| *v = false);
                                    state.inspector_panel.set(panel);
                                })
                                .show_in(scope)?;
                        }
                        Ok(())
                    }))
            },
        )
        .show_in(scope)?;
        // Each panel owns its scroll viewport. Switching panels or removing
        // the last selection must not retain an out-of-range scroll position.
        let panel = scope.bond(move || {
            let _ = *state.render_version.read();
            let panel = *state.inspector_panel.read();
            vec![(
                panel as u8,
                panel != InspectorPanel::Agent && state.engine.peek().selection.is_empty(),
            )]
        });
        each(panel)
            .key(|key| *key)
            .seed(move |(panel_id, _)| {
                let agent_open = panel_id == InspectorPanel::Agent as u8;
                col()
                    .class("inspector-content")
                    .style(
                        flexible_style()
                            .flex_direction(glory::style::FlexDirection::Column)
                            .min_height(UiLength::Px(0.0))
                            .overflow_y(if agent_open {
                                Overflow::Hidden
                            } else {
                                Overflow::Auto
                            })
                            .overflow_x(Overflow::Hidden),
                    )
                    .fill(widget(move |scope| {
                        let part = match *state.inspector_panel.peek() {
                            InspectorPanel::Style => EditorPart::PropertiesPanel,
                            InspectorPanel::Text => EditorPart::TextPanel,
                            InspectorPanel::Objects => EditorPart::ObjectsPanel,
                            InspectorPanel::Pages => EditorPart::PagesPanel,
                            InspectorPanel::Align => EditorPart::AlignPanel,
                            InspectorPanel::Transform => EditorPart::TransformPanel,
                            InspectorPanel::Lpe => EditorPart::LpePanel,
                            InspectorPanel::Filters => EditorPart::FilterEditorPanel,
                            InspectorPanel::Resources => EditorPart::ResourcesPanel,
                            InspectorPanel::Document => EditorPart::DocPropertiesPanel,
                            InspectorPanel::Preferences => EditorPart::PreferencesDialog,
                            InspectorPanel::Agent => {
                                return agent_panel::panel(state).show_in(scope)
                            }
                        };
                        editor_part(state, part).show_in(scope)?;
                        if *state.inspector_panel.peek() == InspectorPanel::Style {
                            editor_part(state, EditorPart::LayersPanel).show_in(scope)?;
                        }
                        Ok(())
                    }))
            })
            .show_in(scope)
    })
}

fn refresh_canvas_bounds(scope: &Scope, state: AppState) {
    if let Some(node) = state.canvas_node.get() {
        let weak = scope.downgrade();
        if let Err(error) = scope.request(Query::Bounds(node), move |result| {
            if let Ok(QueryValue::Bounds(bounds)) = result {
                state.canvas_bounds.revise(|value| *value = Some(bounds));
                let mut engine = state.engine.write();
                engine.viewport.width = bounds.width;
                engine.viewport.height = bounds.height;
            }
            let _ = weak;
        }) {
            scope.report_error(error);
        }
    }
}

fn canvas(state: AppState) -> impl Widget {
    widget(move |scope| {
        let weak = scope.downgrade();
        let bitmap = state.render_version.map(move |_| {
            let engine = state.engine.peek();
            let (w, h) = document_pixel_size(&engine);
            sivgo_ui::render_to_data_uri(&engine, w.min(4096), h.min(4096))
        });
        let geometry = state.render_version.map(move |_| {
            let engine = state.engine.peek();
            let (w, h) = document_pixel_size(&engine);
            UiStyle::new()
                .position(Position::Absolute)
                .left(UiLength::Px(engine.viewport.pan.x))
                .top(UiLength::Px(engine.viewport.pan.y))
                .width(UiLength::Px(w as f64 * engine.viewport.zoom))
                .height(UiLength::Px(h as f64 * engine.viewport.zoom))
        });
        view()
            .name("SVG canvas")
            .class("glory-canvas")
            .node_ref(state.canvas_node)
            .style(
                UiStyle {
                    background: Some(glory::style::Color(48, 49, 54, 255)),
                    ..flexible_style()
                }
                .position(Position::Relative)
                .min_width(UiLength::Px(1.0))
                .min_height(UiLength::Px(120.0))
                .overflow_x(Overflow::Hidden)
                .overflow_y(Overflow::Hidden),
            )
            .on(resize, move |_| {
                if let Some(scope) = weak.upgrade() {
                    refresh_canvas_bounds(&scope, state);
                }
            })
            .fill((
                image().source(bitmap).alt("SVG drawing").style(geometry),
                canvas_grid(state),
                canvas_overlays(state),
                canvas_text_editor(state),
            ))
            .show_in(scope)
    })
}

fn editor_app(state: AppState) -> impl Widget {
    widget(move |scope| {
        let height = state.window_size.map(|size| {
            column_style()
                .height(UiLength::Px(size.1))
                .width(UiLength::Percent(100.0))
        });
        col()
            .class("sivgo-glory")
            .style(height)
            .on(focus_within, move |event| {
                if matches!(event, UiEvent::FocusWithin { focused: false, .. }) {
                    state.editing.revise(|v| *v = false);
                }
            })
            .fill((
                editor_menu(state),
                row()
                    .style(
                        row_style()
                            .gap(6.0)
                            .padding(Edges::all(6.0))
                            .overflow_x(Overflow::Auto),
                    )
                    .fill((
                        text().class("brand-title").content("Sivgo"),
                        action_button("New drawing", EditorIcon::New, "file.new", state),
                        action_button("Open drawing", EditorIcon::Open, "file.open", state),
                        action_button("Save drawing", EditorIcon::Save, "file.save", state),
                        action_button("Export PNG", EditorIcon::Export, "file.export_png", state),
                        action_button("Undo", EditorIcon::Undo, "edit.undo", state),
                        action_button("Redo", EditorIcon::Redo, "edit.redo", state),
                        action_button("Zoom out", EditorIcon::ZoomOut, "view.zoom_out", state),
                        action_button("Zoom in", EditorIcon::ZoomIn, "view.zoom_in", state),
                        action_button("Fit drawing", EditorIcon::Fit, "view.zoom_fit", state),
                    ))
                    .fill(
                        glory::ui::input()
                            .name("Zoom percent")
                            .style(UiStyle {
                                color: Some(sapid::Theme::dark().foreground),
                                background: Some(sapid::Theme::dark().surface),
                                ..UiStyle::new()
                                    .width(UiLength::Px(72.0))
                                    .height(UiLength::Px(26.0))
                                    .padding(Edges::all(4.0))
                            })
                            .value(state.render_version.map(move |_| {
                                format!("{:.0}%", state.engine.peek().viewport.zoom * 100.0)
                            }))
                            .on(glory::ui::events::commit, move |event| {
                                if let UiEvent::Commit { value, .. } = event {
                                    if let Some(zoom) = parse_zoom_percent(&value) {
                                        let mut engine = state.engine.write();
                                        let center = Point::new(
                                            engine.viewport.width / 2.0,
                                            engine.viewport.height / 2.0,
                                        );
                                        set_viewport_zoom_at_screen_point(
                                            &mut engine,
                                            zoom,
                                            center,
                                        );
                                        drop(engine);
                                        bump_render(state.render_version);
                                    }
                                }
                            }),
                    )
                    .fill(
                        text()
                            .class("current-document")
                            .content(scope.bond(move || {
                                let _ = *state.render_version.read();
                                format!(
                                    "{}{}",
                                    state.files.drawing_name(state),
                                    if state.engine.peek().is_dirty() {
                                        " • Unsaved"
                                    } else {
                                        ""
                                    }
                                )
                            })),
                    )
                    .fill(glory::ui::when(state.files.recovery_failed(), || {
                        text()
                            .class("recovery-warning")
                            .content("Recovery unavailable")
                    }))
                    .fill(
                        sapid::button()
                            .theme(sapid::Theme::dark())
                            .size(sapid::Size::ExtraSmall)
                            .label("AI Agent")
                            .variant(sapid::ButtonVariant::Secondary)
                            .left_section(EditorIcon::Agent.widget())
                            .style(state.inspector_panel.map(|panel| {
                                UiStyle {
                                    background: (*panel == InspectorPanel::Agent)
                                        .then(|| sapid::Theme::dark().accent_soft),
                                    ..UiStyle::default()
                                }
                            }))
                            .on_press(move || state.agent.toggle(state)),
                    ),
                row()
                    .style(flexible_style().min_height(UiLength::Px(0.0)))
                    .fill((
                        col()
                            .style(UiStyle {
                                shrink: Some(0.0),
                                ..column_style()
                                    .width(UiLength::Px(44.0))
                                    .overflow_x(Overflow::Hidden)
                                    .overflow_y(Overflow::Auto)
                            })
                            .fill(editor_part(state, EditorPart::Toolbar)),
                        col()
                            .style(
                                flexible_style()
                                    .flex_direction(glory::style::FlexDirection::Column)
                                    .min_width(UiLength::Px(120.0)),
                            )
                            .fill((
                                view()
                                    .style(UiStyle {
                                        shrink: Some(0.0),
                                        ..UiStyle::new()
                                            .height(UiLength::Px(40.0))
                                            .overflow_x(Overflow::Auto)
                                            .overflow_y(Overflow::Hidden)
                                    })
                                    .fill(editor_part(state, EditorPart::ToolControls)),
                                row().fill((
                                    view().style(UiStyle::new().width(UiLength::Px(20.0))),
                                    editor_part(state, EditorPart::HorizontalRuler),
                                )),
                                row()
                                    .style(flexible_style().min_height(UiLength::Px(0.0)))
                                    .fill((
                                        editor_part(state, EditorPart::VerticalRuler),
                                        canvas(state),
                                        editor_part(state, EditorPart::VerticalScrollbar),
                                    )),
                                row().fill((
                                    view().style(UiStyle::new().width(UiLength::Px(20.0))),
                                    editor_part(state, EditorPart::HorizontalScrollbar),
                                )),
                                editor_part(state, EditorPart::ColorBar),
                            )),
                        view().name("Resize inspector").style(UiStyle {
                            width: Some(UiLength::Px(8.0)),
                            shrink: Some(0.0),
                            background: Some(glory::style::Color(65, 65, 72, 255)),
                            ..UiStyle::default()
                        }),
                        col()
                            .style(scope.bond(move || {
                                let width = effective_inspector_width(
                                    state.window_size.read().0,
                                    state.layout_settings.read().inspector_width_px,
                                );
                                UiStyle {
                                    shrink: Some(0.0),
                                    ..column_style()
                                        .width(UiLength::Px(width))
                                        .padding(Edges::all(8.0))
                                        .gap(8.0)
                                        .min_height(UiLength::Px(0.0))
                                        .overflow_x(Overflow::Hidden)
                                        .overflow_y(Overflow::Hidden)
                                }
                            }))
                            .fill(inspector(state)),
                    )),
                editor_part(state, EditorPart::StatusBar),
            ))
            .show_in(scope)
    })
}

fn launch_gui() {
    if let Err(error) = launch_glory() {
        report_error("Could not start Sivgo", &error.to_string());
    }
}

fn launch_glory() -> Result<(), Box<dyn std::error::Error>> {
    let plugin_manager = sivgo_desktop::plugins::init_plugin_manager();
    for (id, name) in sivgo_desktop::plugins::available_plugins(&plugin_manager) {
        log::info!("Available plugin: {id} ({name})");
    }
    let desktop = DesktopContext::new();
    let app = glory::apace::shell::WindowApp::with_config(glory::apace::Config {
        provider: Some(std::sync::Arc::new(
            canvas_assets::CanvasImageProvider::new()?,
        )),
        ..Default::default()
    });
    let state = AppState::new(app.app().scope(), desktop.clone());
    app.renderer()
        .set_css(&format!("{CSS}\n{HOST_CSS}\n{AGENT_CSS}"));
    app.mount(editor_app(state))?;
    refresh_canvas_bounds(app.app().scope(), state);
    let mut offer_recovery = true;
    native_host::run(
        app,
        desktop,
        move |input| handle_native_input(state, input),
        move || state.files.close(state),
        move || {
            if std::mem::take(&mut offer_recovery) {
                state.files.offer_recovery(state, false);
            }
            state.files.tick(state, std::time::Instant::now())
        },
    )
}

const HOST_CSS: &str = r#"
.inspector-tabs { z-index:2; position:relative; flex-shrink:0; background:#232328; } html,body { margin:0; padding:0; } .sivgo-glory { font-family:sans-serif; font-size:13px; background:var(--bg-1); color:var(--ink-0); } button,input,select { font-size:12px; } .glory-canvas { user-select:none; }
.glory-menu-bar { flex-shrink: 0; background: #24252a; border-bottom: 1px solid #34363e; }
.glory-menu-bar .sapid-menu-trigger { border-radius: 4px; padding: 4px 10px; }
.glory-menu-bar .sapid-menu-trigger:hover,
.glory-menu-bar .sapid-menu-trigger[aria-expanded="true"] { background: #383b44 !important; color: #f4f6fa !important; }
.sapid-menu { max-height: calc(100vh - 70px); overflow-y: auto; overflow-x: hidden; box-shadow: 0 8px 24px #00000060; }
.sapid-menu-item { background: transparent !important; border: 0 !important; border-radius: 4px; flex-shrink: 0; text-align: left; }
.sapid-menu-item-label { text-align: left; }
.sapid-menu-item:not(:disabled):hover,
.sapid-menu-item:not(:disabled):focus { background: #354966 !important; color: #ffffff !important; }
.sapid-menu-item:disabled { color: #777d89 !important; }
.sapid-menu-separator { flex-shrink: 0; }
.editor-menu-shortcut { margin-left: 20px; text-align: right; }
"#;
const AGENT_CSS: &str = "
.agent-title { font-size: 15px; font-weight: 600; color: #f1f1f4; }
.agent-muted { font-size: 12px; line-height: 1.45; color: #b7bcc7; }
.agent-card { background: #2c2d34; border: 1px solid #424550; border-radius: 6px; }
.agent-user { background: #29384b; border-color: #40546e; }
.agent-section { font-size: 12px; font-weight: 600; color: #bdcfff; }
.agent-message { line-height: 1.45; white-space: pre-wrap; overflow-wrap: anywhere; }
.recovery-warning { color: #ffd09a; }
";

fn main() {
    env_logger::init();
    if std::env::args().len() > 1 {
        run_cli();
    } else {
        launch_gui();
    }
}

fn canvas_overlays(state: AppState) -> impl Widget {
    widget(move |scope| {
        let drawing = scope.bond(move || {
            use glory::render::{
                GraphicPoint as P, GraphicProps, GraphicStroke, PathCommand as C, StrokeCap,
            };
            let _ = *state.render_version.read();
            let bounds = state.canvas_bounds.get();
            let engine = state.engine.peek();
            let width = bounds.map(|b| b.width).unwrap_or(800.0).max(1.0);
            let height = bounds.map(|b| b.height).unwrap_or(600.0).max(1.0);
            let zoom = engine.viewport.zoom;
            let pan = engine.viewport.pan;
            let mut commands = Vec::new();
            let mut rectangle = |left: f64, top: f64, w: f64, h: f64| {
                commands.extend([
                    C::Move(P::new(left, top)),
                    C::Line(P::new(left + w, top)),
                    C::Line(P::new(left + w, top + h)),
                    C::Line(P::new(left, top + h)),
                    C::Close,
                ]);
            };
            for page in page_entries(&engine).into_iter().take(20) {
                rectangle(
                    page.x * zoom + pan.x,
                    page.y * zoom + pan.y,
                    page.width * zoom,
                    page.height * zoom,
                );
            }
            if let Some(bounds) = engine.selection.bounding_box(&engine.document) {
                let delta = state
                    .canvas_select_preview
                    .read()
                    .map(|p| p.screen_delta())
                    .unwrap_or_default();
                rectangle(
                    bounds.x0 * zoom + pan.x + delta.0,
                    bounds.y0 * zoom + pan.y + delta.1,
                    bounds.width() * zoom,
                    bounds.height() * zoom,
                );
            }
            if let Some(preview) = *state.canvas_select_preview.read() {
                if preview.selection_bounds.is_none() && preview.is_active() {
                    rectangle(
                        preview.start_x.min(preview.current_x),
                        preview.start_y.min(preview.current_y),
                        (preview.current_x - preview.start_x).abs(),
                        (preview.current_y - preview.start_y).abs(),
                    );
                }
            }
            if let Some(preview) = *state.canvas_drag_preview.read() {
                let (x, y, w, h) = canvas_drag_preview_bounds(preview);
                if preview.tool == ToolId::Ellipse {
                    let (cx, cy, rx, ry) = canvas_ellipse_preview_geometry(preview);
                    let k = 0.5522847498307936;
                    commands.extend([
                        C::Move(P::new(cx + rx, cy)),
                        C::Cubic {
                            first: P::new(cx + rx, cy + k * ry),
                            second: P::new(cx + k * rx, cy + ry),
                            end: P::new(cx, cy + ry),
                        },
                        C::Cubic {
                            first: P::new(cx - k * rx, cy + ry),
                            second: P::new(cx - rx, cy + k * ry),
                            end: P::new(cx - rx, cy),
                        },
                        C::Cubic {
                            first: P::new(cx - rx, cy - k * ry),
                            second: P::new(cx - k * rx, cy - ry),
                            end: P::new(cx, cy - ry),
                        },
                        C::Cubic {
                            first: P::new(cx + k * rx, cy - ry),
                            second: P::new(cx + rx, cy - k * ry),
                            end: P::new(cx + rx, cy),
                        },
                        C::Close,
                    ]);
                } else {
                    rectangle(x, y, w, h);
                }
            }
            if let Some(path) = state.canvas_path_preview.read().as_ref() {
                let remaining = (230_usize.saturating_sub(commands.len())).max(1);
                let step = path.points.len().div_ceil(remaining).max(1);
                for (i, (x, y)) in path.points.iter().step_by(step).enumerate() {
                    commands.push(if i == 0 {
                        C::Move(P::new(*x, *y))
                    } else {
                        C::Line(P::new(*x, *y))
                    });
                }
            }
            for guide in engine
                .guide_lines()
                .into_iter()
                .take((256_usize.saturating_sub(commands.len())) / 2)
            {
                let angle = (-guide.normal.x).atan2(guide.normal.y);
                let center = P::new(guide.origin.x * zoom + pan.x, guide.origin.y * zoom + pan.y);
                let length = width.max(height) * 2.0;
                commands.extend([
                    C::Move(P::new(
                        center.x - angle.cos() * length,
                        center.y - angle.sin() * length,
                    )),
                    C::Line(P::new(
                        center.x + angle.cos() * length,
                        center.y + angle.sin() * length,
                    )),
                ]);
            }
            GraphicProps {
                width,
                height,
                commands,
                fill: None,
                stroke: Some(GraphicStroke {
                    color: glory::style::Color(65, 145, 240, 255),
                    width: 1.25,
                    cap: StrokeCap::Round,
                }),
                ..GraphicProps::default()
            }
        });
        glory::ui::graphic()
            .drawing(drawing)
            .style(
                UiStyle::new()
                    .position(Position::Absolute)
                    .left(UiLength::Px(0.0))
                    .top(UiLength::Px(0.0))
                    .width(UiLength::Percent(100.0))
                    .height(UiLength::Percent(100.0))
                    .pointer_events(glory::style::PointerEvents::None),
            )
            .show_in(scope)?;
        text()
            .content(
                state
                    .canvas_tool_feedback
                    .map(|feedback| feedback.unwrap_or("").to_owned()),
            )
            .style(
                UiStyle::new()
                    .position(Position::Absolute)
                    .left(UiLength::Px(12.0))
                    .bottom(UiLength::Px(12.0)),
            )
            .show_in(scope)
    })
}

fn canvas_grid(state: AppState) -> impl Widget {
    widget(move |scope| {
        use glory::render::{
            GraphicPoint as P, GraphicProps, GraphicStroke, PathCommand as C, StrokeCap,
        };
        let drawing = scope.bond(move || {
            let _ = *state.render_version.read();
            let bounds = state.canvas_bounds.get();
            let width = bounds.map(|b| b.width).unwrap_or(800.0).max(1.0);
            let height = bounds.map(|b| b.height).unwrap_or(600.0).max(1.0);
            let engine = state.engine.peek();
            let mut commands = Vec::new();
            if grid_visible(&engine) {
                let mut step = (engine.snapping.prefs.grid_spacing * engine.viewport.zoom).max(1.0);
                while (width + height) / step > 120.0 {
                    step *= 2.0;
                }
                let mut x = engine.viewport.pan.x.rem_euclid(step);
                while x <= width {
                    commands.extend([C::Move(P::new(x, 0.0)), C::Line(P::new(x, height))]);
                    x += step;
                }
                let mut y = engine.viewport.pan.y.rem_euclid(step);
                while y <= height {
                    commands.extend([C::Move(P::new(0.0, y)), C::Line(P::new(width, y))]);
                    y += step;
                }
            }
            GraphicProps {
                width,
                height,
                commands,
                stroke: Some(GraphicStroke {
                    color: glory::style::Color(120, 130, 145, 75),
                    width: 0.6,
                    cap: StrokeCap::Butt,
                }),
                ..Default::default()
            }
        });
        glory::ui::graphic()
            .drawing(drawing)
            .style(
                UiStyle::new()
                    .position(Position::Absolute)
                    .left(UiLength::Px(0.0))
                    .top(UiLength::Px(0.0))
                    .width(UiLength::Percent(100.0))
                    .height(UiLength::Percent(100.0))
                    .pointer_events(glory::style::PointerEvents::None),
            )
            .show_in(scope)
    })
}

fn canvas_text_editor(state: AppState) -> impl Widget {
    widget(move |scope| {
        let selected = scope.bond(move || {
            let _ = *state.render_version.read();
            let _ = *state.active_tool.read();
            let engine = state.engine.peek();
            if engine.tools.active_id() == ToolId::Text
                && engine.selection.len() == 1
                && engine.selected_text_state().is_some()
            {
                engine.selection.primary().into_iter().collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        });
        each(selected)
            .key(|id| *id)
            .seed(move |_| {
                glory::ui::input()
                    .name("Edit selected canvas text")
                    .multiline(true)
                    .value(state.render_version.map(move |_| {
                        state
                            .engine
                            .peek()
                            .selected_text_state()
                            .map(|text| text.content)
                            .unwrap_or_default()
                    }))
                    .style(state.render_version.map(move |_| {
                        let engine = state.engine.peek();
                        let bounds = engine
                            .selection
                            .bounding_box(&engine.document)
                            .unwrap_or(Rect::new(0.0, 0.0, 120.0, 32.0));
                        let zoom = engine.viewport.zoom;
                        let font_size = engine
                            .selected_text_state()
                            .map(|text| text.font_size)
                            .unwrap_or(16.0);
                        UiStyle {
                            color: Some(glory::style::Color(17, 24, 39, 255)),
                            background: Some(glory::style::Color(255, 255, 255, 245)),
                            font_size: Some((font_size * zoom).clamp(8.0, 96.0)),
                            ..UiStyle::new().padding(Edges::all(2.0))
                        }
                        .position(Position::Absolute)
                        .left(UiLength::Px(bounds.x0 * zoom + engine.viewport.pan.x))
                        .top(UiLength::Px(bounds.y0 * zoom + engine.viewport.pan.y))
                        .width(UiLength::Px((bounds.width() * zoom).max(120.0)))
                        .height(UiLength::Px((bounds.height() * zoom).max(48.0)))
                    }))
                    .on(glory::ui::events::value_changed, move |event| {
                        if let UiEvent::ValueChanged { value, .. } = event {
                            if state.engine.write().set_selected_text_content(value) {
                                bump_render(state.render_version);
                            }
                        }
                    })
            })
            .show_in(scope)
    })
}

fn native_modifiers(value: keyboard_types::Modifiers) -> sivgo_core::Modifiers {
    sivgo_core::Modifiers {
        ctrl: value
            .intersects(keyboard_types::Modifiers::CONTROL | keyboard_types::Modifiers::META),
        shift: value.contains(keyboard_types::Modifiers::SHIFT),
        alt: value.contains(keyboard_types::Modifiers::ALT),
    }
}

fn menu_shortcut_action(
    state: AppState,
    key: &keyboard_types::Key,
    modifiers: keyboard_types::Modifiers,
) -> Option<String> {
    let text = key.to_string();
    let code = code_for(&text.to_ascii_lowercase())?;
    let relevant = keyboard_types::Modifiers::CONTROL
        | keyboard_types::Modifiers::SHIFT
        | keyboard_types::Modifiers::ALT
        | keyboard_types::Modifiers::META;
    let mut actual = modifiers & relevant;
    if actual.contains(keyboard_types::Modifiers::META) {
        actual.remove(keyboard_types::Modifiers::META);
        actual.insert(keyboard_types::Modifiers::CONTROL);
    }
    fn find(
        items: &[MenuNode],
        code: Code,
        actual: keyboard_types::Modifiers,
        text: &str,
    ) -> Option<String> {
        for item in items {
            if !item.enabled {
                continue;
            }
            if let Some(action) = find(&item.children, code, actual, text) {
                return Some(action);
            }
            if let Some(hint) = item.shortcut.as_ref() {
                if let Some((expected, key)) = parse_accelerator(hint) {
                    let mut actual = actual;
                    if text.len() == 1
                        && "!@#$%^&*()_+{}|:\"<>?~".contains(text)
                        && !hint.to_ascii_lowercase().contains("shift")
                    {
                        actual.remove(keyboard_types::Modifiers::SHIFT);
                    }
                    if expected == actual && key == code {
                        return item.action.clone();
                    }
                }
            }
        }
        None
    }
    let engine = state.engine.peek();
    find(
        &default_menu_bar_with_state(
            engine.history.can_undo(),
            engine.history.can_redo(),
            !engine.selection.is_empty(),
        ),
        code,
        actual,
        &text,
    )
}

fn cancel_canvas_input(state: AppState) {
    state.canvas_pointer_down.set(false);
    state.canvas_drag_preview.set(None);
    state.canvas_path_preview.set(None);
    state.canvas_select_preview.set(None);
    state.canvas_tool_feedback.set(None);
    state.canvas_interaction_source_hit.set(None);
}

fn handle_native_input(state: AppState, input: NativeInput) -> bool {
    if matches!(input, NativeInput::BackgroundReady) {
        return state.agent.poll(state);
    }
    if let NativeInput::Resize { width, height } = input {
        state.window_size.set((width, height));
        return false;
    }
    if matches!(input, NativeInput::PointerCancel | NativeInput::FocusLost) {
        state.engine.write().cancel_tool_interaction();
        cancel_canvas_input(state);
        state.editing.revise(|v| *v = false);
        state.inspector_resize_drag.set(None);
        bump_render(state.render_version);
        return false;
    }
    if let NativeInput::KeyDown {
        key,
        modifiers,
        editable,
    } = input
    {
        state.editing.revise(|value| *value = editable);
        if key == keyboard_types::Key::Named(keyboard_types::NamedKey::Escape)
            && state.menu_open.get().is_some()
        {
            state.menu_open.revise(|open| *open = None);
            return true;
        }
        let menu_action = menu_shortcut_action(state, &key, modifiers);
        if editable {
            if let Some(action) = menu_action.filter(|action| action.starts_with("file.")) {
                state.action(&action);
                return true;
            }
            return false;
        }
        let Some(key) = to_editor_key(&key) else {
            return false;
        };
        if key == sivgo_core::Key::Escape {
            state.engine.write().cancel_tool_interaction();
            cancel_canvas_input(state);
        }
        let modifiers = native_modifiers(modifiers);
        let action = state.shortcuts.read().lookup(key, modifiers).cloned();
        if let Some(action) = action {
            if let sivgo_core::Action::SwitchTool(tool) = action {
                state.engine.write().tools.switch_to(tool);
                state.active_tool.set(state.engine.peek().tools.active_id());
                if let Some(panel) = inspector_panel_for_tool(tool) {
                    state.inspector_panel.set(panel);
                }
                cancel_canvas_input(state);
                bump_render(state.render_version);
                return true;
            }
            if let Some(action) = menu_action_for_shortcut(&action) {
                state.action(action);
                return true;
            }
        }
        if let Some(action) = menu_action {
            state.action(&action);
            return true;
        }
        if key == sivgo_core::Key::Escape {
            state.engine.write().cancel_tool_interaction();
            cancel_canvas_input(state);
        } else {
            state
                .engine
                .write()
                .on_key_down(sivgo_core::KeyEvent { key, modifiers });
        }
        bump_render(state.render_version);
        return true;
    }
    if let NativeInput::PointerDown {
        x,
        y,
        button: sivgo_core::MouseButton::Left,
        ..
    } = input
    {
        let width = effective_inspector_width(
            state.window_size.peek().0,
            state.layout_settings.peek().inspector_width_px,
        );
        let edge = state.window_size.peek().0 - width;
        if x >= edge - 8.0 && x <= edge && state.canvas_bounds.get().is_some_and(|b| y >= b.y) {
            state.inspector_resize_drag.set(Some(InspectorResizeDrag {
                start_client_x: x,
                start_width_px: width,
            }));
            return true;
        }
    }
    let resizing = *state.inspector_resize_drag.peek();
    if let Some(drag) = resizing {
        match input {
            NativeInput::PointerMove { x, .. } => {
                state.layout_settings.write().inspector_width_px =
                    effective_inspector_width(state.window_size.peek().0, drag.width_at(x));
                return true;
            }
            NativeInput::PointerUp { x, .. } => {
                state.layout_settings.write().inspector_width_px = drag.width_at(x);
                state.inspector_resize_drag.set(None);
                #[cfg(not(test))]
                persist_desktop_settings_with_layout(
                    &state.preferences_state.read(),
                    &state.document_defaults.read(),
                    &state.layout_settings.read(),
                );
                return true;
            }
            _ => {}
        }
    }
    let Some(bounds) = state.canvas_bounds.get() else {
        return false;
    };
    let (x, y, button, modifiers) = match &input {
        NativeInput::PointerDown {
            x,
            y,
            button,
            modifiers,
        }
        | NativeInput::PointerMove {
            x,
            y,
            button,
            modifiers,
        }
        | NativeInput::PointerUp {
            x,
            y,
            button,
            modifiers,
        }
        | NativeInput::DoubleClick {
            x,
            y,
            button,
            modifiers,
        } => (*x, *y, *button, *modifiers),
        NativeInput::Wheel {
            x, y, modifiers, ..
        } => (*x, *y, sivgo_core::MouseButton::Left, *modifiers),
        _ => return false,
    };
    let inside = x >= bounds.x
        && y >= bounds.y
        && x < bounds.x + bounds.width
        && y < bounds.y + bounds.height;
    if matches!(input, NativeInput::PointerDown { .. }) {
        state.editing.revise(|v| *v = false);
    }
    if !inside && !*state.canvas_pointer_down.peek() {
        return false;
    }
    let x = x - bounds.x;
    let y = y - bounds.y;
    let modifiers = native_modifiers(modifiers);
    let mut event = engine_mouse_event_from_viewport_coords(
        x,
        y,
        modifiers.ctrl,
        modifiers.shift,
        modifiers.alt,
    );
    event.button = button;
    let tool = state.engine.peek().tools.active_id();
    if let NativeInput::Wheel { dx, dy, .. } = input {
        let mut engine = state.engine.write();
        if modifiers.ctrl {
            engine.on_scroll(
                wheel_delta_to_zoom_steps(WheelDelta::pixels(0.0, dy, 0.0)),
                x,
                y,
            );
        } else {
            engine.viewport.pan.x -= if modifiers.shift { dy } else { dx };
            if !modifiers.shift {
                engine.viewport.pan.y -= dy;
            }
        }
        drop(engine);
        bump_render(state.render_version);
        return true;
    }
    match input {
        NativeInput::DoubleClick { .. } => {
            if let Some(tool) = enter_canvas_edit_mode_at(&mut state.engine.write(), x, y) {
                state.active_tool.set(tool);
                if let Some(panel) = inspector_panel_for_tool(tool) {
                    state.inspector_panel.set(panel);
                }
                cancel_canvas_input(state);
                bump_render(state.render_version);
            }
        }
        NativeInput::PointerDown { .. } => {
            state.menu_open.revise(|open| *open = None);
            state.canvas_pointer_down.set(true);
            let engine = state.engine.peek();
            let hit = canvas_hit_id_at(&engine, x, y);
            let path = canvas_path_hit_id_at(&engine, x, y);
            state.canvas_interaction_source_hit.set(hit);
            state
                .canvas_tool_feedback
                .set(canvas_tool_feedback_on_mouse_down(
                    tool,
                    engine.selection.primary(),
                    selected_path_has_lpe_target(&engine),
                    hit,
                    path,
                    document_has_drawable_target(&engine),
                    document_has_path_target(&engine),
                ));
            if should_start_canvas_drag_preview(
                tool,
                &page_entries(&engine),
                x,
                y,
                engine.viewport.pan.x,
                engine.viewport.pan.y,
                engine.viewport.zoom,
            ) {
                state.canvas_drag_preview.set(Some(CanvasDragPreview::new(
                    tool,
                    x,
                    y,
                    modifiers.ctrl,
                    modifiers.shift,
                )));
            }
            if tool == ToolId::Select {
                state
                    .canvas_select_preview
                    .set(Some(CanvasSelectPreview::new(
                        x,
                        y,
                        hit.and_then(|_| engine.selection.bounding_box(&engine.document)),
                    )));
            }
            if tool_uses_canvas_path_preview(tool) {
                let previous = state.canvas_path_preview.peek().clone();
                let closes = tool == ToolId::Pen
                    && previous.as_ref().is_some_and(|path| {
                        path.points.len() >= 2
                            && path.points.first().is_some_and(|(sx, sy)| {
                                ((x - sx).powi(2) + (y - sy).powi(2)).sqrt() <= 5.0
                            })
                    });
                state.canvas_path_preview.set(if closes {
                    None
                } else {
                    Some(match previous {
                        Some(path) if tool == ToolId::Pen && path.tool == tool => {
                            path.with_point(x, y)
                        }
                        _ if tool == ToolId::Measure => CanvasPathPreview::line(tool, x, y),
                        _ => CanvasPathPreview::new(tool, x, y),
                    })
                });
            }
            drop(engine);
            state.engine.write().on_mouse_down(event);
            bump_render(state.render_version);
        }
        NativeInput::PointerMove { .. } => {
            let engine = state.engine.peek();
            update_cursor_position(
                state.cursor_position,
                (x - engine.viewport.pan.x) / engine.viewport.zoom,
                (y - engine.viewport.pan.y) / engine.viewport.zoom,
                engine.viewport.zoom,
            );
            drop(engine);
            let drag_preview = *state.canvas_drag_preview.peek();
            if let Some(preview) = drag_preview {
                state.canvas_drag_preview.set(Some(preview.with_current(
                    x,
                    y,
                    modifiers.ctrl,
                    modifiers.shift,
                )));
            }
            let select_preview = *state.canvas_select_preview.peek();
            if let Some(preview) = select_preview {
                state
                    .canvas_select_preview
                    .set(Some(preview.with_current(x, y)));
            }
            let path = state.canvas_path_preview.peek().clone();
            if let Some(path) = path {
                state.canvas_path_preview.set(Some(
                    if matches!(tool, ToolId::Pencil | ToolId::Calligraphy) {
                        path.with_point(x, y)
                    } else {
                        path.with_current(x, y)
                    },
                ));
            }
            if should_dispatch_canvas_mouse_move(*state.canvas_pointer_down.peek(), tool) {
                state.engine.write().on_mouse_move(event);
                if should_invalidate_canvas_bitmap_for_mouse_move_tool(tool) {
                    bump_render(state.render_version);
                }
            }
        }
        NativeInput::PointerUp { .. } => {
            state.canvas_pointer_down.set(false);
            state.canvas_drag_preview.set(None);
            state.canvas_select_preview.set(None);
            let hit = canvas_hit_id_at(&state.engine.peek(), x, y);
            let feedback = canvas_tool_feedback_on_mouse_up(
                tool,
                *state.canvas_interaction_source_hit.peek(),
                hit,
            );
            state.engine.write().on_mouse_up(event);
            if tool != ToolId::Pen {
                state.canvas_path_preview.set(None);
            }
            if feedback.is_some() {
                state.canvas_tool_feedback.set(feedback);
            }
            state.canvas_interaction_source_hit.set(None);
            bump_render(state.render_version);
        }
        _ => {}
    }
    true
}

fn row_style() -> UiStyle {
    UiStyle {
        shrink: Some(0.0),
        ..UiStyle::new()
            .display(glory::style::Display::Flex)
            .flex_direction(glory::style::FlexDirection::Row)
    }
}
fn column_style() -> UiStyle {
    UiStyle::new()
        .display(glory::style::Display::Flex)
        .flex_direction(glory::style::FlexDirection::Column)
}
fn flexible_style() -> UiStyle {
    UiStyle {
        grow: Some(1.0),
        shrink: Some(1.0),
        ..row_style()
    }
}

#[cfg(test)]
mod glory_migration_tests {
    use super::*;

    fn state(app: &glory::App) -> AppState {
        let state = AppState::new(app.scope(), DesktopContext::new());
        state.canvas_bounds.revise(|bounds| {
            *bounds = Some(glory::render::Rect {
                x: 100.0,
                y: 80.0,
                width: 800.0,
                height: 600.0,
            })
        });
        state
    }

    #[test]
    fn every_editor_panel_mounts_on_the_semantic_renderer() {
        for part in [
            EditorPart::Toolbar,
            EditorPart::ToolControls,
            EditorPart::HorizontalRuler,
            EditorPart::VerticalRuler,
            EditorPart::HorizontalScrollbar,
            EditorPart::VerticalScrollbar,
            EditorPart::ColorBar,
            EditorPart::PropertiesPanel,
            EditorPart::LayersPanel,
            EditorPart::TextPanel,
            EditorPart::ObjectsPanel,
            EditorPart::PagesPanel,
            EditorPart::AlignPanel,
            EditorPart::TransformPanel,
            EditorPart::LpePanel,
            EditorPart::FilterEditorPanel,
            EditorPart::ResourcesPanel,
            EditorPart::DocPropertiesPanel,
            EditorPart::PreferencesDialog,
            EditorPart::StatusBar,
        ] {
            let app = glory::App::memory();
            let state = state(&app);
            app.mount(widget(move |scope| build_editor_part(scope, state, part)))
                .unwrap();
            app.settle(8).unwrap();
            assert!(!app.tree().roots().is_empty());
        }
    }

    #[test]
    fn every_editor_panel_mounts_in_a_native_document() {
        for part in [
            EditorPart::Toolbar,
            EditorPart::ToolControls,
            EditorPart::HorizontalRuler,
            EditorPart::VerticalRuler,
            EditorPart::HorizontalScrollbar,
            EditorPart::VerticalScrollbar,
            EditorPart::ColorBar,
            EditorPart::PropertiesPanel,
            EditorPart::LayersPanel,
            EditorPart::TextPanel,
            EditorPart::ObjectsPanel,
            EditorPart::PagesPanel,
            EditorPart::AlignPanel,
            EditorPart::TransformPanel,
            EditorPart::LpePanel,
            EditorPart::FilterEditorPanel,
            EditorPart::ResourcesPanel,
            EditorPart::DocPropertiesPanel,
            EditorPart::PreferencesDialog,
            EditorPart::StatusBar,
        ] {
            let app = glory::apace::shell::WindowApp::new();
            app.renderer().set_viewport(800, 600, 1.0).unwrap();
            let state = state(app.app());
            let document=sivgo_dom::parse_svg(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="800" height="600"><g id="layer" inkscape:groupmode="layer" inkscape:label="Layer 1"><path id="shape" d="M10 10L100 10L100 80Z"/><text id="label" x="20" y="120">Editable text</text></g></svg>"#).unwrap();
            state.engine.write().load_document(document);
            let selected = state
                .engine
                .peek()
                .document
                .nodes()
                .find_map(|(id, node)| {
                    let selected = if matches!(part, EditorPart::TextPanel) {
                        matches!(node.element, SvgElement::Text(_))
                    } else {
                        matches!(node.element, SvgElement::Path(_))
                    };
                    selected.then_some(*id)
                })
                .unwrap();
            state.engine.write().selection.select(selected);
            app.mount(widget(move |scope| build_editor_part(scope, state, part)))
                .unwrap_or_else(|error| panic!("{part:?}: {error}"));
            app.app()
                .settle(8)
                .unwrap_or_else(|error| panic!("{part:?}: {error}"));
        }
    }

    #[test]
    fn displayed_native_menu_shortcuts_cover_fullscreen_print_and_shifted_plus() {
        let app = glory::App::memory();
        let state = state(&app);
        assert_eq!(
            menu_shortcut_action(
                state,
                &keyboard_types::Key::Named(keyboard_types::NamedKey::F11),
                keyboard_types::Modifiers::empty()
            )
            .as_deref(),
            Some("view.fullscreen")
        );
        assert_eq!(
            menu_shortcut_action(
                state,
                &keyboard_types::Key::Character("p".into()),
                keyboard_types::Modifiers::CONTROL
            )
            .as_deref(),
            Some("file.print")
        );
        assert_eq!(
            menu_shortcut_action(
                state,
                &keyboard_types::Key::Character("+".into()),
                keyboard_types::Modifiers::SHIFT
            )
            .as_deref(),
            Some("view.zoom_in")
        );
    }

    #[test]
    fn full_editor_mounts_and_changes_inspector_without_retaining_stale_nodes() {
        let app = glory::App::memory();
        let state = state(&app);
        app.mount(editor_app(state)).unwrap();
        app.settle(8).unwrap();
        for panel in [
            InspectorPanel::Text,
            InspectorPanel::Objects,
            InspectorPanel::Pages,
            InspectorPanel::Align,
            InspectorPanel::Transform,
            InspectorPanel::Lpe,
            InspectorPanel::Filters,
            InspectorPanel::Resources,
            InspectorPanel::Document,
            InspectorPanel::Preferences,
            InspectorPanel::Agent,
            InspectorPanel::Style,
        ] {
            state.inspector_panel.set(panel);
            app.settle(8).unwrap();
        }
        app.clear().unwrap();
        app.settle(8).unwrap();
        assert!(app.tree().roots().is_empty());
    }

    #[test]
    fn native_canvas_draw_undo_and_editable_keyboard_are_routed() {
        let app = glory::App::memory();
        let state = state(&app);
        state.engine.write().tools.switch_to(ToolId::Rect);
        state.active_tool.set(ToolId::Rect);
        let before = state.engine.peek().document.len();
        let modifiers = keyboard_types::Modifiers::empty();
        assert!(!handle_native_input(
            state,
            NativeInput::PointerDown {
                x: 20.0,
                y: 20.0,
                button: sivgo_core::MouseButton::Left,
                modifiers,
            }
        ));
        assert!(handle_native_input(
            state,
            NativeInput::PointerDown {
                x: 140.0,
                y: 120.0,
                button: sivgo_core::MouseButton::Left,
                modifiers,
            }
        ));
        assert!(handle_native_input(
            state,
            NativeInput::PointerMove {
                x: 240.0,
                y: 190.0,
                button: sivgo_core::MouseButton::Left,
                modifiers,
            }
        ));
        assert!(handle_native_input(
            state,
            NativeInput::PointerUp {
                x: 240.0,
                y: 190.0,
                button: sivgo_core::MouseButton::Left,
                modifiers,
            }
        ));
        assert!(state.engine.peek().document.len() > before);
        assert!(!*state.canvas_pointer_down.peek());
        assert!(!handle_native_input(
            state,
            NativeInput::KeyDown {
                key: keyboard_types::Key::Named(keyboard_types::NamedKey::Delete),
                modifiers,
                editable: true,
            }
        ));
        assert!(state.engine.peek().document.len() > before);
        state.action("edit.undo");
        assert_eq!(state.engine.peek().document.len(), before);
        state.action("edit.redo");
        assert!(state.engine.peek().document.len() > before);
    }

    #[test]
    fn canvas_layout_is_measured_in_a_real_apace_document() {
        let app = glory::apace::shell::WindowApp::new();
        app.renderer().set_viewport(1440, 900, 1.0).unwrap();
        app.renderer().set_css(&format!("{CSS}\n{HOST_CSS}"));
        let state = AppState::new(app.app().scope(), DesktopContext::new());
        app.mount(editor_app(state)).unwrap();
        refresh_canvas_bounds(app.app().scope(), state);
        app.app().settle(16).unwrap();
        for index in 0..default_menu_bar_with_state(true, true, true).len() {
            state.menu_open.revise(|open| *open = Some(index));
            app.app().settle(16).unwrap();
        }
        state.menu_open.revise(|open| *open = None);
        app.app().settle(16).unwrap();
        let bounds = state.canvas_bounds.get().expect("canvas query completed");
        assert!(
            bounds.width > 100.0,
            "canvas must have a drawable width: {bounds:?}"
        );
        assert!(
            bounds.height > 100.0,
            "canvas must have a drawable height: {bounds:?}"
        );
        assert!(bounds.x >= 0.0 && bounds.y >= 0.0);
        assert!(
            bounds.x + bounds.width <= 1441.0,
            "canvas must fit the window: {bounds:?}"
        );
        assert!(
            bounds.y + bounds.height <= 901.0,
            "canvas must fit the window: {bounds:?}"
        );
    }
}

#[cfg(test)]
mod usability_tests;
