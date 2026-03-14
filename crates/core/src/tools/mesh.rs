//! Mesh gradient tool: create and edit mesh gradients on canvas.
//!
//! States:
//! - **Idle**: waiting for user input.
//! - **Creating**: user is dragging to define the initial mesh rectangle.
//! - **Editing**: user clicked an object that already has a mesh gradient;
//!   mesh control points/handles can be moved (future work).
//!
//! On mouse-down in Idle:
//! - If the primary selection has a mesh gradient fill, enter Editing.
//! - Otherwise, if there is a primary selection, start Creating.
//!
//! On mouse-up after Creating: add a `<meshGradient>` element to `<defs>`
//! with a single row/patch and apply it as `fill` on the target element.

use sivgo_dom::node::{MeshGradientAttrs, MeshPatchAttrs, MeshRowAttrs};
use sivgo_dom::types::{Color, GradientUnits, LengthUnit, Paint, SvgLength};
use sivgo_dom::{DefsAttrs, NodeId, PresentationAttrs, SvgDocument, SvgElement, SvgNode};
use sivgo_math::Point;

use crate::command::Command;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum State {
    Idle,
    /// Dragging to define the initial mesh rectangle.
    Creating {
        start: Point,
        element_id: NodeId,
    },
    /// Editing an existing mesh gradient (future: drag handles).
    Editing {
        #[allow(dead_code)]
        element_id: NodeId,
    },
}

// ---------------------------------------------------------------------------
// MeshTool
// ---------------------------------------------------------------------------

/// Interactive tool for creating and editing SVG 2 mesh gradients.
///
/// Usage:
/// 1. Select an element with the Select tool.
/// 2. Switch to the Mesh tool.
/// 3. Click-drag on the canvas to define the initial mesh rectangle.
/// 4. On release a `<meshGradient>` with one row/patch is created in `<defs>`
///    and the element's fill is set to `url(#<mesh-id>)`.
#[derive(Debug)]
pub struct MeshTool {
    state: State,
}

impl MeshTool {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    /// Check whether the given element already has a mesh gradient fill.
    fn has_mesh_gradient(doc: &SvgDocument, element_id: NodeId) -> bool {
        let node = match doc.get_node(element_id) {
            Some(n) => n,
            None => return false,
        };
        if let Some(Paint::Url(url)) = &node.presentation.fill {
            // Resolve the url reference to see if it points to a meshGradient.
            let id_ref = url.trim_start_matches('#');
            if let Some(target_id) = doc.find_by_xml_id(id_ref) {
                if let Some(target) = doc.get_node(target_id) {
                    return matches!(target.element, SvgElement::MeshGradient(_));
                }
            }
        }
        false
    }
}

