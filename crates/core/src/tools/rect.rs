//! Rect tool: click-drag to create a rectangle.

use sivgo_dom::{PresentationAttrs, RectAttrs, SvgDocument, SvgElement};
use sivgo_math::{Point, Rect};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// The rect creation tool.
#[derive(Debug)]
pub struct RectTool {
    /// Start point in document space.
    start: Option<Point>,
    /// Whether we are actively dragging.
    dragging: bool,
}

impl RectTool {
    pub fn new() -> Self {
        Self {
            start: None,
            dragging: false,
        }
    }
}

impl Default for RectTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for RectTool {
    fn name(&self) -> &str {
        "Rect"
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
        let rect = compute_rect(start, end, event.modifiers.ctrl, event.modifiers.shift);

        // Don't create zero-sized rects.
        if rect.width() < 0.5 && rect.height() < 0.5 {
            return ToolResponse::None;
        }

        let element = SvgElement::Rect(RectAttrs {
            x: SvgLength::new(rect.x0, LengthUnit::None),
            y: SvgLength::new(rect.y0, LengthUnit::None),
            width: SvgLength::new(rect.width(), LengthUnit::None),
            height: SvgLength::new(rect.height(), LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        });

        let mut pres = PresentationAttrs::default();
        pres.fill = Some(Paint::Color(Color::rgb(100, 149, 237))); // cornflower blue
        pres.stroke = Some(Paint::Color(Color::BLACK));
        pres.stroke_width = Some(SvgLength::new(1.0, LengthUnit::None));

        // Find parent: use the root element.
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

/// Compute rectangle from two corner points, with optional constraints.
///
/// - `square`: constrain to square (Ctrl modifier)
/// - `from_center`: draw from center (Shift modifier)
fn compute_rect(start: Point, end: Point, square: bool, from_center: bool) -> Rect {
    let mut dx = end.x - start.x;
    let mut dy = end.y - start.y;

    if square {
        let side = dx.abs().max(dy.abs());
        dx = side * dx.signum();
        dy = side * dy.signum();
    }

    if from_center {
        Rect::from_points(
            Point::new(start.x - dx, start.y - dy),
            Point::new(start.x + dx, start.y + dy),
        )
    } else {
        Rect::from_points(start, Point::new(start.x + dx, start.y + dy))
    }
}
