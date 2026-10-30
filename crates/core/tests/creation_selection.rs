use sivgo_core::{EditorEngine, Modifiers, MouseButton, MouseEvent, ToolId};
use sivgo_dom::{NodeId, RectAttrs, SvgElement};

fn pointer(x: f64, y: f64) -> MouseEvent {
    MouseEvent {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
    }
}

fn draw(engine: &mut EditorEngine, tool: ToolId) {
    engine.tools.switch_to(tool);
    engine.on_mouse_down(pointer(80.0, 90.0));
    engine.on_mouse_move(pointer(180.0, 190.0));
    engine.on_mouse_up(pointer(180.0, 190.0));
}

fn old_object(engine: &mut EditorEngine) -> NodeId {
    let id = engine
        .document
        .add_node(SvgElement::Rect(RectAttrs::default()));
    engine
        .document
        .append_child(engine.document.root().unwrap(), id);
    engine.selection.select(id);
    id
}

#[test]
fn drawing_replaces_selection_with_the_created_shape_and_redo_restores_it() {
    for tool in [
        ToolId::Rect,
        ToolId::Ellipse,
        ToolId::Star,
        ToolId::Spiral,
        ToolId::Box3d,
    ] {
        let mut engine = EditorEngine::new(600.0, 400.0);
        let old = old_object(&mut engine);
        draw(&mut engine, tool);
        assert_eq!(engine.selection.len(), 1, "{tool:?}");
        let created = engine.selection.primary().unwrap();
        assert_ne!(created, old, "{tool:?} must select its new object");
        if tool == ToolId::Box3d {
            assert!(matches!(
                engine.document.get_node(created).unwrap().element,
                SvgElement::Box3d(_)
            ));
            assert_eq!(engine.document.children(created).len(), 6);
        }
        engine.undo();
        assert!(engine.document.get_node(created).is_none());
        assert!(
            engine.selection.is_empty(),
            "undo must remove stale node IDs"
        );
        engine.selection.select(old);
        engine.redo();
        assert_eq!(
            engine.selection.selected(),
            &[created],
            "redo must replace the old selection"
        );
    }
}

#[test]
fn created_shape_selection_survives_active_layer_routing() {
    let mut engine = EditorEngine::new(600.0, 400.0);
    let layer = engine.add_layer("Drawing").unwrap();
    engine.set_active_layer(layer);
    draw(&mut engine, ToolId::Rect);
    let created = engine
        .selection
        .primary()
        .expect("created rectangle selected");
    assert_eq!(engine.document.parent(created), Some(layer));
    engine.undo();
    engine.redo();
    assert_eq!(engine.selection.selected(), &[created]);
    assert_eq!(engine.document.parent(created), Some(layer));
}

#[test]
fn a_click_without_a_shape_and_a_property_edit_keep_the_existing_selection() {
    let mut engine = EditorEngine::new(600.0, 400.0);
    draw(&mut engine, ToolId::Rect);
    let selected = engine.selection.primary().unwrap();
    engine.on_mouse_down(pointer(300.0, 300.0));
    engine.on_mouse_up(pointer(300.0, 300.0));
    assert_eq!(engine.selection.selected(), &[selected]);
    engine.translate_selected(10.0, 0.0);
    engine.undo();
    engine.redo();
    assert_eq!(engine.selection.selected(), &[selected]);
}

#[test]
fn a_new_history_branch_at_the_saved_depth_stays_dirty() {
    let mut engine = EditorEngine::new(600.0, 400.0);
    draw(&mut engine, ToolId::Rect);
    engine.translate_selected(10.0, 0.0);
    engine.mark_saved();
    engine.undo();
    engine.translate_selected(0.0, 20.0);
    assert!(
        engine.is_dirty(),
        "a different edit cannot reuse the discarded save point"
    );
    engine.undo();
    engine.redo();
    assert!(engine.is_dirty());
    engine.mark_saved();
    assert!(!engine.is_dirty());
}

#[test]
fn spraying_keeps_the_source_selection_during_the_gesture() {
    let mut engine = EditorEngine::new(600.0, 400.0);
    draw(&mut engine, ToolId::Rect);
    let source = engine.selection.primary().unwrap();
    let before = engine.document.len();
    engine.tools.switch_to(ToolId::Spray);
    engine.on_mouse_down(pointer(300.0, 300.0));
    engine.on_mouse_move(pointer(310.0, 300.0));
    engine.on_mouse_move(pointer(320.0, 300.0));
    engine.on_mouse_up(pointer(320.0, 300.0));
    assert!(engine.document.len() > before);
    assert_eq!(engine.selection.selected(), &[source]);
}

#[test]
fn cancelling_a_drag_prevents_mouse_up_from_creating_an_object() {
    for tool in [
        ToolId::Rect,
        ToolId::Ellipse,
        ToolId::Star,
        ToolId::Spiral,
        ToolId::Box3d,
        ToolId::Text,
        ToolId::Pencil,
        ToolId::Calligraphy,
        ToolId::Pages,
    ] {
        let mut engine = EditorEngine::new(600.0, 400.0);
        let old = old_object(&mut engine);
        let before = engine.document.len();
        engine.tools.switch_to(tool);
        engine.on_mouse_down(pointer(80.0, 90.0));
        engine.on_mouse_move(pointer(180.0, 190.0));
        engine.cancel_tool_interaction();
        engine.on_mouse_up(pointer(180.0, 190.0));
        assert_eq!(
            engine.document.len(),
            before,
            "{tool:?} should discard the pending gesture"
        );
        assert_eq!(engine.tools.active_id(), tool);
        assert_eq!(engine.selection.selected(), &[old]);
        assert!(!engine.history.can_undo());
    }
}
