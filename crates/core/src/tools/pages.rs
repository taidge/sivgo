//! Pages tool: add, edit, and remove `<inkscape:page>` elements.
//!
//! States:
//! - **Idle**: waiting for input.
//! - **Creating**: user is dragging to define a new page rectangle.
//! - **Moving**: user is dragging an existing page to a new position.
//! - **Resizing**: user is dragging the edge/corner of an existing page.
//!
//! On double-click on empty area: create a new default-size page at that
//! position.
//! On click on an existing page border: select for move/resize.
//! On drag in Creating mode: define the page rectangle.

use sivgo_dom::{InkscapePageAttrs, NamedViewAttrs, NodeId, SvgDocument, SvgElement, SvgNode};
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
    /// Dragging to define a new page rectangle.
    Creating {
        start: Point,
    },
    /// Moving an existing page.
    Moving {
        page_id: NodeId,
        offset: Point,
        original_x: f64,
        original_y: f64,
    },
    /// Resizing an existing page.
    Resizing {
        page_id: NodeId,
        anchor: Point,
        original_w: f64,
        original_h: f64,
    },
}

// ---------------------------------------------------------------------------
// PagesTool
// ---------------------------------------------------------------------------

/// Interactive tool for managing Inkscape multi-page documents.
///
/// Allows creating, moving, and resizing `<inkscape:page>` elements
/// within the `<sodipodi:namedview>` container.
#[derive(Debug)]
pub struct PagesTool {
    state: State,
    /// Counter for labelling newly created pages.
    page_counter: u32,
}

impl PagesTool {
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            page_counter: 1,
        }
    }

    /// Find the `<sodipodi:namedview>` element under the root.
    fn find_named_view(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        for &child_id in doc.children(root) {
            if let Some(node) = doc.get_node(child_id) {
                if matches!(node.element, SvgElement::SodipodiNamedView(_)) {
                    return Some(child_id);
                }
            }
        }
        None
    }

    /// Find a page at the given document point by checking page bounds.
    ///
    /// Returns the page NodeId if the point falls on a page border (within
    /// a tolerance).
    fn hit_test_page_border(doc: &SvgDocument, pt: Point, tolerance: f64) -> Option<NodeId> {
        let nv_id = Self::find_named_view(doc)?;
        for &child_id in doc.children(nv_id) {
            if let Some(node) = doc.get_node(child_id) {
                if let SvgElement::InkscapePage(attrs) = &node.element {
                    let x: f64 = attrs
                        .x
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let y: f64 = attrs
                        .y
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let w: f64 = attrs
                        .width
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let h: f64 = attrs
                        .height
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);

                    // Check if the point is near any edge of the page.
                    let on_left = (pt.x - x).abs() < tolerance
                        && pt.y >= y - tolerance
                        && pt.y <= y + h + tolerance;
                    let on_right = (pt.x - (x + w)).abs() < tolerance
                        && pt.y >= y - tolerance
                        && pt.y <= y + h + tolerance;
                    let on_top = (pt.y - y).abs() < tolerance
                        && pt.x >= x - tolerance
                        && pt.x <= x + w + tolerance;
                    let on_bottom = (pt.y - (y + h)).abs() < tolerance
                        && pt.x >= x - tolerance
                        && pt.x <= x + w + tolerance;

                    if on_left || on_right || on_top || on_bottom {
                        return Some(child_id);
                    }
                }
            }
        }
        None
    }

    /// Check whether the point is inside any existing page rectangle.
    #[allow(dead_code)]
    fn hit_test_page_interior(doc: &SvgDocument, pt: Point) -> Option<NodeId> {
        let nv_id = Self::find_named_view(doc)?;
        for &child_id in doc.children(nv_id) {
            if let Some(node) = doc.get_node(child_id) {
                if let SvgElement::InkscapePage(attrs) = &node.element {
                    let x: f64 = attrs
                        .x
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let y: f64 = attrs
                        .y
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let w: f64 = attrs
                        .width
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let h: f64 = attrs
                        .height
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);

                    if pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h {
                        return Some(child_id);
                    }
                }
            }
        }
        None
    }
}

