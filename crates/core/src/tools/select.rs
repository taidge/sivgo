//! Select tool: click to select, drag to move, rubber-band select.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::{Point, Rect, Transform2D};

use crate::command::Command;
use crate::commands::TransformCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::{element_bounding_box, node_transform, SelectionSet};
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
        let release_doc = viewport.screen_to_document(Point::new(event.x, event.y));

        match state {
            SelectState::PressedOnObject { .. } => {
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
                last_doc: _,
                old_transforms,
            } => {
                let delta_x = release_doc.x - start_doc.x;
                let delta_y = release_doc.y - start_doc.y;

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

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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

        if let Some(bb) = element_bounding_box_for_hit(doc, id, &node.element) {
            if bb.contains_point(point) {
                result = Some(selectable_hit_id(doc, id));
            }
        }
    }
    result
}

fn selectable_hit_id(doc: &SvgDocument, id: NodeId) -> NodeId {
    let mut current = id;
    while let Some(parent_id) = doc.parent(current) {
        if doc
            .get_node(parent_id)
            .is_some_and(|node| matches!(node.element, SvgElement::Box3d(_)))
        {
            return parent_id;
        }
        current = parent_id;
    }
    id
}

/// Bounding box for hit testing.
fn element_bounding_box_for_hit(
    doc: &SvgDocument,
    id: NodeId,
    element: &SvgElement,
) -> Option<Rect> {
    let bb = element_bounding_box(element)?;
    if bb.is_empty() {
        return None;
    }

    let local_bb = match element {
        // Inflate thin paths and lines so a visible stroke is not a one-pixel target.
        SvgElement::Line(_) | SvgElement::Polyline(_) | SvgElement::Path(_) => {
            inflate_rect(bb, 2.0)
        }
        _ => bb,
    };

    Some(node_transform(doc, id).transform_rect(local_bb))
}

fn inflate_rect(rect: Rect, amount: f64) -> Rect {
    Rect::new(
        rect.x0 - amount,
        rect.y0 - amount,
        rect.x1 + amount,
        rect.y1 + amount,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{
        Box3dAttrs, LengthUnit, PathAttrs, RectAttrs, SvgDocument, SvgElement, SvgLength,
        SvgSvgAttrs,
    };
    use sivgo_math::BezPath;

    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let root = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(root);
        doc
    }

    #[test]
    fn hit_test_uses_shared_selection_bounds_for_drawable_elements() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        let polygon = doc.add_node(SvgElement::Polygon(sivgo_dom::PolygonAttrs {
            points: vec![(10.0, 10.0), (60.0, 10.0), (60.0, 40.0)],
        }));
        doc.append_child(root, polygon);

        assert_eq!(hit_test_at(&doc, Point::new(30.0, 20.0)), Some(polygon));
    }

    #[test]
    fn hit_test_maps_box3d_face_clicks_to_box_container() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        let box_id = doc.add_node(SvgElement::Box3d(Box3dAttrs {
            corner0: Some("10,50".to_string()),
            corner1: Some("70,50".to_string()),
            corner2: Some("70,10".to_string()),
            corner3: Some("10,10".to_string()),
            corner4: Some("25,65".to_string()),
            corner5: Some("85,65".to_string()),
            corner6: Some("85,25".to_string()),
            corner7: Some("25,25".to_string()),
            ..Default::default()
        }));
        doc.append_child(root, box_id);

        let mut face = BezPath::new();
        face.move_to(Point::new(10.0, 50.0));
        face.line_to(Point::new(70.0, 50.0));
        face.line_to(Point::new(70.0, 10.0));
        face.line_to(Point::new(10.0, 10.0));
        face.close();
        let face_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: face,
            ..Default::default()
        }));
        doc.append_child(box_id, face_id);

        assert_eq!(hit_test_at(&doc, Point::new(30.0, 30.0)), Some(box_id));
    }

    #[test]
    fn frontmost_plain_element_still_wins() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        let back = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(0.0, LengthUnit::None),
            y: SvgLength::new(0.0, LengthUnit::None),
            width: SvgLength::new(100.0, LengthUnit::None),
            height: SvgLength::new(100.0, LengthUnit::None),
            ..Default::default()
        }));
        let front = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(20.0, LengthUnit::None),
            y: SvgLength::new(20.0, LengthUnit::None),
            width: SvgLength::new(40.0, LengthUnit::None),
            height: SvgLength::new(40.0, LengthUnit::None),
            ..Default::default()
        }));
        doc.append_child(root, back);
        doc.append_child(root, front);

        assert_eq!(hit_test_at(&doc, Point::new(30.0, 30.0)), Some(front));
    }

    #[test]
    fn hit_test_accounts_for_element_transform() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();
        let rect = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(0.0, LengthUnit::None),
            y: SvgLength::new(0.0, LengthUnit::None),
            width: SvgLength::new(20.0, LengthUnit::None),
            height: SvgLength::new(20.0, LengthUnit::None),
            ..Default::default()
        }));
        doc.get_node_mut(rect).unwrap().presentation.transform =
            Some(Transform2D::translate(80.0, 30.0));
        doc.append_child(root, rect);

        assert_eq!(hit_test_at(&doc, Point::new(90.0, 40.0)), Some(rect));
        assert_eq!(hit_test_at(&doc, Point::new(10.0, 10.0)), None);
    }
}
