//! Select tool: click to select, drag to move, rubber-band select.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::{Point, Rect, Transform2D};

use crate::command::Command;
use crate::commands::TransformCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// The select tool state.
#[allow(dead_code)]
#[derive(Debug)]
enum SelectState {
    Idle,
    /// We pressed down on an object; if the user moves far enough we start
    /// dragging, otherwise it is a click-to-select.
    PressedOnObject {
        node_id: NodeId,
        start_doc: Point,
        start_screen: Point,
    },
    /// Dragging the selected objects.
    Dragging {
        start_doc: Point,
        last_doc: Point,
        /// The old transforms of all dragged nodes.
        old_transforms: Vec<(NodeId, Option<Transform2D>)>,
    },
    /// Rubber-band selection.
    RubberBand {
        start_screen: Point,
    },
}

/// The select tool.
#[derive(Debug)]
pub struct SelectTool {
    state: SelectState,
    drag_threshold: f64,
}

impl SelectTool {
    pub fn new() -> Self {
        Self {
            state: SelectState::Idle,
            drag_threshold: 3.0,
        }
    }
}

impl Default for SelectTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SelectTool {
    fn name(&self) -> &str {
        "Select"
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

        // Hit test against all leaf elements.
        if let Some(hit_id) = hit_test_at(doc, doc_pt) {
            self.state = SelectState::PressedOnObject {
                node_id: hit_id,
                start_doc: doc_pt,
                start_screen: Point::new(event.x, event.y),
            };
        } else {
            // Start rubber band.
            self.state = SelectState::RubberBand {
                start_screen: Point::new(event.x, event.y),
            };
        }

        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        match &mut self.state {
            SelectState::PressedOnObject {
                node_id: _,
                start_doc,
                start_screen,
            } => {
                let dx = event.x - start_screen.x;
                let dy = event.y - start_screen.y;
                if (dx * dx + dy * dy).sqrt() >= self.drag_threshold {
                    // Transition to dragging.
                    let old_transforms: Vec<(NodeId, Option<Transform2D>)> = selection
                        .selected()
                        .iter()
                        .map(|&id| {
                            let t = doc.get_node(id).and_then(|n| n.presentation.transform);
                            (id, t)
                        })
                        .collect();

                    self.state = SelectState::Dragging {
                        start_doc: *start_doc,
                        last_doc: doc_pt,
                        old_transforms,
                    };
                }
                ToolResponse::Handled
            }
            SelectState::Dragging {
                start_doc: _,
                last_doc,
                ..
            } => {
                *last_doc = doc_pt;
                ToolResponse::Handled
            }
            SelectState::RubberBand { .. } => ToolResponse::Handled,
            SelectState::Idle => ToolResponse::None,
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let state = std::mem::replace(&mut self.state, SelectState::Idle);
        let _doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        match state {
            SelectState::PressedOnObject { node_id: _, .. } => {
                // It was a click (not a drag).  Selection logic will be
                // handled by the engine based on modifiers.
                // We return a special "select this" response: we encode it
                // by returning None and relying on the engine to inspect the
                // hit test + modifiers.  For simplicity, the engine calls
                // `select_at` after dispatching to the tool.
                //
                // Actually, let's handle selection directly here by returning
                // Handled and letting the engine check what happened.
                ToolResponse::Handled
            }
            SelectState::Dragging {
                start_doc,
                last_doc,
                old_transforms,
            } => {
                let delta_x = last_doc.x - start_doc.x;
                let delta_y = last_doc.y - start_doc.y;

                if delta_x.abs() < 0.001 && delta_y.abs() < 0.001 {
                    return ToolResponse::Handled;
                }

                // Build a compound command for all dragged nodes.
                let mut commands: Vec<Box<dyn Command>> = Vec::new();
                for (id, old_t) in &old_transforms {
                    let base = old_t.unwrap_or(Transform2D::IDENTITY);
                    let new_t = base.then(&Transform2D::translate(delta_x, delta_y));
                    commands.push(Box::new(TransformCommand::new(*id, *old_t, Some(new_t))));
                }

                if commands.len() == 1 {
                    ToolResponse::ExecuteCommand(commands.remove(0))
                } else if commands.len() > 1 {
                    ToolResponse::ExecuteCommand(Box::new(crate::commands::CompoundCommand::new(
                        "Move objects",
                        commands,
                    )))
                } else {
                    ToolResponse::Handled
                }
            }
            SelectState::RubberBand { start_screen: _ } => {
                // Rubber-band: select all objects within the rectangle.
                // The engine will handle the actual selection update.
                ToolResponse::Handled
            }
            SelectState::Idle => ToolResponse::None,
        }
    }

    fn on_key_down(
        &mut self,
        _event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.state = SelectState::Idle;
    }

    fn on_deactivate(&mut self) {
        self.state = SelectState::Idle;
    }

    fn cursor(&self) -> &str {
        "default"
    }
}

/// Simple hit-test that iterates over the document and finds the front-most
/// element whose bounding box contains the point.
///
/// Skips locked and hidden objects so they cannot be interactively selected.
///
/// Returns `None` if nothing is hit.
pub fn hit_test_at(doc: &SvgDocument, point: Point) -> Option<NodeId> {
    let root = doc.root()?;
    let all = doc.traverse_pre_order(root);

    // We want the *last* match in paint order (front-most).
    let mut result = None;
    for id in all {
        let node = match doc.get_node(id) {
            Some(n) => n,
            None => continue,
        };

        // Skip locked or hidden objects.
        if node.locked || node.hidden {
            continue;
        }

        // Skip non-leaf elements.
        match &node.element {
            SvgElement::Svg(_) | SvgElement::Group(_) | SvgElement::Defs(_) => continue,
            _ => {}
        }

        if let Some(bb) = element_bounding_box_for_hit(&node.element) {
            if bb.contains_point(point) {
                result = Some(id);
            }
        }
    }
    result
}

/// Bounding box for hit testing.
fn element_bounding_box_for_hit(element: &SvgElement) -> Option<Rect> {
    match element {
        SvgElement::Rect(a) => Some(Rect::new(
            a.x.value,
            a.y.value,
            a.x.value + a.width.value,
            a.y.value + a.height.value,
        )),
        SvgElement::Circle(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let r = a.r.value;
            Some(Rect::new(cx - r, cy - r, cx + r, cy + r))
        }
        SvgElement::Ellipse(a) => {
            let cx = a.cx.value;
            let cy = a.cy.value;
            let rx = a.rx.value;
            let ry = a.ry.value;
            Some(Rect::new(cx - rx, cy - ry, cx + rx, cy + ry))
        }
        SvgElement::Line(a) => {
            // Inflate a bit so thin lines are clickable.
            let x0 = a.x1.value.min(a.x2.value) - 2.0;
            let y0 = a.y1.value.min(a.y2.value) - 2.0;
            let x1 = a.x1.value.max(a.x2.value) + 2.0;
            let y1 = a.y1.value.max(a.y2.value) + 2.0;
            Some(Rect::new(x0, y0, x1, y1))
        }
        SvgElement::Path(a) => {
            let bb = a.d.bounding_box();
            if bb.is_empty() {
                None
            } else {
                Some(bb)
            }
        }
        _ => None,
    }
}