impl Default for PagesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for PagesTool {
    fn name(&self) -> &str {
        "Pages"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        let border_tolerance = 5.0 / viewport.zoom;

        // Check if clicking on a page border first (for move/resize).
        if let Some(page_id) = Self::hit_test_page_border(doc, doc_pt, border_tolerance) {
            if let Some(node) = doc.get_node(page_id) {
                if let SvgElement::InkscapePage(attrs) = &node.element {
                    let x: f64 = attrs
                        .x
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let y: f64 = attrs
                        .y
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let w: f64 = attrs
                        .width
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let h: f64 = attrs
                        .height
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);

                    // If near a corner/edge for resize, or interior for move.
                    let near_right = (doc_pt.x - (x + w)).abs() < border_tolerance;
                    let near_bottom = (doc_pt.y - (y + h)).abs() < border_tolerance;

                    if near_right || near_bottom {
                        self.state = State::Resizing {
                            page_id,
                            anchor: Point::new(x, y),
                            original_w: w,
                            original_h: h,
                        };
                    } else {
                        self.state = State::Moving {
                            page_id,
                            offset: Point::new(doc_pt.x - x, doc_pt.y - y),
                            original_x: x,
                            original_y: y,
                        };
                    }
                    return ToolResponse::Handled;
                }
            }
        }

        // Otherwise start creating a new page.
        self.state = State::Creating { start: doc_pt };
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
            _ => ToolResponse::Handled,
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let end = viewport.screen_to_document(Point::new(event.x, event.y));

