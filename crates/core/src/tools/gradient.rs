//! Gradient tool: click-drag on a selected element to apply a linear gradient.
//!
//! On mouse-down the tool records the start point and the primary selected
//! element.  During the drag no visual feedback is emitted yet (future work).
//! On mouse-up the tool creates a `<linearGradient>` element inside `<defs>`
//! with two colour stops and sets the target element's `fill` to reference it.

use sivgo_dom::types::{Color, GradientUnits, LengthUnit, Paint, SvgLength};
use sivgo_dom::{
    DefsAttrs, LinearGradientAttrs, NodeId, PresentationAttrs, StopAttrs, SvgDocument, SvgElement,
    SvgNode,
};
use sivgo_math::Point;

use crate::command::Command;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// Tool state
// ---------------------------------------------------------------------------

/// Internal state machine for the gradient tool.
#[derive(Debug)]
enum State {
    /// Waiting for input.
    Idle,
    /// The user pressed the mouse button on a selected element and is dragging
    /// to define the gradient direction.
    Creating {
        /// Gradient start point in document coordinates.
        start: Point,
        /// The element that will receive the gradient fill.
        element_id: NodeId,
    },
}

// ---------------------------------------------------------------------------
// GradientTool
// ---------------------------------------------------------------------------

/// Interactive tool for creating linear gradients on the canvas.
///
/// Usage:
/// 1. Select an element with the Select tool.
/// 2. Switch to the Gradient tool.
/// 3. Click-drag on the canvas to define the gradient direction.
/// 4. On release a `<linearGradient>` is created in `<defs>` and the selected
///    element's fill is set to `url(#<gradient-id>)`.
#[derive(Debug)]
pub struct GradientTool {
    state: State,
}

impl GradientTool {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }
}

impl Default for GradientTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GradientTool {
    fn name(&self) -> &str {
        "Gradient"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        // We need a selected element to apply the gradient to.
        let element_id = match selection.primary() {
            Some(id) => id,
            None => return ToolResponse::None,
        };

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
            State::Creating { .. } => ToolResponse::Handled,
            State::Idle => ToolResponse::None,
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
            State::Idle => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));

        // Don't create a gradient for a zero-length drag.
        if (end.x - start.x).abs() < 0.5 && (end.y - start.y).abs() < 0.5 {
            return ToolResponse::None;
        }

        // Verify the target element still exists.
        if doc.get_node(element_id).is_none() {
            return ToolResponse::None;
        }

        // Capture old presentation attrs for undo.
        let old_pres = doc
            .get_node(element_id)
            .map(|n| n.presentation.clone())
            .unwrap_or_default();

        let cmd = ApplyGradientCommand::new(element_id, start, end, old_pres);
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

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// ApplyGradientCommand
// ---------------------------------------------------------------------------

/// A compound command that:
/// 1. Ensures a `<defs>` element exists under the document root.
/// 2. Creates a `<linearGradient>` with two `<stop>` children inside `<defs>`.
/// 3. Sets the target element's `fill` to reference the new gradient.
///
/// On undo the gradient nodes and (if created) the `<defs>` node are removed
/// and the target element's fill is restored.
#[derive(Debug)]
struct ApplyGradientCommand {
    /// The element whose fill will be changed.
    target_id: NodeId,
    /// Gradient start in document coordinates.
    start: Point,
    /// Gradient end in document coordinates.
    end: Point,
    /// Original presentation attrs of the target (for undo).
    old_pres: PresentationAttrs,

    // -- State filled on first execute --
    /// Whether we created the `<defs>` element (so we can remove it on undo).
    created_defs: bool,
    defs_id: Option<NodeId>,
    gradient_id: Option<NodeId>,
    gradient_xml_id: Option<String>,
    stop1_id: Option<NodeId>,
    stop2_id: Option<NodeId>,
}

impl ApplyGradientCommand {
    fn new(target_id: NodeId, start: Point, end: Point, old_pres: PresentationAttrs) -> Self {
        Self {
            target_id,
            start,
            end,
            old_pres,
            created_defs: false,
            defs_id: None,
            gradient_id: None,
            gradient_xml_id: None,
            stop1_id: None,
            stop2_id: None,
        }
    }

