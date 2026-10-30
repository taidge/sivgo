use glory::prelude::*;
use glory::render::{NodeId, Props};
use sivgo_ui::doc_properties::DocPropertiesAction;
use sivgo_ui::properties_panel::{PaintType, PropertiesAction};
use sivgo_ui::{Callback, DocPropertiesPanel, PropertiesPanel};
use std::{cell::RefCell, rc::Rc};

fn matching(
    app: &App,
    predicate: impl Fn(&glory::render::DescriptorView<'_>) -> bool,
) -> Vec<NodeId> {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    let mut matches = Vec::new();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        if predicate(&node.descriptor.view()) {
            matches.push(id);
        }
        pending.extend(node.children.iter().copied());
    }
    matches
}

fn button(app: &App, label: &str) -> NodeId {
    matching(
        app,
        |view| matches!(view.props, Props::Button(p) if p.label == label),
    )
    .into_iter()
    .next()
    .unwrap()
}

#[test]
fn compact_paint_selectors_keep_every_fill_and_stroke_choice() {
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    app.mount(PropertiesPanel {
        fill: "#ff0000".into(),
        fill_paint_type: "Flat Color".into(),
        fill_opacity: 1.0,
        stroke: "#000000".into(),
        stroke_paint_type: "None".into(),
        stroke_opacity: 0.5,
        stroke_width: 2.0,
        line_cap: "butt".into(),
        line_join: "miter".into(),
        dash_pattern: String::new(),
        dash_offset: 0.0,
        marker_start: String::new(),
        marker_mid: String::new(),
        marker_end: String::new(),
        has_selection: true,
        on_action: Callback::new(move |action| observed.borrow_mut().push(action)),
    })
    .unwrap();
    assert!(matching(
        &app,
        |view| matches!(view.props, Props::Button(p) if p.label == "Linear Gradient")
    )
    .is_empty());
    for (label, is_fill) in [("Choose Fill paint", true), ("Choose Stroke paint", false)] {
        for paint in PaintType::all() {
            let trigger = matching(&app, |view| view.name.as_deref() == Some(label))[0];
            app.dispatch(trigger, Event::Activate).unwrap();
            app.pump().unwrap();
            for option in PaintType::all() {
                button(&app, option.label());
            }
            app.dispatch(button(&app, paint.label()), Event::Activate)
                .unwrap();
            app.pump().unwrap();
            let actions = actions.borrow();
            let selected = match actions.last().unwrap() {
                PropertiesAction::SetFillPaintType(value) if is_fill => value,
                PropertiesAction::SetStrokePaintType(value) if !is_fill => value,
                other => panic!("wrong paint action: {other:?}"),
            };
            assert_eq!(selected, paint);
            assert!(matching(
                &app,
                |view| matches!(view.props, Props::Button(p) if p.label == "Linear Gradient")
            )
            .is_empty());
        }
    }
    for label in [
        "Fill color",
        "Stroke color",
        "Fill opacity",
        "Stroke opacity",
        "Dash pattern",
    ] {
        assert!(
            !matching(
                &app,
                |view| matches!(view.props, Props::Text(p) if p.content == label)
            )
            .is_empty(),
            "missing visible {label}"
        );
        assert!(
            !matching(&app, |view| view.name.as_deref() == Some(label)
                && matches!(view.props, Props::Input(_)))
            .is_empty(),
            "missing named input {label}"
        );
    }
}

#[test]
fn document_fields_preserve_validation_and_multiline_metadata() {
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    app.mount(DocPropertiesPanel {
        preset: "Custom".into(),
        width: 100.0,
        height: 80.0,
        background_color: "#ffffff".into(),
        orientation: "Landscape".into(),
        title: String::new(),
        author: String::new(),
        description: "First\nSecond".into(),
        grid_enabled: true,
        grid_spacing_x: 10.0,
        grid_spacing_y: 20.0,
        snap_to_grid: false,
        on_action: Callback::new(move |action| observed.borrow_mut().push(action)),
    })
    .unwrap();
    for label in ["Background color", "Description"] {
        assert!(!matching(
            &app,
            |view| matches!(view.props, Props::Text(p) if p.content == label)
        )
        .is_empty());
    }
    let width = matching(&app, |view| view.name.as_deref() == Some("Width (mm)"))[0];
    let commit_value = |id, value: &str| {
        let revision = match app.tree().node(id).unwrap().descriptor.view().props {
            Props::Input(p) => p.revision,
            _ => panic!("expected input"),
        };
        app.dispatch(
            id,
            Event::Commit {
                value: value.into(),
                revision,
            },
        )
        .unwrap();
    };
    commit_value(width, "-5");
    assert!(actions.borrow().is_empty());
    commit_value(width, "210.5");
    assert!(
        matches!(actions.borrow().last(), Some(DocPropertiesAction::SetWidth(value)) if *value == 210.5)
    );
    let description = matching(&app, |view| view.name.as_deref() == Some("Description"))[0];
    commit_value(description, "Updated\nDescription");
    assert!(
        matches!(actions.borrow().last(), Some(DocPropertiesAction::SetDescription(value)) if value == "Updated\nDescription")
    );
}