        match std::mem::replace(&mut self.state, State::Idle) {
            State::Creating { start } => {
                let x = start.x.min(end.x);
                let y = start.y.min(end.y);
                let w = (end.x - start.x).abs();
                let h = (end.y - start.y).abs();

                // If the drag was too small, create a default-size page.
                let (w, h) = if w < 5.0 && h < 5.0 {
                    (210.0, 297.0) // A4-ish default
                } else {
                    (w, h)
                };

                let label = format!("Page {}", self.page_counter);
                self.page_counter += 1;

                let cmd = AddPageCommand::new(x, y, w, h, label);
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            State::Moving {
                page_id,
                offset,
                original_x,
                original_y,
            } => {
                let new_x = end.x - offset.x;
                let new_y = end.y - offset.y;

                if (new_x - original_x).abs() < 0.5 && (new_y - original_y).abs() < 0.5 {
                    return ToolResponse::None;
                }

                let cmd = MovePageCommand::new(page_id, original_x, original_y, new_x, new_y);
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            State::Resizing {
                page_id,
                anchor,
                original_w,
                original_h,
            } => {
                let new_w = (end.x - anchor.x).abs().max(10.0);
                let new_h = (end.y - anchor.y).abs().max(10.0);

                if (new_w - original_w).abs() < 0.5 && (new_h - original_h).abs() < 0.5 {
                    return ToolResponse::None;
                }

                let cmd = ResizePageCommand::new(page_id, original_w, original_h, new_w, new_h);
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            State::Idle => ToolResponse::None,
        }
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
        "default"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// AddPageCommand
// ---------------------------------------------------------------------------

/// Adds a new `<inkscape:page>` element inside `<sodipodi:namedview>`.
///
/// If no `<sodipodi:namedview>` exists, one is created under the root.
#[derive(Debug)]
struct AddPageCommand {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    label: String,

    created_named_view: bool,
    named_view_id: Option<NodeId>,
    page_id: Option<NodeId>,
}

impl AddPageCommand {
    fn new(x: f64, y: f64, width: f64, height: f64, label: String) -> Self {
        Self {
            x,
            y,
            width,
            height,
            label,
            created_named_view: false,
            named_view_id: None,
            page_id: None,
        }
    }

    fn find_named_view(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        for &child_id in doc.children(root) {
            if let Some(node) = doc.get_node(child_id) {
                if matches!(node.element, SvgElement::SodipodiNamedView(_)) {
                    return Some(child_id);
                }
            }
        }
        None
    }
}

impl Command for AddPageCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let root = match doc.root() {
            Some(r) => r,
            None => return,
        };

        // Ensure <sodipodi:namedview> exists.
        let nv_id = if let Some(existing) = Self::find_named_view(doc) {
            self.created_named_view = false;
            existing
        } else {
            let id = if let Some(prev_id) = self.named_view_id {
                let node = SvgNode::new(
                    prev_id,
                    SvgElement::SodipodiNamedView(NamedViewAttrs::default()),
                );
                doc.insert_node(node);
                doc.insert_child_at(root, prev_id, 0);
                prev_id
            } else {
                let id = doc.add_node(SvgElement::SodipodiNamedView(NamedViewAttrs::default()));
                doc.insert_child_at(root, id, 0);
                id
            };
            self.created_named_view = true;
            id
        };
        self.named_view_id = Some(nv_id);

        // Create <inkscape:page>.
        let page_element = SvgElement::InkscapePage(InkscapePageAttrs {
            x: Some(format!("{}", self.x)),
            y: Some(format!("{}", self.y)),
            width: Some(format!("{}", self.width)),
            height: Some(format!("{}", self.height)),
            inkscape_label: Some(self.label.clone()),
        });

        let page_node_id = if let Some(prev_id) = self.page_id {
            let node = SvgNode::new(prev_id, page_element);
            doc.insert_node(node);
            doc.append_child(nv_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(page_element);
            doc.append_child(nv_id, id);
            id
        };
        self.page_id = Some(page_node_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let Some(id) = self.page_id {
            doc.remove_node(id);
        }
        if self.created_named_view {
            if let Some(id) = self.named_view_id {
                doc.remove_node(id);
            }
        }
    }

    fn description(&self) -> &str {
        "Add page"
    }
}

// ---------------------------------------------------------------------------
// MovePageCommand
// ---------------------------------------------------------------------------

/// Moves a page by updating its x/y attributes.
#[derive(Debug)]
struct MovePageCommand {
    page_id: NodeId,
    old_x: f64,
    old_y: f64,
    new_x: f64,
    new_y: f64,
}

impl MovePageCommand {
    fn new(page_id: NodeId, old_x: f64, old_y: f64, new_x: f64, new_y: f64) -> Self {
        Self {
            page_id,
            old_x,
            old_y,
            new_x,
            new_y,
        }
    }

    fn set_position(doc: &mut SvgDocument, page_id: NodeId, x: f64, y: f64) {
        if let Some(node) = doc.get_node_mut(page_id) {
            if let SvgElement::InkscapePage(ref mut attrs) = node.element {
                attrs.x = Some(format!("{}", x));
                attrs.y = Some(format!("{}", y));
            }
        }
    }
}

impl Command for MovePageCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::set_position(doc, self.page_id, self.new_x, self.new_y);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::set_position(doc, self.page_id, self.old_x, self.old_y);
    }

    fn description(&self) -> &str {
        "Move page"
    }
}

// ---------------------------------------------------------------------------
// ResizePageCommand
// ---------------------------------------------------------------------------

/// Resizes a page by updating its width/height attributes.
#[derive(Debug)]
struct ResizePageCommand {
    page_id: NodeId,
    old_w: f64,
    old_h: f64,
    new_w: f64,
    new_h: f64,
}

impl ResizePageCommand {
    fn new(page_id: NodeId, old_w: f64, old_h: f64, new_w: f64, new_h: f64) -> Self {
        Self {
            page_id,
            old_w,
            old_h,
            new_w,
            new_h,
        }
    }

    fn set_size(doc: &mut SvgDocument, page_id: NodeId, w: f64, h: f64) {
        if let Some(node) = doc.get_node_mut(page_id) {
            if let SvgElement::InkscapePage(ref mut attrs) = node.element {
                attrs.width = Some(format!("{}", w));
                attrs.height = Some(format!("{}", h));
            }
        }
    }
}

impl Command for ResizePageCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        Self::set_size(doc, self.page_id, self.new_w, self.new_h);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        Self::set_size(doc, self.page_id, self.old_w, self.old_h);
    }

    fn description(&self) -> &str {
        "Resize page"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::SvgSvgAttrs;

    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        doc
    }

    fn make_doc_with_named_view() -> (SvgDocument, NodeId) {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        let nv_id = doc.add_node(SvgElement::SodipodiNamedView(NamedViewAttrs::default()));
        doc.append_child(root, nv_id);
        (doc, nv_id)
    }

    #[test]
    fn add_page_creates_named_view_and_page() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();