    /// Find an existing `<defs>` child of the root, or return `None`.
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

impl Command for ApplyGradientCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let root = match doc.root() {
            Some(r) => r,
            None => return,
        };

        // --- 1. Ensure <defs> exists ---
        let defs_id = if let Some(existing) = Self::find_defs(doc) {
            self.created_defs = false;
            existing
        } else {
            let id = if let Some(prev_id) = self.defs_id {
                // Re-execute (redo): reuse the same NodeId.
                let node = SvgNode::new(prev_id, SvgElement::Defs(DefsAttrs));
                doc.insert_node(node);
                doc.insert_child_at(root, prev_id, 0);
                prev_id
            } else {
                // First execute: create a new defs node.
                let id = doc.add_node(SvgElement::Defs(DefsAttrs));
                doc.insert_child_at(root, id, 0);
                id
            };
            self.created_defs = true;
            id
        };
        self.defs_id = Some(defs_id);

        // --- 2. Create the <linearGradient> ---
        let grad_xml_id = self.gradient_xml_id.clone().unwrap_or_else(|| {
            // Generate a short unique id for the SVG `id` attribute.
            let uuid = uuid_simple();
            format!("grad_{}", uuid)
        });

        let grad_element = SvgElement::LinearGradient(LinearGradientAttrs {
            x1: Some(SvgLength::new(self.start.x, LengthUnit::None)),
            y1: Some(SvgLength::new(self.start.y, LengthUnit::None)),
            x2: Some(SvgLength::new(self.end.x, LengthUnit::None)),
            y2: Some(SvgLength::new(self.end.y, LengthUnit::None)),
            gradient_units: Some(GradientUnits::UserSpaceOnUse),
            gradient_transform: None,
            spread_method: None,
            href: None,
        });

