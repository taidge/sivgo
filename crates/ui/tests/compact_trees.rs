use glory::prelude::*;
use glory::render::{NodeId, Props};
use sivgo_ui::{
    layers_panel::{LayerAction, LayerEntry, LayersPanel},
    objects_panel::{ObjectAction, ObjectEntry, ObjectsPanel},
    Callback,
};
use std::{cell::RefCell, rc::Rc};

fn nodes(app: &App) -> Vec<NodeId> {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    let mut result = Vec::new();
    while let Some(id) = pending.pop() {
        pending.extend(tree.node(id).unwrap().children.iter().copied());
        result.push(id);
    }
    result
}
fn named(app: &App, name: &str) -> NodeId {
    nodes(app)
        .into_iter()
        .find(|id| {
            app.tree()
                .node(*id)
                .unwrap()
                .descriptor
                .view()
                .name
                .as_deref()
                == Some(name)
        })
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn button_count(app: &App) -> usize {
    nodes(app)
        .into_iter()
        .filter(|id| {
            matches!(
                app.tree().node(*id).unwrap().descriptor.view().props,
                Props::Button(_)
            )
        })
        .count()
}
fn layer(label: &str, depth: usize) -> LayerEntry {
    LayerEntry {
        id: sivgo_dom::NodeId::new(),
        label: label.into(),
        depth,
        selected: false,
        active: false,
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: "normal".into(),
    }
}
#[test]
fn unselected_layers_are_compact_and_dispatch_by_name() {
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let captured = actions.clone();
    let layers = vec![layer("Background", 0), layer("Foreground", 0)];
    let id = layers[1].id;
    app.mount(LayersPanel {
        layers,
        editing_id: None,
        edit_value: String::new(),
        on_action: Callback::new(move |a| captured.borrow_mut().push(a)),
        on_edit_change: Callback::default(),
    })
    .unwrap();
    assert_eq!(
        button_count(&app),
        6,
        "unselected rows must not render per-object inspectors"
    );
    app.dispatch(named(&app, "Select layer Foreground"), Event::Activate)
        .unwrap();
    app.dispatch(named(&app, "Hide layer Foreground"), Event::Activate)
        .unwrap();
    assert!(
        matches!(actions.borrow().as_slice(), [LayerAction::Select(a), LayerAction::ToggleVisibility(b)] if *a == id && *b == id)
    );
}
#[test]
fn collapse_hides_descendants_without_hiding_siblings_and_reopens() {
    let app = App::memory();
    app.mount(LayersPanel {
        layers: vec![layer("Parent", 0), layer("Child", 1), layer("Sibling", 0)],
        editing_id: None,
        edit_value: String::new(),
        on_action: Callback::default(),
        on_edit_change: Callback::default(),
    })
    .unwrap();
    let disclosure = named(&app, "Expand or collapse Parent");
    let row_of = |name| {
        let tree = app.tree();
        tree.node(named(&app, name)).unwrap().parent.unwrap()
    };
    let child = row_of("Select layer Child");
    let sibling = row_of("Select layer Sibling");
    app.dispatch(disclosure, Event::Activate).unwrap();
    app.pump().unwrap();
    assert_eq!(
        app.tree()
            .node(child)
            .unwrap()
            .descriptor
            .view()
            .style
            .display,
        Some(Display::None)
    );
    assert_eq!(
        app.tree()
            .node(sibling)
            .unwrap()
            .descriptor
            .view()
            .style
            .display,
        Some(Display::Flex)
    );
    app.dispatch(disclosure, Event::Activate).unwrap();
    app.pump().unwrap();
    assert_eq!(
        app.tree()
            .node(child)
            .unwrap()
            .descriptor
            .view()
            .style
            .display,
        Some(Display::Flex)
    );
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}
#[test]
fn objects_keep_compact_visibility_and_lock_controls_without_selection() {
    let app = App::memory();
    let id = sivgo_dom::NodeId::new();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let captured = actions.clone();
    app.mount(ObjectsPanel {
        objects: vec![ObjectEntry {
            id,
            label: "Shape".into(),
            type_name: "rect".into(),
            depth: 0,
            visible: true,
            locked: false,
            selected: false,
        }],
        editing_id: None,
        edit_value: String::new(),
        selected_details: None,
        xml_id_edit_value: String::new(),
        on_action: Callback::new(move |a| captured.borrow_mut().push(a)),
        on_edit_change: Callback::default(),
        on_xml_id_change: Callback::default(),
    })
    .unwrap();
    assert_eq!(button_count(&app), 3);
    app.dispatch(named(&app, "Select object Shape"), Event::Activate)
        .unwrap();
    app.dispatch(named(&app, "Lock object Shape"), Event::Activate)
        .unwrap();
    assert!(
        matches!(actions.borrow().as_slice(), [ObjectAction::Select(a), ObjectAction::ToggleLock(b)] if *a==id && *b==id)
    );
}
