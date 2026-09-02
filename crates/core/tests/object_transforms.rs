//! Object-level transform commands must act around the selection's centre.
//!
//! `Transform2D::then(other)` applies `self` first, so centring an operation is
//! written `translate(-c) then op then translate(c)`. Writing it the other way
//! round moves the object to a mirrored position instead of transforming it in
//! place, and no test caught that because nothing asserted the resulting
//! coordinates.

use sivgo_core::selection::node_bounding_box;
use sivgo_core::EditorEngine;
use sivgo_dom::{NodeId, RectAttrs, SvgElement, SvgLength};

/// Build an engine holding a single selected rect at (10,20) sized 30x40.
fn engine_with_rect() -> (EditorEngine, NodeId) {
    let mut engine = EditorEngine::new(200.0, 200.0);
    let root = engine.document.root().expect("document root");

    let attrs = RectAttrs {
        x: SvgLength::px(10.0),
        y: SvgLength::px(20.0),
        width: SvgLength::px(30.0),
        height: SvgLength::px(40.0),
        ..RectAttrs::default()
    };
    let id = engine.document.add_node(SvgElement::Rect(attrs));
    engine.document.append_child(root, id);
    engine.selection.select(id);

    (engine, id)
}

#[track_caller]
fn centre_of(engine: &EditorEngine, id: NodeId) -> (f64, f64) {
    let bb = node_bounding_box(&engine.document, id).expect("bounding box");
    ((bb.x0 + bb.x1) / 2.0, (bb.y0 + bb.y1) / 2.0)
}

#[track_caller]
fn assert_close(actual: (f64, f64), expected: (f64, f64), what: &str) {
    let tol = 1e-6;
    assert!(
        (actual.0 - expected.0).abs() < tol && (actual.1 - expected.1).abs() < tol,
        "{what}: expected centre {expected:?}, got {actual:?}"
    );
}

#[test]
fn horizontal_flip_keeps_the_object_in_place() {
    let (mut engine, id) = engine_with_rect();
    let before = centre_of(&engine, id);

    engine.flip_horizontal();

    assert_close(centre_of(&engine, id), before, "flip_horizontal");
}

#[test]
fn vertical_flip_keeps_the_object_in_place() {
    let (mut engine, id) = engine_with_rect();
    let before = centre_of(&engine, id);

    engine.flip_vertical();

    assert_close(centre_of(&engine, id), before, "flip_vertical");
}

#[test]
fn flipping_twice_returns_to_the_original() {
    let (mut engine, id) = engine_with_rect();
    let before = node_bounding_box(&engine.document, id).expect("bbox");

    engine.flip_horizontal();
    engine.flip_horizontal();

    let after = node_bounding_box(&engine.document, id).expect("bbox");
    for (a, b, name) in [
        (before.x0, after.x0, "x0"),
        (before.y0, after.y0, "y0"),
        (before.x1, after.x1, "x1"),
        (before.y1, after.y1, "y1"),
    ] {
        assert!((a - b).abs() < 1e-6, "{name}: {a} != {b}");
    }
}

#[test]
fn rotation_keeps_the_centre_fixed() {
    let (mut engine, id) = engine_with_rect();
    let before = centre_of(&engine, id);

    engine.rotate_selected(90.0);

    assert_close(centre_of(&engine, id), before, "rotate_selected(90)");
}

#[test]
fn quarter_turns_swap_width_and_height() {
    let (mut engine, id) = engine_with_rect();
    let before = node_bounding_box(&engine.document, id).expect("bbox");

    engine.rotate_selected(90.0);

    let after = node_bounding_box(&engine.document, id).expect("bbox");
    assert!(
        (after.width() - before.height()).abs() < 1e-6,
        "width after a quarter turn should equal the original height"
    );
    assert!(
        (after.height() - before.width()).abs() < 1e-6,
        "height after a quarter turn should equal the original width"
    );
}

#[test]
fn scaling_keeps_the_centre_fixed_and_changes_size() {
    let (mut engine, id) = engine_with_rect();
    let before_box = node_bounding_box(&engine.document, id).expect("bbox");
    let before_centre = centre_of(&engine, id);

    engine.scale_selected(2.0, 3.0);

    assert_close(centre_of(&engine, id), before_centre, "scale_selected");
    let after = node_bounding_box(&engine.document, id).expect("bbox");
    assert!((after.width() - before_box.width() * 2.0).abs() < 1e-6);
    assert!((after.height() - before_box.height() * 3.0).abs() < 1e-6);
}

#[test]
fn skew_keeps_the_centre_fixed() {
    let (mut engine, id) = engine_with_rect();
    let before = centre_of(&engine, id);

    engine.skew_selected(15.0, 0.0);

    assert_close(centre_of(&engine, id), before, "skew_selected");
}

