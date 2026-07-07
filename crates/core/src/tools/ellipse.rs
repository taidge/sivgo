//! Ellipse tool: click-drag to create an ellipse or circle.

use sivgo_dom::{CircleAttrs, EllipseAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::Point;

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::drag::drag_rect;
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// The ellipse/circle creation tool.
#[derive(Debug)]
pub struct EllipseTool {
    start: Option<Point>,
    dragging: bool,
}

impl EllipseTool {
    pub fn new() -> Self {
        Self {
            start: None,
            dragging: false,
        }
    }
}

impl Default for EllipseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for EllipseTool {
    fn name(&self) -> &str {
        "Ellipse"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }
        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        self.start = Some(doc_pt);
        self.dragging = true;
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        if self.dragging {
            ToolResponse::Handled
        } else {
            ToolResponse::None
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.dragging {
            return ToolResponse::None;
        }
        self.dragging = false;

        let start = match self.start.take() {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let rect = drag_rect(start, end, event.modifiers.ctrl, event.modifiers.shift);

        if rect.width() < 0.5 && rect.height() < 0.5 {
            return ToolResponse::None;
        }

        let center = rect.center();
        let rx = rect.width() / 2.0;
        let ry = rect.height() / 2.0;

        let element = if event.modifiers.ctrl {
            let r = rx.max(ry);
            SvgElement::Circle(CircleAttrs {
                cx: SvgLength::new(center.x, LengthUnit::None),
                cy: SvgLength::new(center.y, LengthUnit::None),
                r: SvgLength::new(r, LengthUnit::None),
            })
        } else {
            SvgElement::Ellipse(EllipseAttrs {
                cx: SvgLength::new(center.x, LengthUnit::None),
                cy: SvgLength::new(center.y, LengthUnit::None),
                rx: SvgLength::new(rx, LengthUnit::None),
                ry: SvgLength::new(ry, LengthUnit::None),
            })
        };

        let pres = PresentationAttrs {
            fill: Some(Paint::Color(Color::rgb(100, 149, 237))),
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(SvgLength::new(1.0, LengthUnit::None)),
            ..Default::default()
        };

        let parent = match doc.root() {
            Some(r) => r,
            None => return ToolResponse::None,
        };

        let cmd = AddNodeCommand::new(parent, element, pres, None);
        ToolResponse::ExecuteCommand(Box::new(cmd))
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
        self.start = None;
        self.dragging = false;
    }

    fn on_deactivate(&mut self) {
        self.start = None;
        self.dragging = false;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;
    use sivgo_dom::SvgSvgAttrs;

    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        doc
    }

    fn left_with_mods(x: f64, y: f64, ctrl: bool, shift: bool) -> MouseEvent {
        MouseEvent {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers {
                ctrl,
                shift,
                alt: false,
            },
        }
    }

    #[test]
    fn ellipse_ctrl_shift_matches_constrained_drag_bounds() {
        let doc = make_doc();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);
        let mut tool = EllipseTool::new();

        tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        let resp = tool.on_mouse_up(left_with_mods(120.0, 110.0, true, true), &doc, &sel, &vp);

        match resp {
            ToolResponse::ExecuteCommand(mut cmd) => {
                let mut doc = doc;
                cmd.execute(&mut doc);
                let root = doc.root().unwrap();
                let child = doc.children(root)[0];
                let node = doc.get_node(child).unwrap();

                match &node.element {
                    SvgElement::Circle(attrs) => {
                        assert!((attrs.cx.value - 100.0).abs() < 1e-9);
                        assert!((attrs.cy.value - 100.0).abs() < 1e-9);
                        assert!((attrs.r.value - 20.0).abs() < 1e-9);
                    }
                    _ => panic!("expected constrained ellipse drag to create a circle"),
                }
            }
            _ => panic!("expected ExecuteCommand response from EllipseTool"),
        }
    }
}