        let mut cmd = AddPageCommand::new(0.0, 0.0, 210.0, 297.0, "Page 1".to_string());
        cmd.execute(&mut doc);

        // A <sodipodi:namedview> should have been created.
        let children = doc.children(root);
        assert!(!children.is_empty());
        let nv = doc.get_node(children[0]).unwrap();
        assert_eq!(nv.element.tag_name(), "sodipodi:namedview");

        // The namedview should contain one page.
        let nv_children = doc.children(children[0]);
        assert_eq!(nv_children.len(), 1);
        let page = doc.get_node(nv_children[0]).unwrap();
        assert_eq!(page.element.tag_name(), "inkscape:page");

        match &page.element {
            SvgElement::InkscapePage(attrs) => {
                assert_eq!(attrs.x.as_deref(), Some("0"));
                assert_eq!(attrs.width.as_deref(), Some("210"));
                assert_eq!(attrs.height.as_deref(), Some("297"));
                assert_eq!(attrs.inkscape_label.as_deref(), Some("Page 1"));
            }
            _ => panic!("expected InkscapePage"),
        }
    }

    #[test]
    fn undo_add_page_removes_named_view_if_created() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();

        let mut cmd = AddPageCommand::new(0.0, 0.0, 100.0, 100.0, "Test".to_string());
        cmd.execute(&mut doc);
        assert!(!doc.children(root).is_empty());

        cmd.undo(&mut doc);
        // If we created the namedview, it should be removed too.
        assert!(doc.children(root).is_empty());
    }

    #[test]
    fn move_page_updates_position() {
        let (mut doc, nv_id) = make_doc_with_named_view();

        let page_id = doc.add_node(SvgElement::InkscapePage(InkscapePageAttrs {
            x: Some("10".to_string()),
            y: Some("20".to_string()),
            width: Some("100".to_string()),
            height: Some("100".to_string()),
            inkscape_label: None,
        }));
        doc.append_child(nv_id, page_id);

        let mut cmd = MovePageCommand::new(page_id, 10.0, 20.0, 50.0, 60.0);
        cmd.execute(&mut doc);

        let node = doc.get_node(page_id).unwrap();
        if let SvgElement::InkscapePage(attrs) = &node.element {
            assert_eq!(attrs.x.as_deref(), Some("50"));
            assert_eq!(attrs.y.as_deref(), Some("60"));
        }

        cmd.undo(&mut doc);
        let node = doc.get_node(page_id).unwrap();
        if let SvgElement::InkscapePage(attrs) = &node.element {
            assert_eq!(attrs.x.as_deref(), Some("10"));
            assert_eq!(attrs.y.as_deref(), Some("20"));
        }
    }

    #[test]
    fn resize_page_updates_dimensions() {
        let (mut doc, nv_id) = make_doc_with_named_view();

        let page_id = doc.add_node(SvgElement::InkscapePage(InkscapePageAttrs {
            x: Some("0".to_string()),
            y: Some("0".to_string()),
            width: Some("100".to_string()),
            height: Some("100".to_string()),
            inkscape_label: None,
        }));
        doc.append_child(nv_id, page_id);

        let mut cmd = ResizePageCommand::new(page_id, 100.0, 100.0, 200.0, 150.0);
        cmd.execute(&mut doc);

        let node = doc.get_node(page_id).unwrap();
        if let SvgElement::InkscapePage(attrs) = &node.element {
            assert_eq!(attrs.width.as_deref(), Some("200"));
            assert_eq!(attrs.height.as_deref(), Some("150"));
        }

        cmd.undo(&mut doc);
        let node = doc.get_node(page_id).unwrap();
        if let SvgElement::InkscapePage(attrs) = &node.element {
            assert_eq!(attrs.width.as_deref(), Some("100"));
            assert_eq!(attrs.height.as_deref(), Some("100"));
        }
    }

    #[test]
    fn pages_tool_ignores_right_click() {
        let mut tool = PagesTool::new();
        let doc = make_doc();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let event = MouseEvent {
            x: 100.0,
            y: 100.0,
            button: MouseButton::Right,
            modifiers: crate::input::Modifiers::none(),
        };
        let resp = tool.on_mouse_down(event, &doc, &sel, &vp);
        assert!(matches!(resp, ToolResponse::None));
    }
}