#[test]
fn translate_moves_by_exactly_the_requested_delta() {
    let (mut engine, id) = engine_with_rect();
    let (cx, cy) = centre_of(&engine, id);

    engine.translate_selected(7.0, -3.0);

    assert_close(centre_of(&engine, id), (cx + 7.0, cy - 3.0), "translate");
}

#[test]
fn transforms_are_undoable_back_to_the_original_position() {
    let (mut engine, id) = engine_with_rect();
    let before = centre_of(&engine, id);

    engine.rotate_selected(37.0);
    engine.scale_selected(1.5, 1.5);
    engine.undo();
    engine.undo();

    assert_close(centre_of(&engine, id), before, "after undoing both");
}

/// A rect nested inside a scaled group, plus the group and rect ids.
fn engine_with_nested_rect() -> (EditorEngine, NodeId, NodeId) {
    use sivgo_dom::GroupAttrs;
    use sivgo_math::Transform2D;

    let mut engine = EditorEngine::new(200.0, 200.0);
    let root = engine.document.root().expect("document root");

    let group = engine
        .document
        .add_node(SvgElement::Group(GroupAttrs::default()));
    engine.document.append_child(root, group);
    if let Some(node) = engine.document.get_node_mut(group) {
        node.presentation.transform = Some(Transform2D::scale(2.0));
    }

    let attrs = RectAttrs {
        x: SvgLength::px(0.0),
        y: SvgLength::px(0.0),
        width: SvgLength::px(10.0),
        height: SvgLength::px(10.0),
        ..RectAttrs::default()
    };
    let rect = engine.document.add_node(SvgElement::Rect(attrs));
    engine.document.append_child(group, rect);
    if let Some(node) = engine.document.get_node_mut(rect) {
        node.presentation.transform = Some(Transform2D::translate(10.0, 0.0));
    }

    (engine, group, rect)
}

#[test]
fn ancestor_transforms_compose_from_the_root_down() {
    // The group scales by 2, the rect is offset by 10 inside it, so the rect
    // occupies 20..40 in document space. Composing leaf-first would put it at
    // 10..30 instead.
    let (engine, _group, rect) = engine_with_nested_rect();
    let bb = node_bounding_box(&engine.document, rect).expect("bbox");

    assert!(
        (bb.x0 - 20.0).abs() < 1e-6,
        "left edge: expected 20, got {}",
        bb.x0
    );
    assert!(
        (bb.x1 - 40.0).abs() < 1e-6,
        "right edge: expected 40, got {}",
        bb.x1
    );
}

#[test]
fn hit_testing_follows_the_composed_ancestor_transform() {
    let (mut engine, _group, rect) = engine_with_nested_rect();

    // A point inside the rect's true position selects it.
    engine.on_mouse_down(sivgo_core::input::MouseEvent {
        x: 30.0,
        y: 10.0,
        button: sivgo_core::input::MouseButton::Left,
        modifiers: Default::default(),
    });
    assert!(
        engine.selection.is_selected(rect),
        "clicking inside the transformed rect must select it"
    );
}