impl Default for MeshTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for MeshTool {
    fn name(&self) -> &str {
        "Mesh"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let element_id = match selection.primary() {
            Some(id) => id,
            None => return ToolResponse::None,
        };

        // If the element already has a mesh gradient, enter editing mode.
        if Self::has_mesh_gradient(doc, element_id) {
            self.state = State::Editing { element_id };
            return ToolResponse::Handled;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        self.state = State::Creating {
            start: doc_pt,
            element_id,
        };
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        match &self.state {
            State::Idle => ToolResponse::None,
            State::Creating { .. } | State::Editing { .. } => ToolResponse::Handled,
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let (start, element_id) = match std::mem::replace(&mut self.state, State::Idle) {
            State::Creating { start, element_id } => (start, element_id),
            // Editing clicks are handled without producing a command (for now).
            State::Editing { .. } | State::Idle => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));

        // Need a meaningful drag distance.
        if (end.x - start.x).abs() < 0.5 && (end.y - start.y).abs() < 0.5 {
            return ToolResponse::None;
        }

        if doc.get_node(element_id).is_none() {
            return ToolResponse::None;
        }

        let old_pres = doc
            .get_node(element_id)
            .map(|n| n.presentation.clone())
            .unwrap_or_default();

        let cmd = ApplyMeshGradientCommand::new(element_id, start, end, old_pres);
        ToolResponse::ExecuteCommand(Box::new(cmd))
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        use crate::input::Key;
        if event.key == Key::Escape {
            self.state = State::Idle;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.state = State::Idle;
    }

    fn on_deactivate(&mut self) {
        self.state = State::Idle;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

// ---------------------------------------------------------------------------
// ApplyMeshGradientCommand
// ---------------------------------------------------------------------------

/// Creates a `<meshGradient>` with one `<meshrow>` / `<meshpatch>` inside
/// `<defs>` and sets the target element's fill to reference it.
#[derive(Debug)]
struct ApplyMeshGradientCommand {
    target_id: NodeId,
    start: Point,
    end: Point,
    old_pres: PresentationAttrs,

    // State filled on first execute.
    created_defs: bool,
    defs_id: Option<NodeId>,
    mesh_id: Option<NodeId>,
    mesh_xml_id: Option<String>,
    row_id: Option<NodeId>,
    patch_id: Option<NodeId>,
}

impl ApplyMeshGradientCommand {
    fn new(target_id: NodeId, start: Point, end: Point, old_pres: PresentationAttrs) -> Self {
        Self {
            target_id,
            start,
            end,
            old_pres,
            created_defs: false,
            defs_id: None,
            mesh_id: None,
            mesh_xml_id: None,
            row_id: None,
            patch_id: None,
        }
    }

    fn find_defs(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        for &child_id in doc.children(root) {
            if let Some(node) = doc.get_node(child_id) {
                if matches!(node.element, SvgElement::Defs(_)) {
                    return Some(child_id);
                }
            }
        }
        None
    }
}

impl Command for ApplyMeshGradientCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let root = match doc.root() {
            Some(r) => r,
            None => return,
        };

        // 1. Ensure <defs> exists.
        let defs_id = if let Some(existing) = Self::find_defs(doc) {
            self.created_defs = false;
            existing
        } else {
            let id = if let Some(prev_id) = self.defs_id {
                let node = SvgNode::new(prev_id, SvgElement::Defs(DefsAttrs));
                doc.insert_node(node);
                doc.insert_child_at(root, prev_id, 0);
                prev_id
            } else {
                let id = doc.add_node(SvgElement::Defs(DefsAttrs));
                doc.insert_child_at(root, id, 0);
                id
            };
            self.created_defs = true;
            id
        };
        self.defs_id = Some(defs_id);

        // 2. Create <meshGradient>.
        let mesh_xml_id = self.mesh_xml_id.clone().unwrap_or_else(|| {
            let uuid = uuid_simple();
            format!("mesh_{}", uuid)
        });

        // Use the top-left corner of the drag rect as the mesh origin.
        let x = self.start.x.min(self.end.x);
        let y = self.start.y.min(self.end.y);

        let mesh_element = SvgElement::MeshGradient(MeshGradientAttrs {
            x: Some(SvgLength::new(x, LengthUnit::None)),
            y: Some(SvgLength::new(y, LengthUnit::None)),
            gradient_units: Some(GradientUnits::UserSpaceOnUse),
            gradient_transform: None,
            href: None,
        });

        let mesh_node_id = if let Some(prev_id) = self.mesh_id {
            let mut node = SvgNode::new(prev_id, mesh_element);
            node.xml_id = Some(mesh_xml_id.clone());
            doc.insert_node(node);
            doc.append_child(defs_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(mesh_element);
            if let Some(node) = doc.get_node_mut(id) {
                node.xml_id = Some(mesh_xml_id.clone());
            }
            doc.append_child(defs_id, id);
            id
        };
        self.mesh_id = Some(mesh_node_id);
        self.mesh_xml_id = Some(mesh_xml_id.clone());

        // 3. Create <meshrow>.
        let row_element = SvgElement::MeshRow(MeshRowAttrs {});
        let row_node_id = if let Some(prev_id) = self.row_id {
            let node = SvgNode::new(prev_id, row_element);
            doc.insert_node(node);
            doc.append_child(mesh_node_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(row_element);
            doc.append_child(mesh_node_id, id);
            id
        };
        self.row_id = Some(row_node_id);

        // 4. Create <meshpatch> with corner colors.
        let w = (self.end.x - self.start.x).abs();
        let h = (self.end.y - self.start.y).abs();
        let path_data = format!("l {w},0 l 0,{h} l -{w},0 z", w = w, h = h);

        let patch_element = SvgElement::MeshPatch(MeshPatchAttrs {
            stop_color: Some(Color::rgb(100, 149, 237)), // cornflower blue
            path: Some(path_data),
        });

        let patch_node_id = if let Some(prev_id) = self.patch_id {
            let node = SvgNode::new(prev_id, patch_element);
            doc.insert_node(node);
            doc.append_child(row_node_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(patch_element);
            doc.append_child(row_node_id, id);
            id
        };
        self.patch_id = Some(patch_node_id);

        // 5. Set target fill.
        if let Some(node) = doc.get_node_mut(self.target_id) {
            node.presentation.fill = Some(Paint::Url(format!("#{}", mesh_xml_id)));
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Restore fill.
        if let Some(node) = doc.get_node_mut(self.target_id) {
            node.presentation = self.old_pres.clone();
        }

        // Remove patch, row, mesh.
        if let Some(id) = self.patch_id {
            doc.remove_node(id);
        }
        if let Some(id) = self.row_id {
            doc.remove_node(id);
        }
        if let Some(id) = self.mesh_id {
            doc.remove_node(id);
        }

        // Remove defs if we created it.
        if self.created_defs {
            if let Some(id) = self.defs_id {
                doc.remove_node(id);
            }
        }
    }

    fn description(&self) -> &str {
        "Apply mesh gradient"
    }
}

/// Generate a short pseudo-unique identifier (8 hex chars).
fn uuid_simple() -> String {
    let id = NodeId::new();
    let s = id.0.to_string().replace('-', "");
    s[..8.min(s.len())].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{RectAttrs, SvgSvgAttrs};

    fn make_doc_with_rect() -> (SvgDocument, NodeId) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(10.0, LengthUnit::None),
            width: SvgLength::new(80.0, LengthUnit::None),
            height: SvgLength::new(40.0, LengthUnit::None),
            ..Default::default()
        }));
        doc.append_child(svg_id, rect_id);
        (doc, rect_id)
    }

    #[test]
    fn mesh_tool_ignores_when_no_selection() {
        let mut tool = MeshTool::new();
        let doc = SvgDocument::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let response = tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        assert!(matches!(response, ToolResponse::None));
    }

    #[test]
    fn apply_mesh_gradient_creates_defs_and_mesh() {
        let (mut doc, rect_id) = make_doc_with_rect();
        let root = doc.root().unwrap();

        let old_pres = doc.get_node(rect_id).unwrap().presentation.clone();
        let mut cmd = ApplyMeshGradientCommand::new(
            rect_id,
            Point::new(10.0, 10.0),
            Point::new(90.0, 50.0),
            old_pres,
        );
        cmd.execute(&mut doc);

        // A <defs> should exist.
        let children = doc.children(root);
        assert!(children.len() >= 2);
        let defs_node = doc.get_node(children[0]).unwrap();
        assert_eq!(defs_node.element.tag_name(), "defs");

        // The <defs> should contain a <meshGradient>.
        let defs_children = doc.children(children[0]);
        assert_eq!(defs_children.len(), 1);
        let mesh_node = doc.get_node(defs_children[0]).unwrap();
        assert_eq!(mesh_node.element.tag_name(), "meshGradient");

        // The rect's fill should be a url() reference.
        let rect_node = doc.get_node(rect_id).unwrap();
        match &rect_node.presentation.fill {
            Some(Paint::Url(url)) => {
                assert!(url.starts_with("#mesh_"));
            }
            other => panic!("expected Paint::Url, got {:?}", other),
        }
    }

    #[test]
    fn undo_removes_mesh_gradient() {
        let (mut doc, rect_id) = make_doc_with_rect();

        let old_fill = Paint::Color(Color::RED);
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(old_fill.clone());
        }

        let old_pres = doc.get_node(rect_id).unwrap().presentation.clone();
        let mut cmd = ApplyMeshGradientCommand::new(
            rect_id,
            Point::new(0.0, 0.0),
            Point::new(100.0, 100.0),
            old_pres,
        );
        cmd.execute(&mut doc);
        assert!(matches!(
            doc.get_node(rect_id).unwrap().presentation.fill,
            Some(Paint::Url(_))
        ));

        cmd.undo(&mut doc);
        assert_eq!(
            doc.get_node(rect_id).unwrap().presentation.fill,
            Some(old_fill)
        );
    }
}
