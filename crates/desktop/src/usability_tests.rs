use super::*;

#[test]
fn metadata_is_not_an_editable_layer_or_object() {
    let mut engine = EditorEngine::new(800.0, 600.0);
    engine.load_document(sivgo_dom::parse_svg(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"><title>Drawing</title><desc>Notes</desc><metadata>Author</metadata><sodipodi:namedview pagecolor="#fff"/><defs><linearGradient id="paint"/></defs><rect id="shape" width="20" height="30"/></svg>"##).unwrap());
    let layers = build_layer_entries(&engine);
    let objects = object_tree_entries(&engine);
    assert_eq!(layers.len(), 1);
    assert_eq!(objects.len(), 1);
    assert_eq!(layers[0].id, objects[0].id);
}

#[test]
fn saving_a_copy_preserves_original_unsaved_changes() {
    let app = glory::App::memory();
    let state = AppState::new(app.scope(), DesktopContext::new());
    let parent = state.engine.peek().document.root().unwrap();
    state.engine.write().execute(Box::new(AddNodeCommand::new(
        parent,
        SvgElement::Rect(Default::default()),
        PresentationAttrs::default(),
        None,
    )));
    assert!(state.engine.peek().is_dirty());
    let path = std::env::temp_dir().join(format!("sivgo-save-copy-{}.svg", NodeId::new()));
    assert!(save_svg_document(state.engine, &path, false));
    assert!(
        state.engine.peek().is_dirty(),
        "export must not mark the original saved"
    );
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("<rect"));
    assert!(save_svg_document(state.engine, &path, true));
    assert!(!state.engine.peek().is_dirty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn inspector_width_reserves_canvas_space_without_losing_user_preference() {
    assert_eq!(effective_inspector_width(1440.0, 520.0), 520.0);
    let width = effective_inspector_width(800.0, 520.0);
    assert!(width <= 428.0);
    assert!(800.0 - LEFT_RAIL_WIDTH - INSPECTOR_RESIZE_HANDLE_WIDTH - width >= 320.0);
    assert_eq!(effective_inspector_width(800.0, 300.0), 300.0);
    assert_eq!(
        effective_inspector_width(800.0, f64::NAN),
        INSPECTOR_WIDTH_DEFAULT
    );
}

#[test]
fn native_escape_cancels_shape_before_pointer_release() {
    let app = glory::App::memory();
    let state = AppState::new(app.scope(), DesktopContext::new());
    state.canvas_bounds.revise(|bounds| {
        *bounds = Some(glory::render::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        })
    });
    state.engine.write().tools.switch_to(ToolId::Rect);
    state.active_tool.set(ToolId::Rect);
    let count = state.engine.peek().document.len();
    let modifiers = keyboard_types::Modifiers::empty();
    handle_native_input(
        state,
        NativeInput::PointerDown {
            x: 50.0,
            y: 50.0,
            button: sivgo_core::MouseButton::Left,
            modifiers,
        },
    );
    handle_native_input(
        state,
        NativeInput::PointerMove {
            x: 150.0,
            y: 150.0,
            button: sivgo_core::MouseButton::Left,
            modifiers,
        },
    );
    handle_native_input(
        state,
        NativeInput::KeyDown {
            key: keyboard_types::Key::Named(keyboard_types::NamedKey::Escape),
            modifiers,
            editable: false,
        },
    );
    handle_native_input(
        state,
        NativeInput::PointerUp {
            x: 150.0,
            y: 150.0,
            button: sivgo_core::MouseButton::Left,
            modifiers,
        },
    );
    assert_eq!(state.engine.peek().document.len(), count);
    assert!(!*state.canvas_pointer_down.peek());
    assert!(state.canvas_drag_preview.peek().is_none());
}

#[test]
fn inspector_colors_follow_svg_defaults_stylesheets_and_inheritance() {
    let mut engine = EditorEngine::new(800.0, 600.0);
    engine.load_document(sivgo_dom::parse_svg(r##"<svg xmlns="http://www.w3.org/2000/svg"><style>.colored { fill: #ff0000; stroke: #00ff00; stroke-width: 3 }</style><rect id="plain" width="10" height="10"/><g class="colored"><rect id="child" width="10" height="10"/><rect id="inline" style="fill: #0000ff" width="10" height="10"/></g></svg>"##).unwrap());
    let plain = engine.document.find_by_xml_id("plain").unwrap();
    engine.selection.select(plain);
    assert_eq!(
        get_selected_fill(&selected_presentation(&engine)),
        ("#000000".into(), false)
    );
    assert_eq!(
        get_selected_stroke(&selected_presentation(&engine)),
        (String::new(), true)
    );
    assert_eq!(
        selected_style_state(&engine, &selected_presentation(&engine)).stroke_paint_type,
        "None"
    );
    let child = engine.document.find_by_xml_id("child").unwrap();
    engine.selection.select(child);
    assert_eq!(
        get_selected_fill(&selected_presentation(&engine)),
        ("#ff0000".into(), false)
    );
    assert_eq!(
        get_selected_stroke(&selected_presentation(&engine)),
        ("#00ff00".into(), false)
    );
    assert_eq!(
        get_selected_stroke_width(&selected_presentation(&engine)),
        3.0
    );
    let inline = engine.document.find_by_xml_id("inline").unwrap();
    engine.selection.select(inline);
    assert_eq!(
        get_selected_fill(&selected_presentation(&engine)),
        ("#0000ff".into(), false)
    );
}

fn find_button(app: &glory::App, label: &str) -> glory::render::NodeId {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        if matches!(node.descriptor.view().props, glory::render::Props::Button(p) if p.label == label)
        {
            return id;
        }
        pending.extend(node.children.iter().copied());
    }
    panic!("button not found: {label}");
}

#[test]
fn toolbar_history_actions_track_available_undo_and_redo() {
    let app = glory::App::memory();
    let state = AppState::new(app.scope(), DesktopContext::new());
    app.mount(editor_app(state)).unwrap();
    app.settle(16).unwrap();
    let enabled = |label| match app
        .tree()
        .node(find_button(&app, label))
        .unwrap()
        .descriptor
        .view()
        .props
    {
        glory::render::Props::Button(p) => p.enabled,
        _ => unreachable!(),
    };
    assert!(!enabled("Undo"));
    assert!(!enabled("Redo"));
    let parent = state.engine.peek().document.root().unwrap();
    state.engine.write().execute(Box::new(AddNodeCommand::new(
        parent,
        SvgElement::Rect(Default::default()),
        PresentationAttrs::default(),
        None,
    )));
    bump_render(state.render_version);
    app.settle(16).unwrap();
    assert!(enabled("Undo"));
    assert!(!enabled("Redo"));
    app.dispatch(find_button(&app, "Undo"), UiEvent::Activate)
        .unwrap();
    app.settle(16).unwrap();
    assert!(!enabled("Undo"));
    assert!(enabled("Redo"));
}

#[test]
fn compact_shell_keeps_canvas_visible_at_smaller_window_sizes() {
    for (width, height) in [(960, 640), (1440, 900)] {
        let app = glory::apace::shell::WindowApp::new();
        app.renderer().set_viewport(width, height, 1.0).unwrap();
        app.renderer().set_css(&format!("{CSS}\n{HOST_CSS}"));
        let state = AppState::new(app.app().scope(), DesktopContext::new());
        state.window_size.set((f64::from(width), f64::from(height)));
        state.layout_settings.write().inspector_width_px = 520.0;
        app.mount(editor_app(state)).unwrap();
        refresh_canvas_bounds(app.app().scope(), state);
        app.app().settle(16).unwrap();
        let bounds = state.canvas_bounds.get().unwrap();
        assert!(bounds.width >= 280.0, "canvas too narrow: {bounds:?}");
        assert!(bounds.height >= 350.0, "canvas too short: {bounds:?}");
        assert!(
            bounds.y < 180.0,
            "toolbar occupies too much height: {bounds:?}"
        );
        assert!(
            bounds.y + bounds.height <= f64::from(height),
            "canvas outside window: {bounds:?}"
        );
    }
}