        let grad_node_id = if let Some(prev_id) = self.gradient_id {
            let mut node = SvgNode::new(prev_id, grad_element);
            node.xml_id = Some(grad_xml_id.clone());
            doc.insert_node(node);
            doc.append_child(defs_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(grad_element);
            if let Some(node) = doc.get_node_mut(id) {
                node.xml_id = Some(grad_xml_id.clone());
            }
            doc.append_child(defs_id, id);
            id
        };
        self.gradient_id = Some(grad_node_id);
        self.gradient_xml_id = Some(grad_xml_id.clone());

        // --- 3. Create two <stop> children ---
        let stop1_element = SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::BLACK),
            stop_opacity: Some(1.0),
            path: None,
        });
        let stop2_element = SvgElement::Stop(StopAttrs {
            offset: 1.0,
            stop_color: Some(Color::WHITE),
            stop_opacity: Some(1.0),
            path: None,
        });

        let stop1_id = if let Some(prev_id) = self.stop1_id {
            let node = SvgNode::new(prev_id, stop1_element);
            doc.insert_node(node);
            doc.append_child(grad_node_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(stop1_element);
            doc.append_child(grad_node_id, id);
            id
        };
        self.stop1_id = Some(stop1_id);

        let stop2_id = if let Some(prev_id) = self.stop2_id {
            let node = SvgNode::new(prev_id, stop2_element);
            doc.insert_node(node);
            doc.append_child(grad_node_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(stop2_element);
            doc.append_child(grad_node_id, id);
            id
        };
        self.stop2_id = Some(stop2_id);

        // --- 4. Set the target element's fill to reference the gradient ---
        if let Some(node) = doc.get_node_mut(self.target_id) {
            node.presentation.fill = Some(Paint::Url(format!("#{}", grad_xml_id)));
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Restore the target element's fill.
        if let Some(node) = doc.get_node_mut(self.target_id) {
            node.presentation = self.old_pres.clone();
        }

        // Remove the stop nodes.
        if let Some(id) = self.stop2_id {
            doc.remove_node(id);
        }
        if let Some(id) = self.stop1_id {
            doc.remove_node(id);
        }

        // Remove the gradient node.
        if let Some(id) = self.gradient_id {
            doc.remove_node(id);
        }

        // Remove the defs node if we created it.
        if self.created_defs {
            if let Some(id) = self.defs_id {
                doc.remove_node(id);
            }
        }
    }

    fn description(&self) -> &str {
        "Apply linear gradient"
    }
}

/// Generate a short pseudo-unique identifier (8 hex chars) without pulling in
/// the `uuid` crate directly.  We use `NodeId::new()` under the hood which
/// itself uses `uuid::Uuid::new_v4()`.
fn uuid_simple() -> String {
    let id = NodeId::new();
    // Take the first 8 hex chars of the UUID for brevity.
    let s = id.0.to_string().replace('-', "");
    s[..8.min(s.len())].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{RectAttrs, SvgSvgAttrs};

    /// Helper: create a minimal document with a root `<svg>` and a single
    /// `<rect>` element.  Returns `(doc, rect_id)`.
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
    fn apply_gradient_creates_defs_and_gradient() {
        let (mut doc, rect_id) = make_doc_with_rect();
        let root = doc.root().unwrap();

        let old_pres = doc.get_node(rect_id).unwrap().presentation.clone();
        let mut cmd = ApplyGradientCommand::new(
            rect_id,
            Point::new(10.0, 10.0),
            Point::new(90.0, 50.0),
            old_pres,
        );
        cmd.execute(&mut doc);

        // A <defs> should have been created as the first child of the root.
        let children = doc.children(root);
        assert!(children.len() >= 2, "root should have defs + rect");
        let defs_node = doc.get_node(children[0]).unwrap();
        assert_eq!(defs_node.element.tag_name(), "defs");

        // The <defs> should contain one <linearGradient>.
        let defs_children = doc.children(children[0]);
        assert_eq!(defs_children.len(), 1);
        let grad_node = doc.get_node(defs_children[0]).unwrap();
        assert_eq!(grad_node.element.tag_name(), "linearGradient");
        assert!(grad_node.xml_id.is_some(), "gradient should have an xml_id");

        // The gradient should have two <stop> children.
        let grad_children = doc.children(defs_children[0]);
        assert_eq!(grad_children.len(), 2);
        assert_eq!(
            doc.get_node(grad_children[0]).unwrap().element.tag_name(),
            "stop"
        );
        assert_eq!(
            doc.get_node(grad_children[1]).unwrap().element.tag_name(),
            "stop"
        );

        // The rect's fill should now be a url() reference.
        let rect_node = doc.get_node(rect_id).unwrap();
        match &rect_node.presentation.fill {
            Some(Paint::Url(url)) => {
                assert!(
                    url.starts_with("#grad_"),
                    "fill url should start with #grad_"
                );
            }
            other => panic!("expected Paint::Url, got {:?}", other),
        }
    }

    #[test]
    fn undo_restores_original_fill_and_removes_gradient() {
        let (mut doc, rect_id) = make_doc_with_rect();

        let old_fill = Paint::Color(Color::RED);
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(old_fill.clone());
        }

        let old_pres = doc.get_node(rect_id).unwrap().presentation.clone();
        let mut cmd = ApplyGradientCommand::new(
            rect_id,
            Point::new(0.0, 0.0),
            Point::new(100.0, 100.0),
            old_pres,
        );
        cmd.execute(&mut doc);

        // Verify gradient was applied.
        assert!(matches!(
            doc.get_node(rect_id).unwrap().presentation.fill,
            Some(Paint::Url(_))
        ));

        // Undo.
        cmd.undo(&mut doc);

        // Fill should be restored.
        assert_eq!(
            doc.get_node(rect_id).unwrap().presentation.fill,
            Some(old_fill)
        );

        // The defs should have been removed (we created it).
        let root = doc.root().unwrap();
        let children = doc.children(root);
        for &child_id in children {
            let node = doc.get_node(child_id).unwrap();
            assert_ne!(
                node.element.tag_name(),
                "defs",
                "defs should have been removed on undo"
            );
        }
    }

    #[test]
    fn gradient_tool_ignores_when_no_selection() {
        let mut tool = GradientTool::new();
        let doc = SvgDocument::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let response = tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        assert!(matches!(response, ToolResponse::None));
    }
}
