//! Exercise real semantic controls and their application callbacks without a window.
use glory::prelude::*;
use glory::render::{NodeId, Props};
use sivgo_core::ToolId;
use sivgo_ui::transform_panel::TransformAction;
use sivgo_ui::{Callback, Toolbar, TransformPanel};
use std::{cell::RefCell, rc::Rc};

struct BoundsRenderer {
    memory: glory::render::memory::Memory,
    inputs: Vec<glory::render::Input>,
    applied: u64,
    bounds: glory::render::Rect,
}
impl glory::render::Render for BoundsRenderer {
    fn capabilities(&self) -> glory::render::Capabilities {
        self.memory.capabilities()
    }
    fn style_profile(&self) -> glory::style::Profile {
        glory::style::Profile::RECORD
    }
    fn apply(
        &mut self,
        mutations: &[glory::render::Mutation],
    ) -> std::result::Result<glory::render::Receipt, glory::render::Error> {
        let receipt = self.memory.apply(mutations)?;
        self.applied = receipt.applied;
        Ok(receipt)
    }
    fn request(
        &mut self,
        query: glory::render::Query,
        token: glory::render::QueryToken,
    ) -> std::result::Result<(), glory::render::Error> {
        if matches!(query, glory::render::Query::Bounds(_)) {
            self.inputs.push(glory::render::Input::Response {
                token,
                applied: self.applied,
                result: Ok(glory::render::QueryValue::Bounds(self.bounds)),
            });
            Ok(())
        } else {
            self.memory.request(query, token)
        }
    }
    fn drain(&mut self, inputs: &mut Vec<glory::render::Input>) {
        self.memory.drain(inputs);
        inputs.append(&mut self.inputs);
    }
    fn needs_observe(&self) -> bool {
        !self.inputs.is_empty() || self.memory.needs_observe()
    }
}

fn find(app: &App, predicate: impl Fn(&Props) -> bool) -> NodeId {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        if predicate(node.descriptor.view().props) {
            return id;
        }
        pending.extend(node.children.iter().copied());
    }
    panic!("semantic control missing");
}

