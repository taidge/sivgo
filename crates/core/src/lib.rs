//! `sivgo-core` -- Editor engine core for the Sivgo SVG editor.
//!
//! Provides:
//! - **Command system** with undo/redo ([`command`], [`commands`])
//! - **Selection management** ([`selection`])
//! - **Tool system** with pluggable tools ([`tool`], [`tools`])
//! - **Viewport** pan/zoom and coordinate conversion ([`viewport`])
//! - **Snapping engine** ([`snap`])
//! - **Event bus** for notifying the UI ([`event`])
//! - **EditorEngine** facade tying everything together ([`engine`])

pub mod bbox_cache;
pub mod clone_tiler;
pub mod command;
pub mod commands;
pub mod engine;
pub mod event;
pub mod input;
pub mod lpe;
pub mod selection;
pub mod shortcuts;
pub mod snap;
pub mod tool;
pub mod tools;
pub mod viewport;

// Re-export primary types at crate root.
pub use bbox_cache::BBoxCache;
pub use clone_tiler::{generate_tile_placements, SymmetryGroup, TileParams, TilePlacement};
pub use command::{Command, CommandHistory};
pub use commands::{
    AddNodeCommand, AddPageCommand, ClipMaskMode, CompoundCommand, GroupCommand, MergeLayerCommand,
    ModifyAttributeCommand, ModifyElementCommand, ModifyObjectLabelCommand, ModifyPageCommand,
    ModifyPathCommand, ModifyXmlIdCommand, ReleaseClipMaskCommand, RemoveNodeCommand,
    ReorderCommand, ReorderDirection, ReparentNodeCommand, RouteToActiveLayerCommand,
    SetClipMaskCommand, SetLockCommand, SetVisibilityCommand, TransformCommand, UngroupCommand,
    UnlinkCloneCommand,
};
pub use engine::{
    AlignRelativeTo, Alignment, DistributeAxis, EditorEngine, EngineFilterParam,
    EngineFilterPrimitiveEntry, EngineLpeEntry, EngineLpeParam, EngineResourceEntry,
    EngineTextState, TransformDecomposition,
};
pub use event::{EditorEvent, EventBus};
pub use input::{Key, KeyEvent, Modifiers, MouseButton, MouseEvent};
pub use selection::SelectionSet;
pub use shortcuts::{Action, Shortcut, ShortcutManager};
pub use snap::{
    SnapContext, SnapGuide, SnapIndicator, SnapPreferences, SnapResult, SnapSource, SnapSourceKind,
    SnapSourcePreferences, SnapTarget, SnappingEngine, SourcedSnapResult,
};
pub use tool::{Tool, ToolId, ToolManager, ToolResponse};
pub use tools::{
    Box3dTool, BucketFillTool, CalligraphyTool, ConnectorTool, ConnectorType, DropperTool,
    EllipseTool, EraserTool, MarkerTool, MeasureTool, MeshTool, NodeTool, PagesTool, PenTool,
    RectTool, SelectTool, SpiralTool, SprayMode, SprayTool, TweakMode, TweakTool, ZoomTool,
};
pub use viewport::{DisplayMode, Viewport};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};
    use sivgo_dom::{
        DefsAttrs, FilterAttrs, FlowParaAttrs, GroupAttrs, LineAttrs, NodeId, PathAttrs,
        PresentationAttrs, RectAttrs, SvgDocument, SvgElement, SvgSvgAttrs, TextAttrs,
        TextPathAttrs,
    };
    use sivgo_math::{PathSegment, Point, Transform2D, Vector};

    /// Helper: create a minimal document with a root <svg>.
    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::new(800.0, LengthUnit::None)),
            height: Some(SvgLength::new(600.0, LengthUnit::None)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);
        doc
    }

    /// Helper: add a rect to a document under its root.
    fn add_rect(doc: &mut SvgDocument, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        let root = doc.root().unwrap();
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(x, LengthUnit::None),
            y: SvgLength::new(y, LengthUnit::None),
            width: SvgLength::new(w, LengthUnit::None),
            height: SvgLength::new(h, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(root, rect_id);
        rect_id
    }

    fn add_line(doc: &mut SvgDocument, x1: f64, y1: f64, x2: f64, y2: f64) -> NodeId {
        let root = doc.root().unwrap();
        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::new(x1, LengthUnit::None),
            y1: SvgLength::new(y1, LengthUnit::None),
            x2: SvgLength::new(x2, LengthUnit::None),
            y2: SvgLength::new(y2, LengthUnit::None),
        }));
        doc.append_child(root, line_id);
        line_id
    }

    // --- Command execute/undo/redo cycle ---

    #[test]
    fn test_add_node_command_execute_undo_redo() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        assert_eq!(doc.children(root).len(), 0);

        let element = SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(20.0, LengthUnit::None),
            width: SvgLength::new(100.0, LengthUnit::None),
            height: SvgLength::new(50.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        });
        let pres = PresentationAttrs::default();
        let mut cmd = AddNodeCommand::new(root, element, pres, None);

        // Execute
        cmd.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 1);
        let created_id = cmd.created_id().unwrap();
        assert!(doc.get_node(created_id).is_some());

        // Undo
        cmd.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 0);
        assert!(doc.get_node(created_id).is_none());

        // Redo
        cmd.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 1);
        assert!(doc.get_node(created_id).is_some());
    }

    #[test]
    fn test_remove_node_command() {
        let mut doc = make_doc();
        let rect_id = add_rect(&mut doc, 10.0, 20.0, 100.0, 50.0);
        let root = doc.root().unwrap();
        assert_eq!(doc.children(root).len(), 1);

        let mut cmd = RemoveNodeCommand::new(rect_id);
        cmd.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 0);
        assert!(doc.get_node(rect_id).is_none());

        cmd.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 1);
        assert!(doc.get_node(rect_id).is_some());
    }

    #[test]
    fn test_transform_command() {
        let mut doc = make_doc();
        let rect_id = add_rect(&mut doc, 0.0, 0.0, 50.0, 50.0);

        let old_t = None;
        let new_t = Some(Transform2D::translate(100.0, 200.0));
        let mut cmd = TransformCommand::new(rect_id, old_t, new_t);

        cmd.execute(&mut doc);
        let node = doc.get_node(rect_id).unwrap();
        assert!(node.presentation.transform.is_some());
        let coeffs = node.presentation.transform.unwrap().as_coeffs();
        assert!((coeffs[4] - 100.0).abs() < 1e-10);
        assert!((coeffs[5] - 200.0).abs() < 1e-10);

        cmd.undo(&mut doc);
        let node = doc.get_node(rect_id).unwrap();
        assert!(node.presentation.transform.is_none());
    }

    #[test]
    fn test_engine_transform_decomposition_and_absolute_rotation() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        if let Some(node) = engine.document.get_node_mut(rect_id) {
            node.presentation.transform = Some(Transform2D::rotate(30.0_f64.to_radians()));
        }
        engine.selection.select(rect_id);

        let before = engine.primary_transform_decomposition().unwrap();
        assert!((before.rotation_degrees - 30.0).abs() < 1e-9);

        engine.set_selected_rotation(45.0);

        let after = engine.primary_transform_decomposition().unwrap();
        assert!((after.rotation_degrees - 45.0).abs() < 1e-9);

        engine.undo();
        let undone = engine.primary_transform_decomposition().unwrap();
        assert!((undone.rotation_degrees - 30.0).abs() < 1e-9);
    }

    // --- CommandHistory with multiple operations ---

    #[test]
    fn test_command_history_multiple_ops() {
        let mut doc = make_doc();
        let mut history = CommandHistory::new(100);
        let root = doc.root().unwrap();

        // Add two rects via history.
        let cmd1 = Box::new(AddNodeCommand::new(
            root,
            SvgElement::Rect(RectAttrs::default()),
            PresentationAttrs::default(),
            None,
        ));
        let cmd2 = Box::new(AddNodeCommand::new(
            root,
            SvgElement::Rect(RectAttrs::default()),
            PresentationAttrs::default(),
            None,
        ));

        history.execute(cmd1, &mut doc);
        assert_eq!(doc.children(root).len(), 1);
        history.execute(cmd2, &mut doc);
        assert_eq!(doc.children(root).len(), 2);

        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Undo both.
        history.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 1);
        history.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 0);

        assert!(!history.can_undo());
        assert!(history.can_redo());

        // Redo one.
        history.redo(&mut doc);
        assert_eq!(doc.children(root).len(), 1);
        assert!(history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn test_history_max_depth() {
        let mut doc = make_doc();
        let mut history = CommandHistory::new(3);
        let root = doc.root().unwrap();

        for _ in 0..5 {
            let cmd = Box::new(AddNodeCommand::new(
                root,
                SvgElement::Rect(RectAttrs::default()),
                PresentationAttrs::default(),
                None,
            ));
            history.execute(cmd, &mut doc);
        }

        // Should only be able to undo 3 times (max_depth).
        let mut count = 0;
        while history.undo(&mut doc) {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_history_redo_cleared_after_new_command() {
        let mut doc = make_doc();
        let mut history = CommandHistory::new(100);
        let root = doc.root().unwrap();

        let cmd1 = Box::new(AddNodeCommand::new(
            root,
            SvgElement::Rect(RectAttrs::default()),
            PresentationAttrs::default(),
            None,
        ));
        history.execute(cmd1, &mut doc);
        history.undo(&mut doc);
        assert!(history.can_redo());

        // A new command should clear the redo stack.
        let cmd2 = Box::new(AddNodeCommand::new(
            root,
            SvgElement::Rect(RectAttrs::default()),
            PresentationAttrs::default(),
            None,
        ));
        history.execute(cmd2, &mut doc);
        assert!(!history.can_redo());
    }

    // --- SelectionSet add/remove/toggle ---

    #[test]
    fn test_selection_set_basic() {
        let mut sel = SelectionSet::new();
        assert!(sel.is_empty());

        let id1 = NodeId::new();
        let id2 = NodeId::new();

        sel.select(id1);
        assert_eq!(sel.len(), 1);
        assert!(sel.is_selected(id1));
        assert!(!sel.is_selected(id2));
        assert_eq!(sel.primary(), Some(id1));

        sel.add(id2);
        assert_eq!(sel.len(), 2);
        assert!(sel.is_selected(id2));

        sel.remove(id1);
        assert_eq!(sel.len(), 1);
        assert!(!sel.is_selected(id1));

        sel.toggle(id2); // removes id2
        assert!(sel.is_empty());

        sel.toggle(id1); // adds id1
        assert_eq!(sel.len(), 1);
        assert!(sel.is_selected(id1));

        sel.clear();
        assert!(sel.is_empty());
    }

    #[test]
    fn test_selection_select_replaces() {
        let mut sel = SelectionSet::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        sel.add(id1);
        sel.add(id2);
        assert_eq!(sel.len(), 2);

        // select() should replace the entire selection.
        sel.select(id1);
        assert_eq!(sel.len(), 1);
        assert!(sel.is_selected(id1));
        assert!(!sel.is_selected(id2));
    }

    // --- Viewport coordinate conversion ---

    #[test]
    fn test_viewport_roundtrip() {
        let mut vp = Viewport::new(1024.0, 768.0);
        vp.zoom = 1.5;
        vp.pan = Vector::new(100.0, 50.0);

        let screen = Point::new(400.0, 300.0);
        let doc = vp.screen_to_document(screen);
        let back = vp.document_to_screen(doc);
        assert!((back.x - screen.x).abs() < 1e-10);
        assert!((back.y - screen.y).abs() < 1e-10);
    }

    #[test]
    fn test_viewport_zoom_at() {
        let mut vp = Viewport::new(800.0, 600.0);
        let center = Point::new(200.0, 150.0);
        let doc_before = vp.screen_to_document(center);
        vp.zoom_at(center, 3.0);
        let doc_after = vp.screen_to_document(center);
        assert!((doc_before.x - doc_after.x).abs() < 1e-10);
        assert!((doc_before.y - doc_after.y).abs() < 1e-10);
    }

    // --- SelectTool click-to-select ---

    #[test]
    fn test_select_tool_click_to_select() {
        let mut doc = make_doc();
        let rect_id = add_rect(&mut doc, 10.0, 10.0, 80.0, 80.0);

        let mut tool = SelectTool::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        // Click inside the rect.
        let down = MouseEvent::left(50.0, 50.0);
        let up = MouseEvent::left(50.0, 50.0);

        tool.on_mouse_down(down, &doc, &sel, &vp);
        let _resp = tool.on_mouse_up(up, &doc, &sel, &vp);

        // Verify hit test finds the rect.
        let hit =
            crate::tools::select::hit_test_at(&doc, vp.screen_to_document(Point::new(50.0, 50.0)));
        assert_eq!(hit, Some(rect_id));
    }

    #[test]
    fn test_select_tool_miss() {
        let mut doc = make_doc();
        let _rect_id = add_rect(&mut doc, 10.0, 10.0, 80.0, 80.0);
        let vp = Viewport::new(800.0, 600.0);

        // Click outside the rect.
        let hit = crate::tools::select::hit_test_at(
            &doc,
            vp.screen_to_document(Point::new(200.0, 200.0)),
        );
        assert!(hit.is_none());
    }

    // --- RectTool creates a rect ---

    #[test]
    fn test_rect_tool_creates_rect() {
        let doc = make_doc();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);
        let mut tool = RectTool::new();

        // Mouse down at (100, 100), mouse up at (200, 200).
        tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        let resp = tool.on_mouse_up(MouseEvent::left(200.0, 200.0), &doc, &sel, &vp);

        match resp {
            ToolResponse::ExecuteCommand(mut cmd) => {
                // Execute on the doc.
                let mut doc = doc;
                cmd.execute(&mut doc);
                let root = doc.root().unwrap();
                let children = doc.children(root);
                assert_eq!(children.len(), 1);
                let node = doc.get_node(children[0]).unwrap();
                assert_eq!(node.element.tag_name(), "rect");
                match &node.element {
                    SvgElement::Rect(a) => {
                        assert!((a.x.value - 100.0).abs() < 1e-10);
                        assert!((a.y.value - 100.0).abs() < 1e-10);
                        assert!((a.width.value - 100.0).abs() < 1e-10);
                        assert!((a.height.value - 100.0).abs() < 1e-10);
                    }
                    _ => panic!("Expected Rect"),
                }
            }
            _ => panic!("Expected ExecuteCommand response from RectTool"),
        }
    }

    // --- EditorEngine integration ---

    #[test]
    fn test_engine_create_rect_select_delete_undo() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        // Switch to rect tool.
        engine.tools.switch_to(ToolId::Rect);
        assert_eq!(engine.tools.active_id(), ToolId::Rect);

        // Draw a rect: down at (100,100), up at (200,200).
        engine.on_mouse_down(MouseEvent::left(100.0, 100.0));
        engine.on_mouse_up(MouseEvent::left(200.0, 200.0));

        // Should now have one child under root.
        let root = engine.document.root().unwrap();
        assert_eq!(engine.document.children(root).len(), 1);
        let rect_id = engine.document.children(root)[0];

        // Switch to select tool and select the rect.
        engine.tools.switch_to(ToolId::Select);
        engine.selection.select(rect_id);

        assert!(engine.selection.is_selected(rect_id));
        assert_eq!(engine.selection.len(), 1);

        // Delete it.
        engine.delete_selected();
        assert_eq!(engine.document.children(root).len(), 0);
        assert!(engine.selection.is_empty());

        // Undo the delete.
        engine.undo();
        assert_eq!(engine.document.children(root).len(), 1);

        // Undo the creation.
        engine.undo();
        assert_eq!(engine.document.children(root).len(), 0);

        // Redo creation.
        engine.redo();
        assert_eq!(engine.document.children(root).len(), 1);

        // Redo deletion.
        engine.redo();
        assert_eq!(engine.document.children(root).len(), 0);
    }

    #[test]
    fn test_engine_select_all() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        // Add a few rects directly (bypassing tools).
        let r1 = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        let r2 = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, r1);
        engine.document.append_child(root, r2);

        engine.select_all();
        assert_eq!(engine.selection.len(), 2);
    }

    #[test]
    fn test_engine_select_all_uses_active_layer_scope() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let layer_a = engine.add_layer("A").unwrap();
        let layer_b = engine.add_layer("B").unwrap();
        let rect_a = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        let rect_b = add_rect(&mut engine.document, 20.0, 0.0, 10.0, 10.0);
        engine.document.detach_node(rect_a);
        engine.document.append_child(layer_a, rect_a);
        engine.document.detach_node(rect_b);
        engine.document.append_child(layer_b, rect_b);

        engine.set_active_layer(layer_a);
        engine.select_all();

        assert_eq!(engine.selection.selected(), &[rect_a]);

        engine.select_all_in_all_layers();
        assert!(engine.selection.is_selected(rect_a));
        assert!(engine.selection.is_selected(rect_b));
        assert!(!engine.selection.is_selected(layer_a));
        assert!(!engine.selection.is_selected(layer_b));
    }

    #[test]
    fn test_engine_select_all_skips_defs_resources() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let visible = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        let defs = engine.document.add_node(SvgElement::Defs(DefsAttrs));
        let resource_rect = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, defs);
        engine.document.append_child(defs, resource_rect);

        engine.select_all_in_all_layers();

        assert!(engine.selection.is_selected(visible));
        assert!(!engine.selection.is_selected(defs));
        assert!(!engine.selection.is_selected(resource_rect));
    }

    #[test]
    fn test_engine_move_selection_to_active_layer_is_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let source = engine.add_layer("Source").unwrap();
        let target = engine.add_layer("Target").unwrap();
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        engine.document.detach_node(rect_id);
        engine.document.append_child(source, rect_id);

        engine.set_active_layer(target);
        engine.selection.select(rect_id);

        assert!(engine.move_selection_to_active_layer());
        assert_eq!(engine.document.parent(rect_id), Some(target));
        assert_eq!(engine.document.children(target), &[rect_id]);

        engine.undo();
        assert_eq!(engine.document.parent(rect_id), Some(source));
        assert_eq!(engine.document.children(source), &[rect_id]);
        assert_eq!(engine.document.parent(source), Some(root));
        assert_eq!(engine.document.parent(target), Some(root));

        engine.redo();
        assert_eq!(engine.document.parent(rect_id), Some(target));
    }

    #[test]
    fn test_engine_duplicate_selected() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let r1 = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, r1);
        engine.selection.select(r1);

        engine.duplicate_selected();
        assert_eq!(engine.document.children(root).len(), 2);
    }

    #[test]
    fn test_engine_reparent_node_for_object_drop_is_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let group_id = engine
            .document
            .add_node(SvgElement::Group(GroupAttrs::default()));
        let rect_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, group_id);
        engine.document.append_child(root, rect_id);

        assert!(engine.reparent_node_for_object_drop(rect_id, group_id));
        assert_eq!(engine.document.parent(rect_id), Some(group_id));
        assert_eq!(engine.document.children(group_id), &[rect_id]);

        engine.undo();
        assert_eq!(engine.document.parent(rect_id), Some(root));
        assert_eq!(engine.document.children(root), &[group_id, rect_id]);
    }

    #[test]
    fn test_engine_reparent_rejects_cycles() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let outer_id = engine
            .document
            .add_node(SvgElement::Group(GroupAttrs::default()));
        let inner_id = engine
            .document
            .add_node(SvgElement::Group(GroupAttrs::default()));
        engine.document.append_child(root, outer_id);
        engine.document.append_child(outer_id, inner_id);

        assert!(!engine.reparent_node(outer_id, inner_id, None));
        assert_eq!(engine.document.parent(outer_id), Some(root));
        assert_eq!(engine.document.parent(inner_id), Some(outer_id));
    }

    #[test]
    fn test_engine_create_clone_assigns_source_id_and_undoes() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let rect_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, rect_id);
        engine.selection.select(rect_id);

        engine.clone_selected();

        let children = engine.document.children(root);
        assert_eq!(children.len(), 2);
        let source = engine.document.get_node(rect_id).unwrap();
        let source_xml_id = source
            .xml_id
            .as_deref()
            .expect("clone source should receive an SVG id");
        let clone_node = engine.document.get_node(children[1]).unwrap();
        match &clone_node.element {
            SvgElement::Use(attrs) => {
                let expected_href = format!("#{source_xml_id}");
                assert_eq!(attrs.href.as_deref(), Some(expected_href.as_str()));
            }
            other => panic!("expected use clone, got {}", other.tag_name()),
        }

        engine.undo();
        assert_eq!(engine.document.children(root), &[rect_id]);
        assert_eq!(engine.document.get_node(rect_id).unwrap().xml_id, None);
    }

    #[test]
    fn test_engine_unlink_clone_replaces_use_with_independent_subtree() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let group_id = engine
            .document
            .add_node(SvgElement::Group(GroupAttrs::default()));
        let rect_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, group_id);
        engine.document.append_child(group_id, rect_id);
        engine.selection.select(group_id);
        engine.clone_selected();

        let clone_id = engine.document.children(root)[1];
        engine.selection.select(clone_id);
        engine.unlink_selected_clones();

        let unlinked = engine.document.get_node(clone_id).unwrap();
        assert!(matches!(unlinked.element, SvgElement::Group(_)));
        assert_eq!(unlinked.children.len(), 1);
        let child_id = unlinked.children[0];
        assert_eq!(engine.document.parent(child_id), Some(clone_id));
        assert!(matches!(
            engine.document.get_node(child_id).unwrap().element,
            SvgElement::Rect(_)
        ));

        engine.undo();
        let restored = engine.document.get_node(clone_id).unwrap();
        assert!(matches!(restored.element, SvgElement::Use(_)));
        assert!(restored.children.is_empty());
    }

    #[test]
    fn test_engine_set_clip_moves_source_to_defs_and_undoes() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let target_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        let source_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, target_id);
        engine.document.append_child(root, source_id);
        engine.selection.select(target_id);
        engine.selection.add(source_id);

        engine.set_clip_selected();

        let target = engine.document.get_node(target_id).unwrap();
        let clip_ref = target
            .presentation
            .clip_path
            .as_deref()
            .expect("target should reference a clip path");
        assert!(clip_ref.starts_with("url(#clip_"));
        assert!(engine.selection.is_selected(target_id));
        assert!(!engine.selection.is_selected(source_id));

        let defs_id = engine.document.children(root)[0];
        assert!(matches!(
            engine.document.get_node(defs_id).unwrap().element,
            SvgElement::Defs(_)
        ));
        let clip_id = engine.document.children(defs_id)[0];
        assert!(matches!(
            engine.document.get_node(clip_id).unwrap().element,
            SvgElement::ClipPath(_)
        ));
        assert_eq!(engine.document.children(clip_id), &[source_id]);

        engine.undo();
        assert_eq!(engine.document.children(root), &[target_id, source_id]);
        assert!(engine
            .document
            .get_node(target_id)
            .unwrap()
            .presentation
            .clip_path
            .is_none());
    }

    #[test]
    fn test_engine_release_clip_restores_unused_resource_contents() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let target_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        let source_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, target_id);
        engine.document.append_child(root, source_id);
        engine.selection.select(target_id);
        engine.selection.add(source_id);
        engine.set_clip_selected();

        engine.selection.select(target_id);
        engine.release_clip_selected();

        assert!(engine
            .document
            .get_node(target_id)
            .unwrap()
            .presentation
            .clip_path
            .is_none());
        assert_eq!(engine.document.parent(source_id), Some(root));

        engine.undo();
        let target = engine.document.get_node(target_id).unwrap();
        assert!(target.presentation.clip_path.is_some());
        let clip_id = engine
            .document
            .find_by_xml_id(
                target
                    .presentation
                    .clip_path
                    .as_deref()
                    .and_then(|value| value.strip_prefix("url(#"))
                    .and_then(|value| value.strip_suffix(')'))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(engine.document.parent(source_id), Some(clip_id));
    }

    #[test]
    fn test_engine_set_mask_sets_mask_reference() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let target_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        let source_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, target_id);
        engine.document.append_child(root, source_id);
        engine.selection.select(target_id);
        engine.selection.add(source_id);

        engine.set_mask_selected();

        let target = engine.document.get_node(target_id).unwrap();
        assert!(target
            .presentation
            .mask
            .as_deref()
            .unwrap()
            .starts_with("url(#mask_"));
    }

    #[test]
    fn test_engine_clipboard_svg_text_combines_selection() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let r1 = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        let r2 = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, r1);
        engine.document.append_child(root, r2);

        engine.select_all();
        engine.copy();

        let svg = engine
            .clipboard_svg_text()
            .expect("copied selection should serialize");
        let parsed = sivgo_dom::parse_svg(&svg).expect("clipboard text should be parseable SVG");
        let parsed_root = parsed.root().unwrap();
        assert_eq!(parsed.children(parsed_root).len(), 2);

        engine.set_clipboard_svg_text(svg);
        engine.paste_in_place();
        assert_eq!(engine.document.children(root).len(), 4);
    }

    #[test]
    fn test_engine_offsets_selected_path() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(60.0, 10.0));
        path.line_to(Point::new(60.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path.clone(),
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.offset_selected_paths(5.0);

        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert!(attrs.d.bounding_box().width() > path.bounding_box().width());

        engine.undo();
        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.d, path);
    }

    #[test]
    fn test_engine_dynamic_offset_preserves_original_path_metadata() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(60.0, 10.0));
        path.line_to(Point::new(60.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path.clone(),
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.dynamic_offset_selected_paths(5.0);

        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert!(attrs
            .inkscape_path_effect
            .as_deref()
            .unwrap()
            .contains("dynamic-offset"));
        assert_eq!(attrs.inkscape_original_d.as_ref(), Some(&path));
        assert!(attrs.d.bounding_box().width() > path.bounding_box().width());

        engine.undo();
        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.d, path);
        assert!(attrs.inkscape_path_effect.is_none());
        assert!(attrs.inkscape_original_d.is_none());
    }

    #[test]
    fn test_engine_linked_offset_adds_linked_path_next_to_source() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(60.0, 10.0));
        path.line_to(Point::new(60.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path.clone(),
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        engine.document.append_child(root, path_id);
        engine.document.get_node_mut(path_id).unwrap().xml_id = Some("source-path".to_string());
        engine.selection.select(path_id);

        engine.linked_offset_selected_paths(5.0);

        let children = engine.document.children(root);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], path_id);
        let linked_id = children[1];
        assert!(engine.selection.is_selected(linked_id));

        let SvgElement::Path(source_attrs) = &engine.document.get_node(path_id).unwrap().element
        else {
            panic!("expected source path");
        };
        assert_eq!(source_attrs.d, path);
        assert!(source_attrs.inkscape_path_effect.is_none());

        let SvgElement::Path(linked_attrs) = &engine.document.get_node(linked_id).unwrap().element
        else {
            panic!("expected linked path");
        };
        let effect = linked_attrs.inkscape_path_effect.as_deref().unwrap();
        assert!(effect.contains("linked-offset"));
        assert!(effect.contains("source-path"));
        assert_eq!(linked_attrs.inkscape_original_d.as_ref(), Some(&path));
        assert!(linked_attrs.d.bounding_box().width() > path.bounding_box().width());

        engine.undo();
        assert_eq!(engine.document.children(root), &[path_id]);
    }

    #[test]
    fn test_engine_filter_editor_adds_undoable_filter_definition() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let rect_id = engine.document.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(20.0, LengthUnit::None),
            width: SvgLength::new(30.0, LengthUnit::None),
            height: SvgLength::new(40.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        engine.document.append_child(root, rect_id);
        engine.selection.select(rect_id);

        let primitive_id = engine
            .add_filter_primitive_to_selected("gaussian_blur")
            .expect("filter primitive should be added");
        let primitives = engine.selected_filter_primitives();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].id, primitive_id);
        assert_eq!(primitives[0].kind, "gaussian_blur");

        let rect = engine.document.get_node(rect_id).unwrap();
        let filter_ref = rect.presentation.filter.as_deref().unwrap();
        let filter_id = filter_ref
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let filter_node_id = engine.document.find_by_xml_id(filter_id).unwrap();
        assert!(matches!(
            engine.document.get_node(filter_node_id).unwrap().element,
            SvgElement::Filter(_)
        ));
        assert_eq!(engine.document.children(filter_node_id).len(), 1);

        assert!(engine.set_filter_param_on_selected(&primitive_id, "stdDeviationX", "8".into()));
        let primitives = engine.selected_filter_primitives();
        assert_eq!(
            primitives[0]
                .params
                .iter()
                .find(|param| param.name == "stdDeviationX")
                .unwrap()
                .value,
            "8"
        );

        engine.undo();
        let primitives = engine.selected_filter_primitives();
        assert_eq!(
            primitives[0]
                .params
                .iter()
                .find(|param| param.name == "stdDeviationX")
                .unwrap()
                .value,
            "3"
        );

        engine.undo();
        assert!(engine.selected_filter_primitives().is_empty());
        assert!(engine
            .document
            .get_node(rect_id)
            .unwrap()
            .presentation
            .filter
            .is_none());
    }

    #[test]
    fn test_engine_filter_editor_reorder_toggle_and_preset() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let rect_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, rect_id);
        engine.selection.select(rect_id);

        let blur_id = engine
            .add_filter_primitive_to_selected("gaussian_blur")
            .unwrap();
        let offset_id = engine.add_filter_primitive_to_selected("offset").unwrap();
        assert_eq!(engine.selected_filter_primitives()[0].id, blur_id);

        assert!(engine.move_filter_primitive_on_selected(&offset_id, -1));
        let primitives = engine.selected_filter_primitives();
        assert_eq!(primitives[0].id, offset_id);
        assert_eq!(primitives[1].id, blur_id);

        assert!(engine.toggle_filter_primitive_on_selected(&offset_id));
        assert!(!engine.selected_filter_primitives()[0].enabled);

        assert!(engine.apply_filter_preset_to_selected("drop_shadow"));
        let primitives = engine.selected_filter_primitives();
        assert_eq!(primitives.len(), 5);
        assert_eq!(primitives[0].kind, "gaussian_blur");
        assert_eq!(primitives[4].kind, "merge");
    }

    #[test]
    fn test_engine_document_resources_count_and_delete_unused() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let defs_id = engine.document.add_node(SvgElement::Defs(DefsAttrs));
        engine.document.append_child(root, defs_id);

        let used_filter = engine
            .document
            .add_node(SvgElement::Filter(FilterAttrs::default()));
        engine.document.get_node_mut(used_filter).unwrap().xml_id = Some("used-filter".into());
        engine.document.append_child(defs_id, used_filter);

        let unused_filter = engine
            .document
            .add_node(SvgElement::Filter(FilterAttrs::default()));
        engine.document.get_node_mut(unused_filter).unwrap().xml_id = Some("unused-filter".into());
        engine.document.append_child(defs_id, unused_filter);

        let rect_id = engine
            .document
            .add_node(SvgElement::Rect(RectAttrs::default()));
        engine.document.append_child(root, rect_id);
        engine
            .document
            .get_node_mut(rect_id)
            .unwrap()
            .presentation
            .filter = Some("url(#used-filter)".into());

        let resources = engine.document_resources();
        let used = resources
            .iter()
            .find(|entry| entry.id == "used-filter")
            .unwrap();
        let unused = resources
            .iter()
            .find(|entry| entry.id == "unused-filter")
            .unwrap();
        assert_eq!(used.kind, "filter");
        assert_eq!(used.usage_count, 1);
        assert_eq!(unused.usage_count, 0);

        assert!(!engine.delete_unused_resource("used-filter"));
        assert!(engine.delete_unused_resource("unused-filter"));
        assert!(engine.document.find_by_xml_id("unused-filter").is_none());

        engine.undo();
        assert!(engine.document.find_by_xml_id("unused-filter").is_some());
        assert_eq!(engine.delete_all_unused_resources(), 1);
        assert!(engine.document.find_by_xml_id("unused-filter").is_none());
    }

    #[test]
    fn test_engine_lpe_offset_is_editable_and_non_destructive() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(60.0, 10.0));
        path.line_to(Point::new(60.0, 60.0));
        path.line_to(Point::new(10.0, 60.0));
        path.close();

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path.clone(),
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        let effect_id = engine
            .add_lpe_to_selected_path("offset")
            .expect("offset LPE should be added");
        let entries = engine.selected_lpe_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "offset");
        assert_eq!(entries[0].params[0].name, "offset");

        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.inkscape_original_d.as_ref(), Some(&path));
        let first_width = attrs.d.bounding_box().width();
        assert!(first_width > path.bounding_box().width());

        assert!(engine.set_lpe_param_on_selected_path(&effect_id, "offset", 12.0));
        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.inkscape_original_d.as_ref(), Some(&path));
        assert!(attrs.d.bounding_box().width() > first_width);

        assert!(engine.toggle_lpe_on_selected_path(&effect_id));
        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.d, path);
        assert_eq!(attrs.inkscape_original_d.as_ref(), Some(&path));

        assert!(engine.remove_lpe_from_selected_path(&effect_id));
        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        assert_eq!(attrs.d, path);
        assert!(attrs.inkscape_path_effect.is_none());
        assert!(attrs.inkscape_original_d.is_none());
    }

    #[test]
    fn test_engine_lpe_stack_reorder_changes_serialized_order() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();

        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(100.0, 100.0));
        path.close();

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        let offset_id = engine.add_lpe_to_selected_path("offset").unwrap();
        let simplify_id = engine.add_lpe_to_selected_path("simplify").unwrap();
        assert_eq!(engine.selected_lpe_entries()[0].id, offset_id);

        assert!(engine.move_lpe_on_selected_path(&simplify_id, -1));
        let entries = engine.selected_lpe_entries();
        assert_eq!(entries[0].id, simplify_id);
        assert_eq!(entries[1].id, offset_id);
    }

    #[test]
    fn test_engine_object_to_path_converts_shape_in_place() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let rect_id = engine.document.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(20.0, LengthUnit::None),
            width: SvgLength::new(30.0, LengthUnit::None),
            height: SvgLength::new(40.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        engine.document.append_child(root, rect_id);
        engine.selection.select(rect_id);

        engine.object_to_path_selected();

        let SvgElement::Path(attrs) = &engine.document.get_node(rect_id).unwrap().element else {
            panic!("expected rect to become path");
        };
        assert!(!attrs.d.is_empty());

        engine.undo();
        assert!(matches!(
            engine.document.get_node(rect_id).unwrap().element,
            SvgElement::Rect(_)
        ));
    }

    #[test]
    fn test_engine_text_to_path_converts_text_in_place() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let text_id = engine.document.add_node(SvgElement::Text(TextAttrs {
            x: vec![SvgLength::new(10.0, LengthUnit::None)],
            y: vec![SvgLength::new(30.0, LengthUnit::None)],
            font_size: Some(SvgLength::new(18.0, LengthUnit::None)),
            text_content: "Sivgo".to_string(),
            ..Default::default()
        }));
        engine.document.append_child(root, text_id);
        engine.selection.select(text_id);

        engine.text_to_path_selected();

        let node = engine.document.get_node(text_id).unwrap();
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected text to become path");
        };
        assert!(!attrs.d.is_empty());
        assert!(node.presentation.font_size.is_none());

        engine.undo();
        assert!(matches!(
            engine.document.get_node(text_id).unwrap().element,
            SvgElement::Text(_)
        ));
    }

    #[test]
    fn test_engine_selected_text_state_and_edits_are_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let text_id = engine.document.add_node(SvgElement::Text(TextAttrs {
            font_size: Some(SvgLength::new(18.0, LengthUnit::None)),
            text_content: "Hello".to_string(),
            ..Default::default()
        }));
        engine.document.append_child(root, text_id);
        engine.selection.select(text_id);

        let initial = engine.selected_text_state().unwrap();
        assert_eq!(initial.content, "Hello");
        assert_eq!(initial.font_size, 18.0);

        assert!(engine.set_selected_text_content("Edited".to_string()));
        assert_eq!(engine.selected_text_state().unwrap().content, "Edited");
        engine.undo();
        assert_eq!(engine.selected_text_state().unwrap().content, "Hello");
        engine.redo();
        assert_eq!(engine.selected_text_state().unwrap().content, "Edited");

        assert!(engine.set_selected_text_font_family("Inter".to_string()));
        assert!(engine.set_selected_text_font_size(24.0));
        assert!(engine.set_selected_text_font_weight("Bold"));
        assert!(engine.set_selected_text_font_style("Italic"));
        assert!(engine.set_selected_text_letter_spacing(1.5));
        assert!(engine.set_selected_text_word_spacing(3.0));
        assert!(engine.set_selected_text_anchor("Middle"));
        assert!(engine.set_selected_text_line_height(1.6));

        let state = engine.selected_text_state().unwrap();
        assert_eq!(state.font_family, "Inter");
        assert_eq!(state.font_size, 24.0);
        assert_eq!(state.font_weight, "Bold");
        assert_eq!(state.font_style, "Italic");
        assert_eq!(state.letter_spacing, 1.5);
        assert_eq!(state.word_spacing, 3.0);
        assert_eq!(state.text_anchor, "Middle");
        assert_eq!(state.line_height, 1.6);

        let node = engine.document.get_node(text_id).unwrap();
        assert_eq!(node.presentation.font_family.as_deref(), Some("Inter"));
        assert!(matches!(
            node.presentation.font_weight,
            Some(sivgo_dom::FontWeight::Bold)
        ));
        assert!(matches!(
            node.presentation.font_style,
            Some(sivgo_dom::FontStyle::Italic)
        ));
        assert!(matches!(
            node.presentation.text_anchor,
            Some(sivgo_dom::TextAnchor::Middle)
        ));
        assert_eq!(
            node.attributes.get("line-height").map(String::as_str),
            Some("1.6")
        );
    }

    #[test]
    fn test_engine_selected_text_state_covers_text_path_and_flowed_text() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let text_path_id = engine
            .document
            .add_node(SvgElement::TextPath(TextPathAttrs {
                text_content: "Along path".to_string(),
                ..Default::default()
            }));
        let flow_para_id = engine
            .document
            .add_node(SvgElement::FlowPara(FlowParaAttrs {
                text_content: "Flowed".to_string(),
            }));
        engine.document.append_child(root, text_path_id);
        engine.document.append_child(root, flow_para_id);

        engine.selection.select(text_path_id);
        assert_eq!(engine.selected_text_state().unwrap().content, "Along path");
        assert!(engine.set_selected_text_content("Edited path".to_string()));
        assert_eq!(engine.selected_text_state().unwrap().content, "Edited path");

        engine.selection.select(flow_para_id);
        assert_eq!(engine.selected_text_state().unwrap().content, "Flowed");
        assert!(engine.set_selected_text_content("Edited flow".to_string()));
        assert_eq!(engine.selected_text_state().unwrap().content, "Edited flow");
    }

    #[test]
    fn test_engine_editor_interaction_workflow_smoke() {
        let mut engine = EditorEngine::new(200.0, 160.0);

        engine.tools.switch_to(ToolId::Rect);
        engine.on_mouse_down(MouseEvent::left(20.0, 20.0));
        engine.on_mouse_move(MouseEvent::left(90.0, 70.0));
        engine.on_mouse_up(MouseEvent::left(90.0, 70.0));

        let rect_id = engine
            .document
            .nodes()
            .find_map(|(&id, node)| matches!(node.element, SvgElement::Rect(_)).then_some(id))
            .expect("rect tool should create a rectangle");

        engine.tools.switch_to(ToolId::Select);
        engine.on_mouse_down(MouseEvent::left(30.0, 30.0));
        assert_eq!(engine.selection.primary(), Some(rect_id));

        let old_attrs = engine
            .document
            .get_node(rect_id)
            .unwrap()
            .presentation
            .clone();
        let mut new_attrs = old_attrs.clone();
        new_attrs.fill = Some(Paint::Color(Color::RED));
        engine.execute(Box::new(ModifyAttributeCommand::new(
            rect_id, old_attrs, new_attrs,
        )));
        assert!(matches!(
            engine.document.get_node(rect_id).unwrap().presentation.fill,
            Some(Paint::Color(Color::RED))
        ));

        let layer_id = engine.add_layer("Interaction Layer").unwrap();
        engine.rename_layer(layer_id, "Renamed Layer");
        engine.set_active_layer(layer_id);
        assert_eq!(engine.get_layers()[0].1, "Renamed Layer");
        assert_eq!(engine.active_layer, Some(layer_id));

        engine.toggle_lock(rect_id);
        assert!(engine.document.get_node(rect_id).unwrap().locked);
        engine.toggle_visibility(rect_id);
        assert!(engine.document.get_node(rect_id).unwrap().hidden);

        let center = Point::new(100.0, 80.0);
        let before = engine.viewport.screen_to_document(center);
        engine.viewport.zoom_at(center, 2.0);
        let after = engine.viewport.screen_to_document(center);
        assert!((before.x - after.x).abs() < 1e-9);
        assert!((before.y - after.y).abs() < 1e-9);

        let pan_before = engine.viewport.pan;
        engine.viewport.pan_by(Vector::new(12.0, -6.0));
        assert_eq!(engine.viewport.pan, pan_before + Vector::new(12.0, -6.0));
    }

    #[test]
    fn test_engine_select_node_and_object_label_are_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);

        engine.select_node(rect_id);
        assert_eq!(engine.selection.primary(), Some(rect_id));

        engine.set_object_label(rect_id, "Logo mark");
        assert_eq!(
            engine
                .document
                .get_node(rect_id)
                .unwrap()
                .attributes
                .get("inkscape:label")
                .map(String::as_str),
            Some("Logo mark")
        );

        engine.undo();
        assert!(!engine
            .document
            .get_node(rect_id)
            .unwrap()
            .attributes
            .contains_key("inkscape:label"));
    }

    #[test]
    fn test_engine_object_xml_id_is_undoable_and_unique() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let r1 = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        let r2 = add_rect(&mut engine.document, 20.0, 0.0, 10.0, 10.0);

        assert!(engine.set_object_xml_id(r1, "Logo Mark"));
        assert_eq!(
            engine.document.get_node(r1).unwrap().xml_id.as_deref(),
            Some("Logo-Mark")
        );
        assert!(!engine.set_object_xml_id(r2, "Logo-Mark"));
        assert_eq!(engine.document.get_node(r2).unwrap().xml_id, None);

        engine.undo();
        assert_eq!(engine.document.get_node(r1).unwrap().xml_id, None);
    }

    #[test]
    fn test_engine_routes_new_tool_objects_to_active_layer() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let layer_id = engine.add_layer("Artwork").unwrap();
        engine.tools.switch_to(ToolId::Rect);

        engine.on_mouse_down(MouseEvent::left(10.0, 10.0));
        engine.on_mouse_up(MouseEvent::left(80.0, 60.0));

        assert_eq!(engine.document.children(root), &[layer_id]);
        assert_eq!(engine.document.children(layer_id).len(), 1);
        let rect_id = engine.document.children(layer_id)[0];
        assert!(matches!(
            engine.document.get_node(rect_id).unwrap().element,
            SvgElement::Rect(_)
        ));

        engine.undo();
        assert!(engine.document.children(layer_id).is_empty());

        engine.redo();
        assert_eq!(engine.document.children(root), &[layer_id]);
        assert_eq!(engine.document.children(layer_id).len(), 1);
    }

    #[test]
    fn test_engine_snaps_tool_mouse_input_to_grid() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        engine.tools.switch_to(ToolId::Rect);
        engine.snapping.prefs.snap_to_grid = true;
        engine.snapping.prefs.grid_spacing = 10.0;
        engine.snapping.prefs.tolerance = 10.0;

        engine.on_mouse_down(MouseEvent::left(12.0, 18.0));
        assert!(engine.last_snap_indicator.is_some());
        engine.on_mouse_up(MouseEvent::left(43.0, 47.0));

        let rect_id = engine.document.children(root)[0];
        let SvgElement::Rect(attrs) = &engine.document.get_node(rect_id).unwrap().element else {
            panic!("expected rect");
        };
        assert_eq!(attrs.x.value, 10.0);
        assert_eq!(attrs.y.value, 20.0);
        assert_eq!(attrs.width.value, 30.0);
        assert_eq!(attrs.height.value, 30.0);
    }

    #[test]
    fn test_engine_respects_cursor_snap_source_toggle() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        engine.tools.switch_to(ToolId::Rect);
        engine.snapping.prefs.snap_to_grid = true;
        engine.snapping.prefs.grid_spacing = 10.0;
        engine.snapping.prefs.tolerance = 10.0;
        engine.snapping.prefs.sources.snap_from_cursor = false;

        engine.on_mouse_down(MouseEvent::left(12.0, 18.0));
        assert!(engine.last_snap_indicator.is_none());
        engine.on_mouse_up(MouseEvent::left(43.0, 47.0));

        let rect_id = engine.document.children(root)[0];
        let SvgElement::Rect(attrs) = &engine.document.get_node(rect_id).unwrap().element else {
            panic!("expected rect");
        };
        assert_eq!(attrs.x.value, 12.0);
        assert_eq!(attrs.y.value, 18.0);
    }

    #[test]
    fn test_engine_snaps_tool_mouse_input_to_object_bbox() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        add_rect(&mut engine.document, 100.0, 100.0, 50.0, 50.0);
        engine.tools.switch_to(ToolId::Rect);
        engine.snapping.prefs.snap_to_grid = false;
        engine.snapping.prefs.snap_to_bbox_corners = true;
        engine.snapping.prefs.snap_to_bbox_edges = false;
        engine.snapping.prefs.snap_to_bbox_centers = false;
        engine.snapping.prefs.snap_to_path = false;
        engine.snapping.prefs.snap_to_path_nodes = false;
        engine.snapping.prefs.tolerance = 10.0;

        engine.on_mouse_down(MouseEvent::left(103.0, 98.0));
        engine.on_mouse_up(MouseEvent::left(130.0, 130.0));

        let rect_id = engine.document.children(root)[1];
        let SvgElement::Rect(attrs) = &engine.document.get_node(rect_id).unwrap().element else {
            panic!("expected rect");
        };
        assert_eq!(attrs.x.value, 100.0);
        assert_eq!(attrs.y.value, 100.0);
        assert_eq!(attrs.width.value, 30.0);
        assert_eq!(attrs.height.value, 30.0);
    }

    #[test]
    fn test_engine_adds_guides_and_snaps_tool_input_to_them() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let guide_id = engine.add_horizontal_guide(75.0).unwrap();
        engine.tools.switch_to(ToolId::Rect);
        engine.snapping.prefs.snap_to_grid = false;
        engine.snapping.prefs.snap_to_guides = true;
        engine.snapping.prefs.snap_to_bbox_corners = false;
        engine.snapping.prefs.snap_to_bbox_edges = false;
        engine.snapping.prefs.snap_to_bbox_centers = false;
        engine.snapping.prefs.snap_to_path = false;
        engine.snapping.prefs.snap_to_path_nodes = false;
        engine.snapping.prefs.tolerance = 10.0;

        assert!(matches!(
            engine.document.get_node(guide_id).unwrap().element,
            SvgElement::SodipodiGuide(_)
        ));
        assert_eq!(engine.guide_lines().len(), 1);

        engine.on_mouse_down(MouseEvent::left(15.0, 78.0));
        engine.on_mouse_up(MouseEvent::left(45.0, 95.0));

        let rect_id = engine.document.children(root)[1];
        let SvgElement::Rect(attrs) = &engine.document.get_node(rect_id).unwrap().element else {
            panic!("expected rect");
        };
        assert_eq!(attrs.y.value, 75.0);

        engine.undo();
        assert!(engine.document.get_node(guide_id).is_some());

        engine.undo();
        assert!(engine.document.get_node(guide_id).is_none());
    }

    #[test]
    fn test_engine_add_sublayer_targets_active_layer() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let parent = engine.add_layer("Parent").unwrap();
        let child = engine.add_sublayer("Child").unwrap();

        assert_eq!(
            engine.document.get_node(child).unwrap().parent,
            Some(parent)
        );
        assert_eq!(engine.active_layer, Some(child));

        engine.undo();
        assert!(engine.document.get_node(child).is_none());
        assert!(engine.document.get_node(parent).is_some());
        assert_eq!(engine.active_layer, None);
    }

    #[test]
    fn test_engine_paste_targets_active_layer() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let layer = engine.add_layer("Paste Target").unwrap();
        let rect = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);

        engine.select_node(rect);
        engine.copy();
        engine.selection.clear();
        engine.paste();

        assert_eq!(engine.document.children(root), &[layer, rect]);
        assert_eq!(engine.document.children(layer).len(), 1);
        let pasted = engine.document.children(layer)[0];
        assert!(matches!(
            engine.document.get_node(pasted).unwrap().element,
            SvgElement::Rect(_)
        ));

        engine.undo();
        assert!(engine.document.children(layer).is_empty());
    }

    #[test]
    fn test_engine_selected_plain_text_requires_text_only_selection() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let text_id = engine.document.add_node(SvgElement::Text(TextAttrs {
            text_content: "Hello".to_string(),
            ..Default::default()
        }));
        engine.document.append_child(root, text_id);
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);

        engine.select_node(text_id);
        assert_eq!(engine.selected_plain_text().as_deref(), Some("Hello"));

        engine.selection.add(rect_id);
        assert_eq!(engine.selected_plain_text(), None);
    }

    #[test]
    fn test_engine_rename_layer_is_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let layer_id = engine.add_layer("Layer 1").unwrap();

        engine.rename_layer(layer_id, "Sketch");
        assert_eq!(engine.get_layers()[0].1, "Sketch");

        engine.undo();
        assert_eq!(engine.get_layers()[0].1, "Layer 1");

        engine.redo();
        assert_eq!(engine.get_layers()[0].1, "Sketch");
    }

    #[test]
    fn test_engine_merge_layer_down_is_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let bottom = engine.add_layer("Bottom").unwrap();
        let top = engine.add_layer("Top").unwrap();
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        engine.document.detach_node(rect_id);
        engine.document.append_child(top, rect_id);

        engine.merge_layer_down(top);

        assert_eq!(engine.get_layers().len(), 1);
        assert_eq!(engine.get_layers()[0].0, bottom);
        assert_eq!(engine.document.children(bottom), &[rect_id]);

        engine.undo();
        assert_eq!(engine.get_layers().len(), 2);
        assert_eq!(engine.document.children(top), &[rect_id]);

        engine.redo();
        assert_eq!(engine.get_layers().len(), 1);
        assert_eq!(engine.document.children(bottom), &[rect_id]);
    }

    #[test]
    fn test_engine_flatten_layers_is_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let layer = engine.add_layer("Layer").unwrap();
        let rect_id = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        engine.document.detach_node(rect_id);
        engine.document.append_child(layer, rect_id);

        engine.flatten_layers();

        assert!(engine.get_layers().is_empty());
        assert_eq!(engine.document.children(root), &[rect_id]);

        engine.undo();
        assert_eq!(engine.get_layers().len(), 1);
        let restored_layer = engine.get_layers()[0].0;
        assert_eq!(engine.get_layers()[0].1, "Layer");
        assert_eq!(engine.document.children(restored_layer), &[rect_id]);
    }

    #[test]
    fn test_engine_stroke_to_path_uses_stroke_as_fill() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        let node = engine.document.get_node_mut(path_id).unwrap();
        node.presentation.stroke = Some(Paint::Color(Color::RED));
        node.presentation.stroke_width = Some(SvgLength::new(8.0, LengthUnit::None));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.stroke_to_path_selected();

        let node = engine.document.get_node(path_id).unwrap();
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected path");
        };
        assert!(attrs.d.bounding_box().height() >= 8.0);
        assert_eq!(node.presentation.fill, Some(Paint::Color(Color::RED)));
        assert_eq!(node.presentation.stroke, Some(Paint::None));
    }

    #[test]
    fn test_engine_stroke_to_path_preserves_dash_geometry() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        let node = engine.document.get_node_mut(path_id).unwrap();
        node.presentation.stroke = Some(Paint::Color(Color::BLACK));
        node.presentation.stroke_width = Some(SvgLength::new(8.0, LengthUnit::None));
        node.presentation.stroke_dasharray = Some(vec![20.0, 10.0]);
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.stroke_to_path_selected();

        let SvgElement::Path(attrs) = &engine.document.get_node(path_id).unwrap().element else {
            panic!("expected path");
        };
        let move_count = attrs
            .d
            .elements()
            .iter()
            .filter(|segment| matches!(segment, PathSegment::MoveTo(_)))
            .count();
        assert!(
            move_count >= 2,
            "dashed stroke should become multiple filled subpaths, got {move_count}"
        );
    }

    #[test]
    fn test_engine_stroke_to_path_scales_regular_transformed_stroke() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        let node = engine.document.get_node_mut(path_id).unwrap();
        node.presentation.stroke = Some(Paint::Color(Color::BLACK));
        node.presentation.stroke_width = Some(SvgLength::new(8.0, LengthUnit::None));
        node.presentation.transform = Some(Transform2D::scale(2.0));
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.stroke_to_path_selected();

        let node = engine.document.get_node(path_id).unwrap();
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected path");
        };
        let height = attrs.d.bounding_box().height();
        assert!(
            (height - 16.0).abs() < 0.5,
            "scaled stroke should expand to visible width 16, got {height}"
        );
        assert!(node.presentation.transform.is_none());
    }

    #[test]
    fn test_engine_stroke_to_path_keeps_non_scaling_stroke_width() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        let node = engine.document.get_node_mut(path_id).unwrap();
        node.presentation.stroke = Some(Paint::Color(Color::BLACK));
        node.presentation.stroke_width = Some(SvgLength::new(8.0, LengthUnit::None));
        node.presentation.transform = Some(Transform2D::scale(2.0));
        node.presentation.vector_effect = Some("non-scaling-stroke".to_string());
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.stroke_to_path_selected();

        let node = engine.document.get_node(path_id).unwrap();
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected path");
        };
        let height = attrs.d.bounding_box().height();
        assert!(
            (height - 8.0).abs() < 0.5,
            "non-scaling stroke should keep visible width 8, got {height}"
        );
        assert!(node.presentation.transform.is_none());
        assert!(node.presentation.vector_effect.is_none());
        let effect = attrs.inkscape_path_effect.as_deref().unwrap();
        assert!(effect.contains("vector-effect=non-scaling-stroke"));
        assert!(attrs.inkscape_original_d.is_some());
    }

    #[test]
    fn test_engine_stroke_to_path_records_marker_metadata() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(60.0, 40.0));
        path.line_to(Point::new(110.0, 10.0));

        let path_id = engine.document.add_node(SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        let node = engine.document.get_node_mut(path_id).unwrap();
        node.presentation.stroke = Some(Paint::Color(Color::BLACK));
        node.presentation.stroke_width = Some(SvgLength::new(6.0, LengthUnit::None));
        node.presentation.marker_start = Some("url(#arrow-start)".to_string());
        node.presentation.marker_mid = Some("url(#dot-mid)".to_string());
        node.presentation.marker_end = Some("url(#arrow-end)".to_string());
        engine.document.append_child(root, path_id);
        engine.selection.select(path_id);

        engine.stroke_to_path_selected();

        let node = engine.document.get_node(path_id).unwrap();
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected path");
        };
        let effect = attrs.inkscape_path_effect.as_deref().unwrap();
        assert!(effect.contains("marker-start=url(#arrow-start)"));
        assert!(effect.contains("marker-mid=url(#dot-mid)"));
        assert!(effect.contains("marker-end=url(#arrow-end)"));
        assert!(attrs.inkscape_original_d.is_some());
        assert!(node.presentation.marker_start.is_none());
        assert!(node.presentation.marker_mid.is_none());
        assert!(node.presentation.marker_end.is_none());
    }

    #[test]
    fn test_engine_combine_and_break_apart_paths() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let first = add_rect(&mut engine.document, 0.0, 0.0, 10.0, 10.0);
        let second = add_rect(&mut engine.document, 20.0, 20.0, 10.0, 10.0);
        engine.selection.select(first);
        engine.selection.add(second);

        engine.combine_selected_paths();

        assert_eq!(engine.document.children(root).len(), 1);
        let SvgElement::Path(attrs) = &engine.document.get_node(first).unwrap().element else {
            panic!("expected combined path");
        };
        assert!(attrs.d.element_count() > 8);

        engine.break_apart_selected_paths();
        assert_eq!(engine.document.children(root).len(), 2);
    }

    #[test]
    fn test_engine_intersection_replaces_selection_with_result_path() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let first = add_rect(&mut engine.document, 0.0, 0.0, 100.0, 100.0);
        let second = add_rect(&mut engine.document, 50.0, 50.0, 100.0, 100.0);
        engine.selection.select(first);
        engine.selection.add(second);

        engine.intersection_selected_paths();

        assert_eq!(engine.document.children(root).len(), 1);
        let SvgElement::Path(attrs) = &engine.document.get_node(first).unwrap().element else {
            panic!("expected boolean result path");
        };
        let bbox = attrs.d.bounding_box();
        assert!(bbox.width() <= 51.0);
        assert!(bbox.height() <= 51.0);
    }

    #[test]
    fn test_engine_boolean_ops_bake_object_transforms() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let first = add_rect(&mut engine.document, 0.0, 0.0, 100.0, 100.0);
        let second = add_rect(&mut engine.document, 75.0, 0.0, 100.0, 100.0);
        engine
            .document
            .get_node_mut(first)
            .unwrap()
            .presentation
            .transform = Some(Transform2D::translate(50.0, 0.0));
        engine.selection.select(first);
        engine.selection.add(second);

        engine.intersection_selected_paths();

        assert_eq!(engine.document.children(root).len(), 1);
        let node = engine.document.get_node(first).unwrap();
        assert!(node.presentation.transform.is_none());
        let SvgElement::Path(attrs) = &node.element else {
            panic!("expected transformed boolean result path");
        };
        let bbox = attrs.d.bounding_box();
        assert!((bbox.x0 - 75.0).abs() < 1.0);
        assert!((bbox.x1 - 150.0).abs() < 1.0);

        engine.undo();
        assert_eq!(engine.document.children(root).len(), 2);
        assert!(engine
            .document
            .get_node(first)
            .unwrap()
            .presentation
            .transform
            .is_some());
    }

    #[test]
    fn test_engine_cut_path_splits_primary_and_removes_knife() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        let root = engine.document.root().unwrap();
        let subject = add_line(&mut engine.document, 0.0, 0.0, 10.0, 0.0);
        let knife = add_line(&mut engine.document, 5.0, -5.0, 5.0, 5.0);
        engine.selection.select(subject);
        engine.selection.add(knife);

        engine.cut_selected_paths();

        assert!(engine.document.get_node(knife).is_none());
        assert!(engine.document.children(root).len() > 1);
        let SvgElement::Path(_) = &engine.document.get_node(subject).unwrap().element else {
            panic!("expected cut subject to become a path");
        };

        engine.undo();
        assert!(engine.document.get_node(knife).is_some());
        assert_eq!(engine.document.children(root).len(), 2);

        engine.redo();
        assert!(engine.document.get_node(knife).is_none());
        assert!(engine.document.children(root).len() > 1);
    }

    #[test]
    fn test_compound_command() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();

        let mut compound = CompoundCommand::new(
            "Add two rects",
            vec![
                Box::new(AddNodeCommand::new(
                    root,
                    SvgElement::Rect(RectAttrs::default()),
                    PresentationAttrs::default(),
                    None,
                )),
                Box::new(AddNodeCommand::new(
                    root,
                    SvgElement::Rect(RectAttrs::default()),
                    PresentationAttrs::default(),
                    None,
                )),
            ],
        );

        compound.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 2);

        compound.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 0);

        compound.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 2);
    }

    #[test]
    fn test_modify_attribute_command() {
        let mut doc = make_doc();
        let rect_id = add_rect(&mut doc, 0.0, 0.0, 50.0, 50.0);

        let old_attrs = doc.get_node(rect_id).unwrap().presentation.clone();
        let mut new_attrs = old_attrs.clone();
        new_attrs.fill = Some(Paint::Color(Color::RED));

        let mut cmd = ModifyAttributeCommand::new(rect_id, old_attrs.clone(), new_attrs);
        cmd.execute(&mut doc);

        let node = doc.get_node(rect_id).unwrap();
        assert_eq!(node.presentation.fill, Some(Paint::Color(Color::RED)));

        cmd.undo(&mut doc);
        let node = doc.get_node(rect_id).unwrap();
        assert_eq!(node.presentation.fill, old_attrs.fill);
    }

    #[test]
    fn test_selection_bounding_box() {
        let mut doc = make_doc();
        let r1 = add_rect(&mut doc, 10.0, 20.0, 100.0, 50.0);
        let r2 = add_rect(&mut doc, 200.0, 300.0, 50.0, 80.0);

        let mut sel = SelectionSet::new();
        sel.add(r1);
        sel.add(r2);

        let bb = sel.bounding_box(&doc).unwrap();
        assert!((bb.x0 - 10.0).abs() < 1e-10);
        assert!((bb.y0 - 20.0).abs() < 1e-10);
        assert!((bb.x1 - 250.0).abs() < 1e-10);
        assert!((bb.y1 - 380.0).abs() < 1e-10);
    }

    // --- Multi-page features (17.6) ---

    #[test]
    fn test_engine_add_page_and_get_pages() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        let p1 = engine.add_page(0.0, 0.0, 210.0, 297.0, Some("Page 1"));
        assert!(p1.is_some());
        let p2 = engine.add_page(220.0, 0.0, 210.0, 297.0, Some("Page 2"));
        assert!(p2.is_some());

        let pages = engine.get_pages();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].1, "Page 1");
        assert_eq!(pages[1].1, "Page 2");
        assert!((pages[0].4 - 210.0).abs() < 1e-10); // width
        assert!((pages[0].5 - 297.0).abs() < 1e-10); // height
    }

    #[test]
    fn test_engine_page_count() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        assert_eq!(engine.page_count(), 0);

        engine.add_page(0.0, 0.0, 100.0, 100.0, Some("A"));
        assert_eq!(engine.page_count(), 1);

        engine.add_page(110.0, 0.0, 100.0, 100.0, Some("B"));
        assert_eq!(engine.page_count(), 2);
    }

    #[test]
    fn test_engine_page_operations_are_undoable() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        let page = engine
            .add_page(0.0, 0.0, 100.0, 120.0, Some("Original"))
            .unwrap();
        assert_eq!(engine.page_count(), 1);

        engine.undo();
        assert_eq!(engine.page_count(), 0);

        engine.redo();
        assert_eq!(engine.page_count(), 1);
        assert_eq!(engine.get_pages()[0].0, page);

        engine.move_page(page, 25.0, 35.0);
        let pages = engine.get_pages();
        assert_eq!(pages[0].2, 25.0);
        assert_eq!(pages[0].3, 35.0);

        engine.undo();
        let pages = engine.get_pages();
        assert_eq!(pages[0].2, 0.0);
        assert_eq!(pages[0].3, 0.0);

        engine.resize_page(page, 210.0, 297.0);
        engine.set_page_label(page, "Renamed");
        let pages = engine.get_pages();
        assert_eq!(pages[0].1, "Renamed");
        assert_eq!(pages[0].4, 210.0);
        assert_eq!(pages[0].5, 297.0);

        engine.undo();
        assert_eq!(engine.get_pages()[0].1, "Original");

        engine.delete_page(page);
        assert_eq!(engine.page_count(), 0);
        engine.undo();
        assert_eq!(engine.page_count(), 1);
        assert_eq!(engine.get_pages()[0].0, page);
    }

    #[test]
    fn test_engine_navigate_next_prev_page() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        // No pages => navigation returns false.
        assert!(!engine.navigate_to_next_page());
        assert!(!engine.navigate_to_prev_page());

        let p1 = engine.add_page(0.0, 0.0, 100.0, 100.0, Some("P1")).unwrap();
        let p2 = engine
            .add_page(200.0, 0.0, 100.0, 100.0, Some("P2"))
            .unwrap();
        let p3 = engine
            .add_page(400.0, 0.0, 100.0, 100.0, Some("P3"))
            .unwrap();

        // Navigate to page 1 first.
        engine.navigate_to_page(p1);

        // Navigate next should move forward.
        assert!(engine.navigate_to_next_page());
        // Navigate prev should move backward.
        assert!(engine.navigate_to_prev_page());
    }

    #[test]
    fn test_engine_export_page_svg() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        let p1 = engine
            .add_page(0.0, 0.0, 210.0, 297.0, Some("Page 1"))
            .unwrap();

        let svg = engine.export_page_svg(p1);
        assert!(svg.is_some(), "export_page_svg should return Some");
        let svg = svg.unwrap();
        assert!(svg.contains("<svg"), "Should contain svg tag");
        assert!(svg.contains("width=\"210\""), "Should have page width");
        assert!(svg.contains("height=\"297\""), "Should have page height");
        assert!(
            svg.contains("viewBox=\"0 0 210 297\""),
            "Should have page viewBox"
        );
    }

    #[test]
    fn test_engine_export_all_pages() {
        let mut engine = EditorEngine::new(800.0, 600.0);

        engine.add_page(0.0, 0.0, 100.0, 100.0, Some("Alpha"));
        engine.add_page(110.0, 0.0, 200.0, 200.0, Some("Beta"));

        let exports = engine.export_all_pages_svg();
        assert_eq!(exports.len(), 2);
        assert_eq!(exports[0].0, "Alpha");
        assert_eq!(exports[1].0, "Beta");
        assert!(exports[0].1.contains("<svg"));
        assert!(exports[1].1.contains("<svg"));
    }

    #[test]
    fn test_engine_export_nonexistent_page() {
        let engine = EditorEngine::new(800.0, 600.0);
        let fake_id = NodeId::new();
        assert!(engine.export_page_svg(fake_id).is_none());
    }
}