#[test]
fn corner_radius_applies_to_every_selected_rectangle() {
    use sivgo_dom::SvgElement;

    let mut engine = EditorEngine::new(200.0, 200.0);
    let root = engine.document.root().expect("root");

    let mut rects = Vec::new();
    for _ in 0..3 {
        let id = engine.document.add_node(SvgElement::Rect(RectAttrs {
            width: SvgLength::px(20.0),
            height: SvgLength::px(20.0),
            ..RectAttrs::default()
        }));
        engine.document.append_child(root, id);
        rects.push(id);
    }
    engine.selection.select(rects[0]);
    for &id in &rects[1..] {
        engine.selection.add(id);
    }

    assert_eq!(engine.set_selected_corner_radius(4.0), 3);
    for &id in &rects {
        match &engine.document.get_node(id).expect("rect").element {
            SvgElement::Rect(attrs) => {
                assert!((attrs.rx.to_px() - 4.0).abs() < 1e-9, "rx should be set");
                assert!((attrs.ry.to_px() - 4.0).abs() < 1e-9, "ry should match rx");
            }
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    // The change is one undoable step.
    engine.undo();
    match &engine.document.get_node(rects[0]).expect("rect").element {
        SvgElement::Rect(attrs) => assert!(attrs.rx.to_px() < 1e-9, "undo restores square corners"),
        other => panic!("expected a rect, got {other:?}"),
    }
}

#[test]
fn corner_radius_ignores_non_rectangles() {
    use sivgo_dom::{CircleAttrs, SvgElement};

    let mut engine = EditorEngine::new(200.0, 200.0);
    let root = engine.document.root().expect("root");
    let circle = engine.document.add_node(SvgElement::Circle(CircleAttrs {
        r: SvgLength::px(5.0),
        ..CircleAttrs::default()
    }));
    engine.document.append_child(root, circle);
    engine.selection.select(circle);

    assert_eq!(engine.set_selected_corner_radius(4.0), 0);
}

#[test]
fn tools_can_be_reached_as_their_concrete_type() {
    use sivgo_core::tool::ToolId;
    use sivgo_core::tools::{spiral::SpiralTool, star::StarTool};

    let mut engine = EditorEngine::new(200.0, 200.0);

    // Without a typed accessor a settings panel cannot change any of these.
    let star = engine
        .tools
        .get_mut::<StarTool>(ToolId::Star)
        .expect("the star tool should be registered");
    star.num_points = 7;
    star.inner_ratio = 0.25;

    let spiral = engine
        .tools
        .get_mut::<SpiralTool>(ToolId::Spiral)
        .expect("the spiral tool should be registered");
    spiral.turns = 4.5;

    assert_eq!(
        engine
            .tools
            .get::<StarTool>(ToolId::Star)
            .unwrap()
            .num_points,
        7
    );
    assert!(
        (engine
            .tools
            .get::<SpiralTool>(ToolId::Spiral)
            .unwrap()
            .turns
            - 4.5)
            .abs()
            < 1e-9
    );

    // Asking for the wrong type yields nothing rather than a wrong answer.
    assert!(engine.tools.get::<StarTool>(ToolId::Spiral).is_none());
}

#[test]
fn importing_a_document_merges_its_top_level_elements() {
    use sivgo_dom::parse_svg;

    let mut engine = EditorEngine::new(200.0, 200.0);
    let before = engine.document.len();

    let source = parse_svg(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\">\
         <rect width=\"4\" height=\"4\"/><circle cx=\"5\" cy=\"5\" r=\"2\"/>\
         <g><rect width=\"1\" height=\"1\"/></g></svg>",
    )
    .expect("parse");

    assert_eq!(
        engine.import_document(&source),
        3,
        "three top-level elements"
    );
    assert!(
        engine.document.len() > before + 3,
        "the group's child should come along too"
    );

    // The whole import is a single undoable step.
    engine.undo();
    assert_eq!(
        engine.document.len(),
        before,
        "undo removes everything added"
    );
}

#[test]
fn importing_an_empty_document_changes_nothing() {
    use sivgo_dom::parse_svg;

    let mut engine = EditorEngine::new(200.0, 200.0);
    let before = engine.document.len();
    let empty = parse_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>")
        .expect("parse");

    assert_eq!(engine.import_document(&empty), 0);
    assert_eq!(engine.document.len(), before);
}

#[test]
fn a_new_document_has_no_unsaved_changes() {
    let engine = EditorEngine::new(200.0, 200.0);
    assert!(!engine.is_dirty());
}

#[test]
fn editing_marks_the_document_dirty_and_saving_clears_it() {
    let (mut engine, _id) = engine_with_rect();
    // Adding the rect went through the document directly, so start from a
    // known state.
    engine.mark_saved();
    assert!(!engine.is_dirty());

    engine.translate_selected(5.0, 0.0);
    assert!(engine.is_dirty(), "an edit should mark the document dirty");

    engine.mark_saved();
    assert!(!engine.is_dirty(), "saving should clear the flag");
}

#[test]
fn undoing_back_to_the_saved_state_reports_no_changes() {
    // A plain boolean flag gets this wrong: it stays set after the edit is
    // undone, so the user is asked about changes that no longer exist.
    let (mut engine, _id) = engine_with_rect();
    engine.mark_saved();

    engine.translate_selected(5.0, 0.0);
    engine.rotate_selected(10.0);
    assert!(engine.is_dirty());

    engine.undo();
    assert!(engine.is_dirty(), "one edit still outstanding");
    engine.undo();
    assert!(!engine.is_dirty(), "back at the saved state");

    // Redoing makes it dirty again.
    engine.redo();
    assert!(engine.is_dirty());
}

#[test]
fn saving_midway_through_a_history_moves_the_save_point() {
    let (mut engine, _id) = engine_with_rect();
    engine.mark_saved();

    engine.translate_selected(5.0, 0.0);
    engine.mark_saved();
    assert!(!engine.is_dirty());

    // Undoing past the save point is a change from what is on disk.
    engine.undo();
    assert!(
        engine.is_dirty(),
        "undoing past a save is an unsaved change"
    );
}