#[test]
fn toolbar_activation_dispatches_the_selected_tool_and_cleans_up() {
    let app = App::memory();
    let selected = Rc::new(RefCell::new(None));
    let result = selected.clone();
    app.mount(Toolbar {
        active_tool: ToolId::Select,
        on_tool_change: Callback::new(move |tool| *result.borrow_mut() = Some(tool)),
    })
    .unwrap();
    let button = {
        let tree = app.tree();
        let mut pending = tree.roots().to_vec();
        let mut found = None;
        while let Some(id) = pending.pop() {
            let node = tree.node(id).unwrap();
            if node.descriptor.view().name.as_deref() == Some("Rectangle (R)") {
                found = Some(id);
                break;
            }
            pending.extend(node.children.iter().copied());
        }
        found.expect("rectangle tool")
    };
    assert!(app.dispatch(button, Event::Activate).unwrap());
    assert_eq!(*selected.borrow(), Some(ToolId::Rect));
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn numeric_commit_preserves_fractional_values_and_rejects_invalid_text() {
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let result = actions.clone();
    app.mount(TransformPanel {
        x: 12.0,
        y: 24.0,
        width: 40.0,
        height: 80.0,
        rotation: 0.0,
        skew_x: 0.0,
        skew_y: 0.0,
        lock_aspect: false,
        percent_mode: false,
        has_selection: true,
        on_action: Callback::new(move |action| result.borrow_mut().push(action)),
    })
    .unwrap();
    let input = find(
        &app,
        |props| matches!(props, Props::Input(p) if p.value == "12.0" || p.value == "12"),
    );
    let revision = match app.tree().node(input).unwrap().descriptor.view().props {
        Props::Input(p) => p.revision,
        _ => unreachable!(),
    };
    assert!(app
        .dispatch(
            input,
            Event::Commit {
                value: "-7.25".into(),
                revision
            }
        )
        .unwrap());
    assert!(matches!(actions.borrow().last(), Some(TransformAction::SetX(v)) if *v == -7.25));
    let accepted = actions.borrow().len();
    let revision = match app.tree().node(input).unwrap().descriptor.view().props {
        Props::Input(p) => p.revision,
        _ => unreachable!(),
    };
    app.dispatch(
        input,
        Event::Commit {
            value: "invalid".into(),
            revision,
        },
    )
    .unwrap();
    assert_eq!(actions.borrow().len(), accepted);
}

#[test]
fn scrollbar_uses_its_viewport_origin_and_commits_on_release() {
    let app = App::new(|runtime| BoundsRenderer {
        memory: glory::render::memory::Memory::new(runtime),
        inputs: Vec::new(),
        applied: 0,
        bounds: glory::render::Rect {
            x: 300.0,
            y: 50.0,
            width: 100.0,
            height: 14.0,
        },
    });
    let pan = Rc::new(RefCell::new(0.0));
    let observed = pan.clone();
    app.mount(sivgo_ui::HorizontalScrollbar {
        track_length: 100.0,
        document_width: 200.0,
        pan_x: 0.0,
        zoom: 1.0,
        on_scroll: Callback::new(move |value| *observed.borrow_mut() = value),
    })
    .unwrap();
    let node = app.tree().roots()[0];
    app.dispatch(
        node,
        Event::Drag {
            from: node,
            phase: glory::render::DragPhase::Start,
            x: 375.0,
            y: 55.0,
        },
    )
    .unwrap();
    app.pump().unwrap();
    assert_eq!(
        *pan.borrow(),
        0.0,
        "drag preview must not retire the captured component"
    );
    app.dispatch(
        node,
        Event::Drag {
            from: node,
            phase: glory::render::DragPhase::Cancel,
            x: 0.0,
            y: 5.0,
        },
    )
    .unwrap();
    app.pump().unwrap();
    assert_eq!(*pan.borrow(), 0.0);
    app.dispatch(
        node,
        Event::Drag {
            from: node,
            phase: glory::render::DragPhase::Drop,
            x: 360.0,
            y: 55.0,
        },
    )
    .unwrap();
    app.pump().unwrap();
    assert_eq!(
        *pan.borrow(),
        -70.0,
        "360 viewport pixels means 60 pixels within this track"
    );
}

#[test]
fn ruler_drop_reports_viewport_axis_without_stale_cursor_offsets() {
    let app = App::memory();
    let result = Rc::new(RefCell::new(None));
    let observed = result.clone();
    app.mount(sivgo_ui::HorizontalRuler {
        width: 400.0,
        scroll_x: 25.0,
        zoom: 2.0,
        cursor_position: app.scope().cage((999.0, 888.0)),
        on_guide_drag: Callback::new(move |value| *observed.borrow_mut() = Some(value)),
    })
    .unwrap();
    let node = app.tree().roots()[0];
    app.dispatch(
        node,
        Event::Drag {
            from: node,
            phase: glory::render::DragPhase::Drop,
            x: 100.0,
            y: 240.0,
        },
    )
    .unwrap();
    assert_eq!(*result.borrow(), Some(240.0));
}

#[test]
fn document_description_keeps_multiline_content_editable() {
    let app = App::memory();
    app.mount(sivgo_ui::DocPropertiesPanel {
        preset: "Custom".into(),
        width: 100.0,
        height: 80.0,
        background_color: "#ffffff".into(),
        orientation: "Landscape".into(),
        title: String::new(),
        author: String::new(),
        description: "First line\nSecond line".into(),
        grid_enabled: false,
        grid_spacing_x: 10.0,
        grid_spacing_y: 10.0,
        snap_to_grid: false,
        on_action: Callback::default(),
    })
    .unwrap();
    find(
        &app,
        |props| matches!(props, Props::Input(p) if p.multiline && p.value == "First line\nSecond line"),
    );
}

#[test]
fn object_drag_commits_reparenting_only_after_release_and_hit_testing() {
    use sivgo_ui::objects_panel::{ObjectAction, ObjectEntry, ObjectsPanel};
    let app = App::new(|runtime| BoundsRenderer {
        memory: glory::render::memory::Memory::new(runtime),
        inputs: Vec::new(),
        applied: 0,
        bounds: glory::render::Rect {
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 40.0,
        },
    });
    let first = sivgo_dom::NodeId::new();
    let second = sivgo_dom::NodeId::new();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    app.mount(ObjectsPanel {
        objects: [(first, "Source"), (second, "Target")]
            .into_iter()
            .map(|(id, label)| ObjectEntry {
                id,
                label: label.into(),
                type_name: "g".into(),
                depth: 0,
                visible: true,
                locked: false,
                selected: false,
            })
            .collect(),
        editing_id: None,
        edit_value: String::new(),
        selected_details: None,
        xml_id_edit_value: String::new(),
        on_action: Callback::new(move |action| observed.borrow_mut().push(action)),
        on_edit_change: Callback::default(),
        on_xml_id_change: Callback::default(),
    })
    .unwrap();
    let source = {
        let tree = app.tree();
        let mut pending = tree.roots().to_vec();
        let mut found = None;
        while let Some(id) = pending.pop() {
            let node = tree.node(id).unwrap();
            if node.descriptor.view().name.as_deref() == Some("Move object Source") {
                found = Some(id);
                break;
            }
            pending.extend(node.children.iter().copied());
        }
        found.unwrap()
    };
    app.dispatch(
        source,
        Event::Drag {
            from: source,
            phase: glory::render::DragPhase::Start,
            x: 125.0,
            y: 65.0,
        },
    )
    .unwrap();
    assert!(actions.borrow().is_empty());
    app.dispatch(
        source,
        Event::Drag {
            from: source,
            phase: glory::render::DragPhase::Drop,
            x: 125.0,
            y: 65.0,
        },
    )
    .unwrap();
    app.pump().unwrap();
    assert!(
        matches!(actions.borrow().as_slice(), [ObjectAction::BeginDrag(id), ObjectAction::DropOn(target), ObjectAction::EndDrag] if *id == first && *target == second)
    );
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn page_template_choices_expand_select_and_keep_the_selected_payload() {
    use sivgo_ui::pages_panel::{PageAction, PageTemplate, PagesPanel};
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let observed = actions.clone();
    app.mount(PagesPanel {
        pages: Vec::new(),
        on_action: Callback::new(move |action| observed.borrow_mut().push(action)),
    })
    .unwrap();
    let trigger = find(
        &app,
        |props| matches!(props, Props::Button(p) if p.label == "Page template: Document"),
    );
    app.dispatch(trigger, Event::Activate).unwrap();
    for template in PageTemplate::all() {
        find(
            &app,
            |props| matches!(props, Props::Button(p) if p.label == template.label()),
        );
    }
    let choice = find(
        &app,
        |props| matches!(props, Props::Button(p) if p.label == "A4 Landscape"),
    );
    app.dispatch(choice, Event::Activate).unwrap();
    find(
        &app,
        |props| matches!(props, Props::Button(p) if p.label == "Page template: A4 Landscape"),
    );
    let add = find(
        &app,
        |props| matches!(props, Props::Button(p) if p.label == "Add template page"),
    );
    app.dispatch(add, Event::Activate).unwrap();
    assert!(matches!(
        actions.borrow().as_slice(),
        [PageAction::AddTemplate(PageTemplate::A4Landscape)]
    ));
}

#[test]
fn compact_color_bar_keeps_fill_stroke_and_no_paint_actions() {
    let app = App::memory();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let fill = actions.clone();
    let stroke = actions.clone();
    let none = actions.clone();
    app.mount(sivgo_ui::ColorBar {
        fill_color: "#ff0000".into(),
        stroke_color: "#0000ff".into(),
        fill_none: false,
        stroke_none: false,
        on_fill_click: Callback::new(move |_| fill.borrow_mut().push("fill")),
        on_stroke_click: Callback::new(move |_| stroke.borrow_mut().push("stroke")),
        on_set_none: Callback::new(move |_| none.borrow_mut().push("none")),
    })
    .unwrap();
    for name in ["Fill", "Stroke"] {
        let button = {
            let tree = app.tree();
            let mut pending = tree.roots().to_vec();
            let mut found = None;
            while let Some(id) = pending.pop() {
                let node = tree.node(id).unwrap();
                if node.descriptor.view().name.as_deref() == Some(name) {
                    found = Some(id);
                    break;
                }
                pending.extend(node.children.iter().copied());
            }
            found.expect("interactive color swatch")
        };
        assert!(app.dispatch(button, Event::Activate).unwrap());
    }
    let none = find(
        &app,
        |props| matches!(props, Props::Button(button) if button.label == "No paint"),
    );
    assert!(app.dispatch(none, Event::Activate).unwrap());
    assert_eq!(&*actions.borrow(), &["fill", "stroke", "none"]);
}

#[test]
fn color_picker_commits_final_document_color_when_closed() {
    use sivgo_ui::doc_properties::DocPropertiesAction;
    let app = App::memory();
    let colors = Rc::new(RefCell::new(Vec::new()));
    let observed = colors.clone();
    app.mount(sivgo_ui::DocPropertiesPanel {
        preset: "Custom".into(),
        width: 100.0,
        height: 80.0,
        background_color: "#ff0000".into(),
        orientation: "Landscape".into(),
        title: String::new(),
        author: String::new(),
        description: String::new(),
        grid_enabled: false,
        grid_spacing_x: 10.0,
        grid_spacing_y: 10.0,
        snap_to_grid: false,
        on_action: Callback::new(move |action| {
            if let DocPropertiesAction::SetBackgroundColor(color) = action {
                observed.borrow_mut().push(color);
            }
        }),
    })
    .unwrap();
    let trigger = find(
        &app,
        |props| matches!(props, Props::Button(button) if button.label == "Choose color"),
    );
    app.dispatch(trigger, Event::Activate).unwrap();
    app.pump().unwrap();
    let (hue, popup) = {
        let tree = app.tree();
        let mut pending = tree.roots().to_vec();
        let mut hue = None;
        let mut popup = None;
        while let Some(id) = pending.pop() {
            let node = tree.node(id).unwrap();
            let view = node.descriptor.view();
            if view.name.as_deref() == Some("Hue") {
                hue = Some(id);
            }
            if view
                .semantics
                .as_ref()
                .is_some_and(|semantics| semantics.layer.is_some())
            {
                popup = Some(id);
            }
            pending.extend(node.children.iter().copied());
        }
        (
            hue.expect("actual picker hue slider"),
            popup.expect("open picker popup"),
        )
    };
    app.dispatch(
        hue,
        Event::Range {
            from: hue,
            command: glory::render::RangeCommand::Set(120.0),
        },
    )
    .unwrap();
    app.pump().unwrap();
    assert!(
        colors.borrow().is_empty(),
        "preview must not rebuild and dismiss its popup"
    );
    assert!(app.tree().node(popup).is_ok());
    app.dispatch(popup, Event::Dismiss(glory::render::DismissReason::Escape))
        .unwrap();
    app.pump().unwrap();
    assert_eq!(&*colors.borrow(), &["#00ff00ff"]);
    assert_eq!(
        app.tree()
            .node(trigger)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|value| value.expanded),
        Some(false)
    );
    let field = find(
        &app,
        |props| matches!(props, Props::Input(input) if input.value == "#00ff00ff"),
    );
    let revision = match app.tree().node(field).unwrap().descriptor.view().props {
        Props::Input(input) => input.revision,
        _ => unreachable!(),
    };
    app.dispatch(
        field,
        Event::Commit {
            value: "invalid".into(),
            revision,
        },
    )
    .unwrap();
    assert_eq!(
        colors.borrow().len(),
        1,
        "invalid text must not reach the document"
    );
    let revision = match app.tree().node(field).unwrap().descriptor.view().props {
        Props::Input(input) => input.revision,
        _ => unreachable!(),
    };
    app.dispatch(
        field,
        Event::Commit {
            value: "#0000ff".into(),
            revision,
        },
    )
    .unwrap();
    assert_eq!(&*colors.borrow(), &["#00ff00ff", "#0000ff"]);
}
