#[allow(unused_variables, unused_mut)]
fn build_editor_part(scope: &Scope, state: AppState, part: EditorPart) -> glory::ui::Result {
    let mut engine = state.engine;
    let mut render_version = state.render_version;
    let mut canvas_pointer_down = state.canvas_pointer_down;
    let mut canvas_drag_preview = state.canvas_drag_preview;
    let mut canvas_path_preview = state.canvas_path_preview;
    let mut canvas_select_preview = state.canvas_select_preview;
    let mut canvas_tool_feedback = state.canvas_tool_feedback;
    let mut canvas_interaction_source_hit = state.canvas_interaction_source_hit;
    let mut file_path = state.file_path;
    let mut active_tool = state.active_tool;
    let mut is_fullscreen = state.is_fullscreen;
    let mut inspector_panel = state.inspector_panel;
    let mut document_defaults = state.document_defaults;
    let mut preferences_state = state.preferences_state;
    let mut shortcuts = state.shortcuts;
    let mut layout_settings = state.layout_settings;
    let mut inspector_resize_drag = state.inspector_resize_drag;
    let mut window_size = state.window_size;
    let mut align_relative_to = state.align_relative_to;
    let mut transform_lock_aspect = state.transform_lock_aspect;
    let mut transform_percent_mode = state.transform_percent_mode;
    let mut object_editing_id = state.object_editing_id;
    let mut object_edit_value = state.object_edit_value;
    let mut object_xml_id_editing_id = state.object_xml_id_editing_id;
    let mut object_xml_id_edit_value = state.object_xml_id_edit_value;
    let mut object_dragging_id = state.object_dragging_id;
    let mut layer_editing_id = state.layer_editing_id;
    let mut layer_edit_value = state.layer_edit_value;
    let mut lpe_selected_id = state.lpe_selected_id;
    let mut filter_selected_id = state.filter_selected_id;
    let mut resource_kind_filter = state.resource_kind_filter;
    let mut resource_selected_id = state.resource_selected_id;
    let cursor_position = state.cursor_position;
    let desktop = state.desktop.get();

    let (doc_width, doc_height) = document_pixel_size(&engine.peek());
    let layers = build_layer_entries(&engine.peek());
    let objects = object_tree_entries(&engine.peek());
    let pages = page_entries(&engine.peek());
    let lpe_entries = selected_lpe_entries(&engine.peek());
    let filter_primitives = selected_filter_primitives(&engine.peek());
    let resource_entries = document_resource_entries(&engine.peek());
    let guides = engine.peek().guide_lines();
    let eng = engine.peek();
    let can_undo = eng.history.can_undo();
    let can_redo = eng.history.can_redo();
    let selection_count = eng.selection.len();
    let primary_selection = eng.selection.primary();
    let object_count = objects.len();
    let has_document_drawable_target = document_has_drawable_target(&eng);
    let has_document_path_target = document_has_path_target(&eng);
    let has_lpe_target = selected_path_has_lpe_target(&eng);
    let has_filter_target = eng.selected_filter_target_available();
    let text_state = eng.selected_text_state();
    let selected_object_details = selected_object_details(&eng);
    let presentation = if matches!(part, EditorPart::PropertiesPanel | EditorPart::ColorBar) {
        selected_presentation(&eng)
    } else {
        PresentationAttrs::default()
    };
    let (fill_hex, fill_is_none) = get_selected_fill(&presentation);
    let (stroke_hex, stroke_is_none) = get_selected_stroke(&presentation);
    let stroke_width_val = get_selected_stroke_width(&presentation);
    let style_state = selected_style_state(&eng, &presentation);
    let selection_bounds = eng.selection.bounding_box(&eng.document);
    let transform_decomposition = eng.primary_transform_decomposition().unwrap_or_default();
    let zoom = eng.viewport.zoom;
    let pan_x = eng.viewport.pan.x;
    let pan_y = eng.viewport.pan.y;
    let display_mode = eng.viewport.display_mode;
    let snap_indicator = eng.last_snap_indicator.clone();
    let snap_grid = eng.snapping.prefs.snap_to_grid;
    let snap_guides = eng.snapping.prefs.snap_to_guides;
    let doc_grid_visible = grid_visible(&eng);
    let grid_spacing = eng.snapping.prefs.grid_spacing;
    let snap_objects = eng.snapping.prefs.snap_to_bbox_edges
        || eng.snapping.prefs.snap_to_bbox_centers
        || eng.snapping.prefs.snap_to_bbox_corners;
    let snap_paths = eng.snapping.prefs.snap_to_path || eng.snapping.prefs.snap_to_path_nodes;
    let doc_title = root_text(&eng, "title");
    let doc_author = root_text(&eng, "metadata");
    let doc_description = root_text(&eng, "desc");
    let doc_background = page_color(&eng);
    drop(eng);

    let current_file = file_path.read().clone();
    let document_name = current_document_name(current_file.as_ref());
    let active_tool_id = *active_tool.read();
    let active_tool_name = tool_label(active_tool_id);
    let active_canvas_drag_preview = *canvas_drag_preview.read();
    let active_canvas_path_preview = canvas_path_preview.read().clone();
    let active_canvas_select_preview = *canvas_select_preview.read();
    let active_canvas_tool_feedback = *canvas_tool_feedback.read();
    let canvas_cursor = canvas_cursor_for_tool(active_tool_id);
    let capture_canvas_mouse_move =
        should_capture_canvas_mouse_move(*canvas_pointer_down.read(), active_tool_id);
    let zoom_pct = format!("{:.0}%", zoom * 100.0);
    let display_mode_name = match display_mode {
        DisplayMode::Normal => "Normal",
        DisplayMode::Outline => "Outline",
        DisplayMode::NoFilters => "No filters",
        DisplayMode::VisibleHairlines => "Hairlines",
        DisplayMode::PrintPreview => "Print",
        DisplayMode::ProofColors => "Proof",
    };
    let (sel_x, sel_y, sel_w, sel_h) = selection_bounds
        .map(|bounds| (bounds.x0, bounds.y0, bounds.width(), bounds.height()))
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    let doc_width_mm = px_to_mm(doc_width as f64);
    let doc_height_mm = px_to_mm(doc_height as f64);
    let workspace_doc_width = pages.iter().fold(doc_width as f64, |max_w, page| {
        max_w.max(page.x + page.width)
    });
    let workspace_doc_height = pages.iter().fold(doc_height as f64, |max_h, page| {
        max_h.max(page.y + page.height)
    });
    let canvas_w = workspace_doc_width * zoom;
    let canvas_h = workspace_doc_height * zoom;
    let document_canvas_w = doc_width as f64 * zoom;
    let document_canvas_h = doc_height as f64 * zoom;
    let document_size = format!("{} x {} px", doc_width, doc_height);
    let active_inspector = *inspector_panel.read();
    let preferences_snapshot = preferences_state.read().clone();
    let align_relative_snapshot = align_relative_to.read().clone();
    let transform_lock_aspect_value = *transform_lock_aspect.read();
    let transform_percent_mode_value = *transform_percent_mode.read();
    let object_editing_snapshot = *object_editing_id.read();
    let object_edit_value_snapshot = object_edit_value.read().clone();
    let object_xml_id_editing_snapshot = *object_xml_id_editing_id.read();
    let object_xml_id_edit_value_snapshot = object_xml_id_edit_value.read().clone();
    let selected_object_xml_id_value = selected_object_details
        .as_ref()
        .map(|details| {
            if object_xml_id_editing_snapshot == Some(details.id) {
                object_xml_id_edit_value_snapshot.clone()
            } else {
                details.xml_id.clone()
            }
        })
        .unwrap_or_default();
    let layer_editing_snapshot = *layer_editing_id.read();
    let layer_edit_value_snapshot = layer_edit_value.read().clone();
    let lpe_selected_snapshot = lpe_selected_id
        .read()
        .clone()
        .filter(|id| lpe_entries.iter().any(|entry| &entry.id == id))
        .or_else(|| lpe_entries.first().map(|entry| entry.id.clone()));
    let filter_selected_snapshot = filter_selected_id
        .read()
        .clone()
        .filter(|id| filter_primitives.iter().any(|entry| &entry.id == id))
        .or_else(|| filter_primitives.first().map(|entry| entry.id.clone()));
    let resource_kind_filter_snapshot = resource_kind_filter.read().clone();
    let resource_selected_snapshot = resource_selected_id
        .read()
        .clone()
        .filter(|id| resource_entries.iter().any(|entry| &entry.id == id));
    let has_text_selection = text_state.is_some();
    let text_content_value = text_state
        .as_ref()
        .map(|state| state.content.clone())
        .unwrap_or_default();
    let text_font_family_value = text_state
        .as_ref()
        .map(|state| state.font_family.clone())
        .unwrap_or_else(|| "sans-serif".to_string());
    let text_font_size_value = text_state
        .as_ref()
        .map(|state| state.font_size)
        .unwrap_or(16.0);
    let text_font_weight_value = text_state
        .as_ref()
        .map(|state| state.font_weight.clone())
        .unwrap_or_else(|| "Normal".to_string());
    let text_font_style_value = text_state
        .as_ref()
        .map(|state| state.font_style.clone())
        .unwrap_or_else(|| "Normal".to_string());
    let text_line_height_value = text_state
        .as_ref()
        .map(|state| state.line_height)
        .unwrap_or(1.2);
    let text_letter_spacing_value = text_state
        .as_ref()
        .map(|state| state.letter_spacing)
        .unwrap_or(0.0);
    let text_word_spacing_value = text_state
        .as_ref()
        .map(|state| state.word_spacing)
        .unwrap_or(0.0);
    let text_anchor_value = text_state
        .as_ref()
        .map(|state| state.text_anchor.clone())
        .unwrap_or_else(|| "Start".to_string());
    let text_align_css = match text_anchor_value.as_str() {
        "Middle" => "center",
        "End" => "right",
        _ => "left",
    };
    let text_canvas_editor_style =
        (active_tool_id == ToolId::Text && has_text_selection && selection_count == 1).then(|| {
            let left = sel_x * zoom + pan_x;
            let top = sel_y * zoom + pan_y;
            let width = (sel_w.max(120.0) * zoom).max(120.0);
            let height = (sel_h.max(text_font_size_value * text_line_height_value) * zoom)
                .max(32.0);
            let font_size = (text_font_size_value * zoom).clamp(8.0, 96.0);
            format!(
                "left: {left}px; top: {top}px; width: {width}px; min-height: {height}px; font-family: {font}; font-size: {font_size}px; line-height: {line_height}; text-align: {align};",
                font = text_font_family_value,
                line_height = text_line_height_value,
                align = text_align_css,
            )
        });
    let transform_width_value = if transform_percent_mode_value {
        100.0
    } else {
        sel_w
    };
    let transform_height_value = if transform_percent_mode_value {
        100.0
    } else {
        sel_h
    };
    let selection_summary = if selection_count == 0 {
        "No selection".to_string()
    } else if selection_count == 1 {
        "1 selected".to_string()
    } else {
        format!("{} selected", selection_count)
    };
    let pages_for_canvas_events = pages.clone();
    let inspector_width_px = layout_settings.read().inspector_width_px;
    let workspace_style =
        format!("grid-template-columns: 44px minmax(0, 1fr) 8px {inspector_width_px}px;");
    let inspector_resizing = inspector_resize_drag.read().is_some();
    let app_class = if inspector_resizing {
        "app resizing-inspector"
    } else {
        "app"
    };
    let inspector_resize_handle_class = if inspector_resizing {
        "inspector-resize-handle active"
    } else {
        "inspector-resize-handle"
    };
    let (window_width, _) = *window_size.read();
    let canvas_viewport_width = state
        .canvas_bounds
        .get()
        .map(|bounds| bounds.width)
        .unwrap_or_else(|| canvas_viewport_width_for_window(window_width, inspector_width_px))
        .max(1.0);
    let canvas_viewport_height = state
        .canvas_bounds
        .get()
        .map(|bounds| bounds.height)
        .unwrap_or(480.0)
        .max(1.0);

    match part {
        EditorPart::Toolbar => {
            Toolbar {
                active_tool: active_tool_id,
                on_tool_change: Callback::new(move |id: ToolId| {
                    let mut eng = engine.write();
                    eng.tools.switch_to(id);
                    let actual = eng.tools.active_id();
                    drop(eng);
                    active_tool.set(actual);
                    canvas_pointer_down.set(false);
                    canvas_drag_preview.set(None);
                    canvas_path_preview.set(None);
                    canvas_select_preview.set(None);
                    canvas_tool_feedback.set(None);
                    canvas_interaction_source_hit.set(None);
                    if let Some(panel) = inspector_panel_for_tool(actual) {
                        inspector_panel.set(panel);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::ToolControls => {
            ToolControls {
                active_tool: active_tool_id,
                sel_x,
                sel_y,
                sel_w,
                sel_h,
                snap_grid,
                snap_guides,
                snap_objects,
                snap_paths,
                on_action: Callback::new({
                    let tool_control_font_weight = text_font_weight_value.clone();
                    let tool_control_font_style = text_font_style_value.clone();
                    move |action: ToolControlAction| {
                        match action {
                            ToolControlAction::ToggleSnapToGrid => {
                                let next = {
                                    let mut eng = engine.write();
                                    eng.snapping.prefs.snap_to_grid =
                                        !eng.snapping.prefs.snap_to_grid;
                                    eng.snapping.prefs.snap_to_grid
                                };
                                preferences_state.write().snap_to_grid = next;
                            }
                            ToolControlAction::ToggleSnapToGuides => {
                                let next = {
                                    let mut eng = engine.write();
                                    eng.snapping.prefs.snap_to_guides =
                                        !eng.snapping.prefs.snap_to_guides;
                                    eng.snapping.prefs.snap_to_guides
                                };
                                preferences_state.write().snap_to_guides = next;
                            }
                            ToolControlAction::ToggleSnapToObjects => {
                                let next = {
                                    let mut eng = engine.write();
                                    let next = !(eng.snapping.prefs.snap_to_bbox_edges
                                        || eng.snapping.prefs.snap_to_bbox_centers
                                        || eng.snapping.prefs.snap_to_bbox_corners);
                                    eng.snapping.prefs.snap_to_bbox_edges = next;
                                    eng.snapping.prefs.snap_to_bbox_centers = next;
                                    eng.snapping.prefs.snap_to_bbox_corners = next;
                                    eng.snapping.prefs.snap_to_midpoints = next;
                                    next
                                };
                                preferences_state.write().snap_to_objects = next;
                            }
                            ToolControlAction::ToggleSnapToPaths => {
                                let next = {
                                    let mut eng = engine.write();
                                    let next = !(eng.snapping.prefs.snap_to_path
                                        || eng.snapping.prefs.snap_to_path_nodes);
                                    eng.snapping.prefs.snap_to_path = next;
                                    eng.snapping.prefs.snap_to_path_nodes = next;
                                    next
                                };
                                let mut prefs = preferences_state.write();
                                prefs.snap_to_paths = next;
                                prefs.snap_to_nodes = next;
                            }
                            // -- Selection geometry --
                            // Expressed as a move or scale so the change
                            // goes through the command stack and can be
                            // undone like any other edit.
                            ToolControlAction::SetX(x) => {
                                if sel_w > 0.0 || sel_h > 0.0 {
                                    engine.write().translate_selected(x - sel_x, 0.0);
                                }
                            }
                            ToolControlAction::SetY(y) => {
                                if sel_w > 0.0 || sel_h > 0.0 {
                                    engine.write().translate_selected(0.0, y - sel_y);
                                }
                            }
                            ToolControlAction::SetWidth(w) => {
                                if sel_w > f64::EPSILON && w > 0.0 {
                                    engine.write().scale_selected(w / sel_w, 1.0);
                                }
                            }
                            ToolControlAction::SetHeight(h) => {
                                if sel_h > f64::EPSILON && h > 0.0 {
                                    engine.write().scale_selected(1.0, h / sel_h);
                                }
                            }
                            ToolControlAction::SetShapeWidth(w) => {
                                if sel_w > f64::EPSILON && w > 0.0 {
                                    engine.write().scale_selected(w / sel_w, 1.0);
                                }
                            }
                            ToolControlAction::SetShapeHeight(h) => {
                                if sel_h > f64::EPSILON && h > 0.0 {
                                    engine.write().scale_selected(1.0, h / sel_h);
                                }
                            }
                            ToolControlAction::SetCornerRadius(radius) => {
                                engine.write().set_selected_corner_radius(radius);
                            }

                            // -- Shape tool settings --
                            // These configure the tool itself, so the next
                            // shape drawn uses them.
                            ToolControlAction::SetStarCorners(points) => {
                                if let Some(tool) =
                                    engine
                                        .write()
                                        .tools
                                        .get_mut::<sivgo_core::tools::star::StarTool>(ToolId::Star)
                                {
                                    tool.num_points = points.max(3);
                                }
                            }
                            ToolControlAction::SetStarRatio(ratio) => {
                                if let Some(tool) =
                                    engine
                                        .write()
                                        .tools
                                        .get_mut::<sivgo_core::tools::star::StarTool>(ToolId::Star)
                                {
                                    tool.inner_ratio = ratio.clamp(0.01, 1.0);
                                }
                            }
                            ToolControlAction::SetSpiralTurns(turns) => {
                                if let Some(tool) = engine
                                    .write()
                                    .tools
                                    .get_mut::<sivgo_core::tools::spiral::SpiralTool>(
                                    ToolId::Spiral,
                                ) {
                                    tool.turns = turns.clamp(0.1, 100.0);
                                }
                            }
                            ToolControlAction::SetSmoothing(amount) => {
                                if let Some(tool) = engine
                                    .write()
                                    .tools
                                    .get_mut::<sivgo_core::tools::pencil::PencilTool>(
                                    ToolId::Pencil,
                                ) {
                                    tool.set_smoothing(amount);
                                }
                            }

                            // -- Text --
                            ToolControlAction::SetFontFamily(family) => {
                                engine.write().set_selected_text_font_family(family);
                            }
                            ToolControlAction::SetFontSize(size) => {
                                engine.write().set_selected_text_font_size(size);
                            }
                            ToolControlAction::ToggleBold => {
                                let bold = tool_control_font_weight == "Bold";
                                let next = if bold { "Normal" } else { "Bold" };
                                engine.write().set_selected_text_font_weight(next);
                            }
                            ToolControlAction::ToggleItalic => {
                                let italic = tool_control_font_style == "Italic";
                                let next = if italic { "Normal" } else { "Italic" };
                                engine.write().set_selected_text_font_style(next);
                            }

                            other => {
                                log::info!("Tool control action is not wired yet: {:?}", other)
                            }
                        }
                        let prefs_snapshot = preferences_state.read().clone();
                        let defaults_snapshot = document_defaults.read().clone();
                        persist_desktop_settings(&prefs_snapshot, &defaults_snapshot);
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::HorizontalRuler => {
            HorizontalRuler {
                width: canvas_viewport_width,
                scroll_x: pan_x,
                zoom,
                cursor_position,
                on_guide_drag: Callback::new(move |global_y: f64| {
                    let eng = engine.peek();
                    let y = (global_y
                        - state.canvas_bounds.get().map(|b| b.y).unwrap_or_default()
                        - eng.viewport.pan.y)
                        / eng.viewport.zoom;
                    drop(eng);
                    if engine.write().add_horizontal_guide(y).is_some() {
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::VerticalRuler => {
            VerticalRuler {
                height: canvas_viewport_height,
                scroll_y: pan_y,
                zoom,
                cursor_position,
                on_guide_drag: Callback::new(move |global_x: f64| {
                    let eng = engine.peek();
                    let x = (global_x
                        - state.canvas_bounds.get().map(|b| b.x).unwrap_or_default()
                        - eng.viewport.pan.x)
                        / eng.viewport.zoom;
                    drop(eng);
                    if engine.write().add_vertical_guide(x).is_some() {
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::HorizontalScrollbar => {
            HorizontalScrollbar {
                track_length: canvas_viewport_width,
                document_width: workspace_doc_width,
                pan_x,
                zoom,
                on_scroll: Callback::new(move |new_pan_x: f64| {
                    engine.write().viewport.pan.x = new_pan_x;
                    bump_render(render_version);
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::VerticalScrollbar => {
            VerticalScrollbar {
                track_length: canvas_viewport_height,
                document_height: workspace_doc_height,
                pan_y,
                zoom,
                on_scroll: Callback::new(move |new_pan_y: f64| {
                    engine.write().viewport.pan.y = new_pan_y;
                    bump_render(render_version);
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::ColorBar => {
            if selection_count == 0 {
                text()
                    .content("Select an object to edit its fill and stroke")
                    .style(
                        UiStyle::new()
                            .height(UiLength::Px(32.0))
                            .padding(Edges::all(6.0)),
                    )
                    .show_in(scope)?;
                return Ok(());
            }
            ColorBar {
                fill_color: fill_hex.clone(),
                stroke_color: stroke_hex.clone(),
                fill_none: fill_is_none,
                stroke_none: stroke_is_none,
                on_fill_click: Callback::new(move |_| {
                    inspector_panel.set(InspectorPanel::Style);
                }),
                on_stroke_click: Callback::new(move |_| {
                    inspector_panel.set(InspectorPanel::Style);
                }),
                on_set_none: Callback::new(move |_| {
                    let mut eng = engine.write();
                    if let Some(id) = eng.selection.primary() {
                        if let Some(node) = eng.document.get_node(id) {
                            let old_attrs = node.presentation.clone();
                            let mut new_attrs = old_attrs.clone();
                            new_attrs.fill = Some(Paint::None);
                            let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                            eng.execute(Box::new(cmd));
                        }
                    }
                    drop(eng);
                    bump_render(render_version);
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::PropertiesPanel => {
            PropertiesPanel {
fill: fill_hex.clone(),
fill_paint_type: style_state.fill_paint_type.to_string(),
fill_opacity: style_state.fill_opacity,
stroke: stroke_hex.clone(),
stroke_paint_type: style_state.stroke_paint_type.to_string(),
stroke_opacity: style_state.stroke_opacity,
stroke_width: stroke_width_val,
line_cap: style_state.line_cap.to_string(),
line_join: style_state.line_join.to_string(),
dash_pattern: style_state.dash_pattern.clone(),
dash_offset: style_state.dash_offset,
marker_start: style_state.marker_start.clone(),
marker_mid: style_state.marker_mid.clone(),
marker_end: style_state.marker_end.clone(),
has_selection: selection_count > 0,
on_action: Callback::new(move |action: sivgo_ui::properties_panel::PropertiesAction| {
                                    use sivgo_ui::properties_panel::{PaintType, PropertiesAction};
                                    let changed = match action {
                                        PropertiesAction::SetFillPaintType(paint_type) => match paint_type {
                                            PaintType::None => modify_primary_presentation(engine, |attrs| {
                                                attrs.fill = Some(Paint::None);
                                            }),
                                            PaintType::FlatColor => modify_primary_presentation(engine, |attrs| {
                                                if !matches!(attrs.fill, Some(Paint::Color(_))) {
                                                    attrs.fill = Some(Paint::Color(Color::BLACK));
                                                }
                                            }),
                                            other => {
                                                log::info!(
                                                    "Fill paint type '{}' needs an existing paint server; assign it via url(#id).",
                                                    other.label()
                                                );
                                                false
                                            }
                                        },
                                        PropertiesAction::SetFillColor(hex) => {
                                            hex_to_color(&hex).is_some_and(|color| {
                                                modify_primary_presentation(engine, |attrs| {
                                                    attrs.fill = Some(Paint::Color(color));
                                                })
                                            })
                                        }
                                        PropertiesAction::SetFillOpacity(value) => {
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.fill_opacity = Some(value.clamp(0.0, 1.0));
                                            })
                                        }
                                        PropertiesAction::SetStrokePaintType(paint_type) => match paint_type {
                                            PaintType::None => modify_primary_presentation(engine, |attrs| {
                                                attrs.stroke = Some(Paint::None);
                                            }),
                                            PaintType::FlatColor => modify_primary_presentation(engine, |attrs| {
                                                if !matches!(attrs.stroke, Some(Paint::Color(_))) {
                                                    attrs.stroke = Some(Paint::Color(Color::BLACK));
                                                }
                                            }),
                                            other => {
                                                log::info!(
                                                    "Stroke paint type '{}' needs an existing paint server; assign it via url(#id).",
                                                    other.label()
                                                );
                                                false
                                            }
                                        },
                                        PropertiesAction::SetStrokeColor(hex) => {
                                            hex_to_color(&hex).is_some_and(|color| {
                                                modify_primary_presentation(engine, |attrs| {
                                                    attrs.stroke = Some(Paint::Color(color));
                                                })
                                            })
                                        }
                                        PropertiesAction::SetStrokeOpacity(value) => {
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.stroke_opacity = Some(value.clamp(0.0, 1.0));
                                            })
                                        }
                                        PropertiesAction::SetStrokeWidth(value) => {
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.stroke_width = Some(SvgLength::new(
                                                    value.max(0.0),
                                                    LengthUnit::None,
                                                ));
                                            })
                                        }
                                        PropertiesAction::SetLineCap(value) => {
                                            LineCap::parse(&value.to_ascii_lowercase()).is_some_and(|cap| {
                                                modify_primary_presentation(engine, |attrs| {
                                                    attrs.stroke_linecap = Some(cap);
                                                })
                                            })
                                        }
                                        PropertiesAction::SetLineJoin(value) => {
                                            LineJoin::parse(&value.to_ascii_lowercase()).is_some_and(|join| {
                                                modify_primary_presentation(engine, |attrs| {
                                                    attrs.stroke_linejoin = Some(join);
                                                })
                                            })
                                        }
                                        PropertiesAction::SetDashPattern(value) => {
                                            let dashes = parse_dash_pattern(&value);
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.stroke_dasharray = dashes;
                                            })
                                        }
                                        PropertiesAction::SetDashOffset(value) => {
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.stroke_dashoffset =
                                                    (value != 0.0).then_some(value);
                                            })
                                        }
                                        PropertiesAction::SetMarkerStart(value) => {
                                            let marker = normalize_marker_ref(value);
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.marker_start = marker;
                                            })
                                        }
                                        PropertiesAction::SetMarkerMid(value) => {
                                            let marker = normalize_marker_ref(value);
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.marker_mid = marker;
                                            })
                                        }
                                        PropertiesAction::SetMarkerEnd(value) => {
                                            let marker = normalize_marker_ref(value);
                                            modify_primary_presentation(engine, |attrs| {
                                                attrs.marker_end = marker;
                                            })
                                        }
                                    };
                                    if changed {
                                        bump_render(render_version);
                                    }
                                })
}.show_in(scope)?;
        }
        EditorPart::LayersPanel => {
            LayersPanel {
                layers,
                editing_id: layer_editing_snapshot,
                edit_value: layer_edit_value_snapshot.clone(),
                on_edit_change: Callback::new(move |value: String| {
                    layer_edit_value.set(value);
                }),
                on_action: Callback::new(move |action: sivgo_ui::layers_panel::LayerAction| {
                    use sivgo_ui::layers_panel::LayerAction;
                    match action {
                        LayerAction::Select(id) => {
                            let mut eng = engine.write();
                            eng.selection.select(id);
                            if is_layer_node(&eng, id) {
                                eng.set_active_layer(id);
                            }
                            drop(eng);
                            layer_editing_id.set(None);
                            bump_render(render_version);
                        }
                        LayerAction::BeginRename(id) => {
                            let label = {
                                let eng = engine.read();
                                eng.get_layers()
                                    .into_iter()
                                    .find_map(|(layer_id, label)| (layer_id == id).then_some(label))
                                    .unwrap_or_else(|| "Layer".to_string())
                            };
                            layer_editing_id.set(Some(id));
                            layer_edit_value.set(label);
                        }
                        LayerAction::Rename(id, value) => {
                            engine.write().rename_layer(id, &value);
                            layer_editing_id.set(None);
                            layer_edit_value.set(String::new());
                            bump_render(render_version);
                        }
                        LayerAction::ToggleVisibility(id) => {
                            engine.write().toggle_visibility(id);
                            bump_render(render_version);
                        }
                        LayerAction::ToggleLock(id) => {
                            engine.write().toggle_lock(id);
                            bump_render(render_version);
                        }
                        LayerAction::MoveUp(id) => {
                            let mut eng = engine.write();
                            eng.selection.select(id);
                            eng.raise_selected();
                            drop(eng);
                            bump_render(render_version);
                        }
                        LayerAction::MoveDown(id) => {
                            let mut eng = engine.write();
                            eng.selection.select(id);
                            eng.lower_selected();
                            drop(eng);
                            bump_render(render_version);
                        }
                        LayerAction::SetOpacity(id, value) => {
                            let mut eng = engine.write();
                            if let Some(node) = eng.document.get_node(id) {
                                let old_attrs = node.presentation.clone();
                                let mut new_attrs = old_attrs.clone();
                                new_attrs.opacity = Some(value);
                                let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                eng.execute(Box::new(cmd));
                            }
                            drop(eng);
                            bump_render(render_version);
                        }
                        LayerAction::SetBlendMode(id, mode) => {
                            if let Some(blend) = BlendMode::parse(&mode) {
                                let mut eng = engine.write();
                                if let Some(node) = eng.document.get_node(id) {
                                    let old_attrs = node.presentation.clone();
                                    let mut new_attrs = old_attrs.clone();
                                    new_attrs.mix_blend_mode =
                                        (blend != BlendMode::Normal).then_some(blend);
                                    let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                    eng.execute(Box::new(cmd));
                                }
                                drop(eng);
                                bump_render(render_version);
                            }
                        }
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::TextPanel => {
            TextPanel {
                text_content: text_content_value.clone(),
                font_family: text_font_family_value.clone(),
                font_size: text_font_size_value,
                font_weight: text_font_weight_value.clone(),
                font_style: text_font_style_value.clone(),
                line_height: text_line_height_value,
                letter_spacing: text_letter_spacing_value,
                word_spacing: text_word_spacing_value,
                text_align: text_anchor_value.clone(),
                has_text_selection,
                on_action: Callback::new(move |action: UiTextAction| {
                    let changed = match action {
                        UiTextAction::SetTextContent(value) => {
                            engine.write().set_selected_text_content(value)
                        }
                        UiTextAction::SetFontFamily(value) => {
                            engine.write().set_selected_text_font_family(value)
                        }
                        UiTextAction::SetFontSize(value) => {
                            engine.write().set_selected_text_font_size(value)
                        }
                        UiTextAction::SetFontWeight(value) => {
                            engine.write().set_selected_text_font_weight(&value)
                        }
                        UiTextAction::SetFontStyle(value) => {
                            engine.write().set_selected_text_font_style(&value)
                        }
                        UiTextAction::SetLineHeight(value) => {
                            engine.write().set_selected_text_line_height(value)
                        }
                        UiTextAction::SetLetterSpacing(value) => {
                            engine.write().set_selected_text_letter_spacing(value)
                        }
                        UiTextAction::SetWordSpacing(value) => {
                            engine.write().set_selected_text_word_spacing(value)
                        }
                        UiTextAction::SetTextAlign(value) => {
                            engine.write().set_selected_text_anchor(&value)
                        }
                    };
                    if changed {
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::ObjectsPanel => {
            ObjectsPanel {
                objects: objects.clone(),
                editing_id: object_editing_snapshot,
                edit_value: object_edit_value_snapshot.clone(),
                selected_details: selected_object_details.clone(),
                xml_id_edit_value: selected_object_xml_id_value.clone(),
                on_edit_change: Callback::new(move |value: String| {
                    object_edit_value.set(value);
                }),
                on_xml_id_change: Callback::new(move |value: String| {
                    if let Some(id) = engine.read().selection.primary() {
                        object_xml_id_editing_id.set(Some(id));
                        object_xml_id_edit_value.set(value);
                    }
                }),
                on_action: Callback::new(move |action: ObjectAction| match action {
                    ObjectAction::Select(id) => {
                        engine.write().select_node(id);
                        object_editing_id.set(None);
                        object_xml_id_editing_id.set(None);
                        object_xml_id_edit_value.set(String::new());
                        bump_render(render_version);
                    }
                    ObjectAction::BeginRename(id) => {
                        let label = {
                            let eng = engine.read();
                            object_tree_label(&eng, id)
                        };
                        object_editing_id.set(Some(id));
                        object_edit_value.set(label);
                    }
                    ObjectAction::Rename(id, value) => {
                        engine.write().set_object_label(id, value);
                        object_editing_id.set(None);
                        object_edit_value.set(String::new());
                        bump_render(render_version);
                    }
                    ObjectAction::SetXmlId(id, value) => {
                        engine.write().set_object_xml_id(id, value);
                        object_xml_id_editing_id.set(None);
                        object_xml_id_edit_value.set(String::new());
                        bump_render(render_version);
                    }
                    ObjectAction::BeginDrag(id) => {
                        object_dragging_id.set(Some(id));
                    }
                    ObjectAction::EndDrag => {
                        object_dragging_id.set(None);
                    }
                    ObjectAction::DropOn(target_id) => {
                        let dragged = { *object_dragging_id.read() };
                        if let Some(dragged_id) = dragged {
                            engine
                                .write()
                                .reparent_node_for_object_drop(dragged_id, target_id);
                            object_dragging_id.set(None);
                            bump_render(render_version);
                        }
                    }
                    ObjectAction::DropToRoot => {
                        let dragged = { *object_dragging_id.read() };
                        if let Some(dragged_id) = dragged {
                            engine.write().reparent_node_to_root(dragged_id);
                            object_dragging_id.set(None);
                            bump_render(render_version);
                        }
                    }
                    ObjectAction::ToggleVisibility(id) => {
                        engine.write().toggle_visibility(id);
                        bump_render(render_version);
                    }
                    ObjectAction::ToggleLock(id) => {
                        engine.write().toggle_lock(id);
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::PagesPanel => {
            PagesPanel {
                pages: pages.clone(),
                on_action: Callback::new(move |action: PageAction| match action {
                    PageAction::Add => {
                        let mut eng = engine.write();
                        add_page_like_previous(&mut eng);
                        drop(eng);
                        bump_render(render_version);
                    }
                    PageAction::AddTemplate(template) => {
                        let mut eng = engine.write();
                        add_page_from_template(&mut eng, template);
                        drop(eng);
                        bump_render(render_version);
                    }
                    PageAction::Navigate(id) => {
                        engine.write().navigate_to_page(id);
                        bump_render(render_version);
                    }
                    PageAction::Delete(id) => {
                        let mut eng = engine.write();
                        eng.delete_page(id);
                        if let Some((next_id, _, _, _, _, _)) = eng.get_pages().first().cloned() {
                            eng.navigate_to_page(next_id);
                        }
                        drop(eng);
                        bump_render(render_version);
                    }
                    PageAction::SetLabel(id, value) => {
                        engine.write().set_page_label(id, value);
                        bump_render(render_version);
                    }
                    PageAction::SetX(id, value) => {
                        let current = {
                            let eng = engine.read();
                            eng.get_pages().into_iter().find(|page| page.0 == id)
                        };
                        if let Some((_, _, _, y, _, _)) = current {
                            engine.write().move_page(id, value, y);
                            bump_render(render_version);
                        }
                    }
                    PageAction::SetY(id, value) => {
                        let current = {
                            let eng = engine.read();
                            eng.get_pages().into_iter().find(|page| page.0 == id)
                        };
                        if let Some((_, _, x, _, _, _)) = current {
                            engine.write().move_page(id, x, value);
                            bump_render(render_version);
                        }
                    }
                    PageAction::SetWidth(id, value) => {
                        let current = {
                            let eng = engine.read();
                            eng.get_pages().into_iter().find(|page| page.0 == id)
                        };
                        if let Some((_, _, _, _, _, height)) = current {
                            engine.write().resize_page(id, value, height);
                            bump_render(render_version);
                        }
                    }
                    PageAction::SetHeight(id, value) => {
                        let current = {
                            let eng = engine.read();
                            eng.get_pages().into_iter().find(|page| page.0 == id)
                        };
                        if let Some((_, _, _, _, width, _)) = current {
                            engine.write().resize_page(id, width, value);
                            bump_render(render_version);
                        }
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::AlignPanel => {
            AlignPanel {
                relative_to: align_relative_snapshot.clone(),
                on_action: Callback::new(move |action: UiAlignAction| match action {
                    UiAlignAction::SetRelativeTo(relative_to) => {
                        let label = match relative_to {
                            UiAlignRelativeTo::Selection => "Selection",
                            UiAlignRelativeTo::Page => "Page",
                        };
                        align_relative_to.set(label.to_string());
                    }
                    UiAlignAction::Align(op) => {
                        let relative = {
                            let current = align_relative_to.read().clone();
                            if current == "Page" {
                                CoreAlignRelativeTo::Page
                            } else {
                                CoreAlignRelativeTo::Selection
                            }
                        };
                        let alignment = match op {
                            UiAlignOp::Left => CoreAlignment::Left,
                            UiAlignOp::CenterH => CoreAlignment::CenterH,
                            UiAlignOp::Right => CoreAlignment::Right,
                            UiAlignOp::Top => CoreAlignment::Top,
                            UiAlignOp::CenterV => CoreAlignment::CenterV,
                            UiAlignOp::Bottom => CoreAlignment::Bottom,
                        };
                        engine.write().align_selected(alignment, relative);
                        bump_render(render_version);
                    }
                    UiAlignAction::Distribute(op) => {
                        let axis = match op {
                            UiDistributeOp::HorizontalGaps
                            | UiDistributeOp::Left
                            | UiDistributeOp::CenterH
                            | UiDistributeOp::Right => CoreDistributeAxis::Horizontal,
                            UiDistributeOp::VerticalGaps
                            | UiDistributeOp::Top
                            | UiDistributeOp::CenterV
                            | UiDistributeOp::Bottom => CoreDistributeAxis::Vertical,
                        };
                        engine.write().distribute_selected(axis);
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::TransformPanel => {
            TransformPanel {
                x: sel_x,
                y: sel_y,
                width: transform_width_value,
                height: transform_height_value,
                rotation: transform_decomposition.rotation_degrees,
                skew_x: transform_decomposition.skew_x_degrees,
                skew_y: 0.0,
                lock_aspect: transform_lock_aspect_value,
                percent_mode: transform_percent_mode_value,
                has_selection: selection_count > 0,
                on_action: Callback::new(move |action: TransformAction| match action {
                    TransformAction::SetX(value) => {
                        let (current_x, _, _, _) = {
                            let eng = engine.read();
                            selection_bounds_values(&eng)
                        };
                        engine.write().translate_selected(value - current_x, 0.0);
                        bump_render(render_version);
                    }
                    TransformAction::SetY(value) => {
                        let (_, current_y, _, _) = {
                            let eng = engine.read();
                            selection_bounds_values(&eng)
                        };
                        engine.write().translate_selected(0.0, value - current_y);
                        bump_render(render_version);
                    }
                    TransformAction::SetWidth(value) => {
                        let (_, _, current_w, current_h) = {
                            let eng = engine.read();
                            selection_bounds_values(&eng)
                        };
                        if value > 0.0 && current_w > 0.0 {
                            let sx = value / current_w;
                            let sy = if *transform_lock_aspect.read() {
                                sx
                            } else {
                                1.0
                            };
                            if current_h > 0.0 || !*transform_lock_aspect.read() {
                                engine.write().scale_selected(sx, sy);
                                bump_render(render_version);
                            }
                        }
                    }
                    TransformAction::SetHeight(value) => {
                        let (_, _, current_w, current_h) = {
                            let eng = engine.read();
                            selection_bounds_values(&eng)
                        };
                        if value > 0.0 && current_h > 0.0 {
                            let sy = value / current_h;
                            let sx = if *transform_lock_aspect.read() {
                                sy
                            } else {
                                1.0
                            };
                            if current_w > 0.0 || !*transform_lock_aspect.read() {
                                engine.write().scale_selected(sx, sy);
                                bump_render(render_version);
                            }
                        }
                    }
                    TransformAction::SetScalePercent(width_pct, height_pct) => {
                        let lock = *transform_lock_aspect.read();
                        let sx = if width_pct > 0.0 {
                            width_pct / 100.0
                        } else {
                            1.0
                        };
                        let sy = if height_pct > 0.0 {
                            height_pct / 100.0
                        } else if lock && width_pct > 0.0 {
                            sx
                        } else {
                            1.0
                        };
                        engine.write().scale_selected(sx, sy);
                        bump_render(render_version);
                    }
                    TransformAction::ToggleLockAspect => {
                        let next = !*transform_lock_aspect.read();
                        transform_lock_aspect.set(next);
                    }
                    TransformAction::TogglePercentMode => {
                        let next = !*transform_percent_mode.read();
                        transform_percent_mode.set(next);
                    }
                    TransformAction::SetRotation(degrees) => {
                        engine.write().set_selected_rotation(degrees);
                        bump_render(render_version);
                    }
                    TransformAction::SetSkewX(degrees) => {
                        engine.write().set_selected_skew_x(degrees);
                        bump_render(render_version);
                    }
                    TransformAction::SetSkewY(degrees) => {
                        engine.write().skew_selected(0.0, degrees);
                        bump_render(render_version);
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::LpePanel => {
            LpePanel {
                effects: lpe_entries.clone(),
                selected_id: lpe_selected_snapshot.clone(),
                has_selection: has_lpe_target,
                on_action: Callback::new(move |action: UiLpeAction| match action {
                    UiLpeAction::Add(kind) => {
                        let added = engine.write().add_lpe_to_selected_path(kind.key());
                        if let Some(id) = added {
                            lpe_selected_id.set(Some(id));
                        }
                        bump_render(render_version);
                    }
                    UiLpeAction::Remove(id) => {
                        let removed = engine.write().remove_lpe_from_selected_path(&id);
                        if removed {
                            if lpe_selected_id.read().as_deref() == Some(id.as_str()) {
                                lpe_selected_id.set(None);
                            }
                            bump_render(render_version);
                        }
                    }
                    UiLpeAction::MoveUp(id) => {
                        if engine.write().move_lpe_on_selected_path(&id, -1) {
                            lpe_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiLpeAction::MoveDown(id) => {
                        if engine.write().move_lpe_on_selected_path(&id, 1) {
                            lpe_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiLpeAction::ToggleEnabled(id) => {
                        if engine.write().toggle_lpe_on_selected_path(&id) {
                            lpe_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiLpeAction::SetParam(id, name, value) => {
                        if let Ok(number) = value.trim().parse::<f64>() {
                            if engine
                                .write()
                                .set_lpe_param_on_selected_path(&id, &name, number)
                            {
                                lpe_selected_id.set(Some(id));
                                bump_render(render_version);
                            }
                        }
                    }
                    UiLpeAction::Select(id) => {
                        lpe_selected_id.set(Some(id));
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::FilterEditorPanel => {
            FilterEditorPanel {
                primitives: filter_primitives.clone(),
                selected_id: filter_selected_snapshot.clone(),
                on_action: Callback::new(move |action: UiFilterEditorAction| match action {
                    UiFilterEditorAction::AddPrimitive(kind) => {
                        let added = engine.write().add_filter_primitive_to_selected(kind.key());
                        if let Some(id) = added {
                            filter_selected_id.set(Some(id));
                        }
                        bump_render(render_version);
                    }
                    UiFilterEditorAction::RemovePrimitive(id) => {
                        if engine.write().remove_filter_primitive_from_selected(&id) {
                            if filter_selected_id.read().as_deref() == Some(id.as_str()) {
                                filter_selected_id.set(None);
                            }
                            bump_render(render_version);
                        }
                    }
                    UiFilterEditorAction::MoveUp(id) => {
                        if engine.write().move_filter_primitive_on_selected(&id, -1) {
                            filter_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiFilterEditorAction::MoveDown(id) => {
                        if engine.write().move_filter_primitive_on_selected(&id, 1) {
                            filter_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiFilterEditorAction::ToggleEnabled(id) => {
                        if engine.write().toggle_filter_primitive_on_selected(&id) {
                            filter_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiFilterEditorAction::SetParam(id, name, value) => {
                        if engine
                            .write()
                            .set_filter_param_on_selected(&id, &name, value)
                        {
                            filter_selected_id.set(Some(id));
                            bump_render(render_version);
                        }
                    }
                    UiFilterEditorAction::SelectPrimitive(id) => {
                        filter_selected_id.set(Some(id));
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::ResourcesPanel => {
            ResourcesPanel {
                resources: resource_entries.clone(),
                kind_filter: resource_kind_filter_snapshot.clone(),
                selected_id: resource_selected_snapshot.clone(),
                on_action: Callback::new(move |action: UiResourcesAction| match action {
                    UiResourcesAction::Select(id) => {
                        resource_selected_id.set(Some(id));
                    }
                    UiResourcesAction::SetKindFilter(kind) => {
                        resource_kind_filter.set(kind.map(|kind| kind.label().to_string()));
                    }
                    UiResourcesAction::DeleteUnused(id) => {
                        if engine.write().delete_unused_resource(&id) {
                            resource_selected_id.set(None);
                            bump_render(render_version);
                        }
                    }
                    UiResourcesAction::DeleteAllUnused => {
                        let deleted = engine.write().delete_all_unused_resources();
                        if deleted > 0 {
                            resource_selected_id.set(None);
                            bump_render(render_version);
                        }
                    }
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::DocPropertiesPanel => {
            DocPropertiesPanel {
                preset: "Custom".to_string(),
                width: doc_width_mm,
                height: doc_height_mm,
                background_color: doc_background.clone(),
                orientation: if doc_width <= doc_height {
                    "Portrait".to_string()
                } else {
                    "Landscape".to_string()
                },
                title: doc_title.clone(),
                author: doc_author.clone(),
                description: doc_description.clone(),
                grid_enabled: doc_grid_visible,
                grid_spacing_x: grid_spacing,
                grid_spacing_y: grid_spacing,
                snap_to_grid: snap_grid,
                on_action: Callback::new(move |action: DocPropertiesAction| {
                    let mut eng = engine.write();
                    match action {
                        DocPropertiesAction::SetPreset(label) => {
                            let preset = match label.as_str() {
                                "A4" => PageSizePreset::A4,
                                "A3" => PageSizePreset::A3,
                                "Letter" => PageSizePreset::Letter,
                                "Legal" => PageSizePreset::Legal,
                                _ => PageSizePreset::Custom,
                            };
                            if let Some((w, h)) = preset.dimensions_mm() {
                                eng.set_document_size(mm_to_px(w), mm_to_px(h));
                            }
                        }
                        DocPropertiesAction::SetWidth(width_mm) => {
                            eng.set_document_size(mm_to_px(width_mm), doc_height as f64);
                        }
                        DocPropertiesAction::SetHeight(height_mm) => {
                            eng.set_document_size(doc_width as f64, mm_to_px(height_mm));
                        }
                        DocPropertiesAction::SetBackgroundColor(color) => eng.set_page_color(color),
                        DocPropertiesAction::SetOrientation(PageOrientation::Portrait) => {
                            let w = (doc_width as f64).min(doc_height as f64);
                            let h = (doc_width as f64).max(doc_height as f64);
                            eng.set_document_size(w, h);
                        }
                        DocPropertiesAction::SetOrientation(PageOrientation::Landscape) => {
                            let w = (doc_width as f64).max(doc_height as f64);
                            let h = (doc_width as f64).min(doc_height as f64);
                            eng.set_document_size(w, h);
                        }
                        DocPropertiesAction::SetTitle(value) => eng.set_document_title(value),
                        DocPropertiesAction::SetAuthor(value) => eng.set_document_metadata(value),
                        DocPropertiesAction::SetDescription(value) => {
                            eng.set_document_description(value)
                        }
                        DocPropertiesAction::SetGridEnabled(value) => eng.set_grid_visible(value),
                        DocPropertiesAction::SetGridSpacingX(value) => {
                            eng.snapping.prefs.grid_spacing =
                                sanitize_positive(value, 10.0, 0.1, 10_000.0);
                        }
                        DocPropertiesAction::SetGridSpacingY(value) => {
                            eng.snapping.prefs.grid_spacing =
                                sanitize_positive(value, 10.0, 0.1, 10_000.0);
                        }
                        DocPropertiesAction::SetSnapToGrid(value) => {
                            eng.snapping.prefs.snap_to_grid = value
                        }
                    }
                    let updated_defaults = DocumentDefaults::from_engine(&eng);
                    drop(eng);
                    document_defaults.set(updated_defaults);
                    let defaults_snapshot = document_defaults.read().clone();
                    let preferences_snapshot = preferences_state.read().clone();
                    persist_desktop_settings(&preferences_snapshot, &defaults_snapshot);
                    bump_render(render_version);
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::PreferencesDialog => {
            PreferencesDialog {
                state: preferences_snapshot.clone(),
                on_action: Callback::new(move |action: PreferencesAction| {
                    if matches!(action, PreferencesAction::Close) {
                        inspector_panel.set(InspectorPanel::Style);
                        return;
                    }

                    {
                        let mut state = preferences_state.write();
                        update_preferences_state(&mut state, action.clone());
                    }

                    let snapshot = preferences_state.read().clone();
                    let mut eng = engine.write();
                    apply_preferences_to_engine(&mut eng, &snapshot);
                    drop(eng);
                    let defaults_snapshot = document_defaults.read().clone();
                    persist_desktop_settings(&snapshot, &defaults_snapshot);
                    bump_render(render_version);
                }),
            }
            .show_in(scope)?;
        }
        EditorPart::StatusBar => {
            StatusBar {
                selection_type: if selection_count == 1 {
                    selected_object_details
                        .as_ref()
                        .map(|details| details.type_name.clone())
                        .unwrap_or_else(|| "object".into())
                } else {
                    "objects".into()
                },
                cursor_position,
                zoom,
                selection_count,
                object_count,
            }
            .show_in(scope)?;
        }
    }
    Ok(())
}
