//! Ellipse tool: click-drag to create an ellipse or circle.

use sivgo_dom::{CircleAttrs, EllipseAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::Point;

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
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
        let circle = event.modifiers.ctrl;

        let dx = (end.x - start.x).abs();
        let dy = (end.y - start.y).abs();

        if dx < 0.5 && dy < 0.5 {
            return ToolResponse::None;
        }

        let cx = (start.x + end.x) / 2.0;
        let cy = (start.y + end.y) / 2.0;
        let rx = dx / 2.0;
        let ry = dy / 2.0;

        let element = if circle {
            let r = rx.max(ry);
            SvgElement::Circle(CircleAttrs {
                cx: SvgLength::new(cx, LengthUnit::None),
                cy: SvgLength::new(cy, LengthUnit::None),
                r: SvgLength::new(r, LengthUnit::None),
            })
        } else {
            SvgElement::Ellipse(EllipseAttrs {
                cx: SvgLength::new(cx, LengthUnit::None),
                cy: SvgLength::new(cy, LengthUnit::None),
                rx: SvgLength::new(rx, LengthUnit::None),
                ry: SvgLength::new(ry, LengthUnit::None),
            })
        };

        let mut pres = PresentationAttrs::default();
        pres.fill = Some(Paint::Color(Color::rgb(100, 149, 237)));
        pres.stroke = Some(Paint::Color(Color::BLACK));
        pres.stroke_width = Some(SvgLength::new(1.0, LengthUnit::None));

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
