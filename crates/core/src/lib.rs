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
    AddNodeCommand, CompoundCommand, GroupCommand, ModifyAttributeCommand, ModifyPathCommand,
    RemoveNodeCommand, ReorderCommand, ReorderDirection, SetLockCommand, SetVisibilityCommand,
    TransformCommand, UngroupCommand,
};
pub use engine::EditorEngine;
pub use event::{EditorEvent, EventBus};
pub use input::{Key, KeyEvent, Modifiers, MouseButton, MouseEvent};
pub use selection::SelectionSet;
pub use shortcuts::{Action, Shortcut, ShortcutManager};
pub use snap::{
    SnapContext, SnapIndicator, SnapPreferences, SnapResult, SnapTarget, SnappingEngine,
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
    use sivgo_dom::{NodeId, PresentationAttrs, RectAttrs, SvgDocument, SvgElement, SvgSvgAttrs};
    use sivgo_math::{Point, Transform2D, Vector};

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
