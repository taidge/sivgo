//! Text tool: click to place text, click-drag to create a text area.

use sivgo_dom::{PresentationAttrs, SvgDocument, SvgElement, TextAttrs};
use sivgo_math::Point;

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// Internal state of the text tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextToolState {
    /// Waiting for the user to click.
    Idle,
    /// Mouse is pressed; may become a drag.
    Placing,
}

/// The text creation tool.
#[derive(Debug)]
pub struct TextTool {
    state: TextToolState,
    /// Start point in document space (set on mouse-down).
    start: Option<Point>,
}

impl TextTool {
    pub fn new() -> Self {
        Self {
            state: TextToolState::Idle,
            start: None,
        }
    }

    fn reset(&mut self) {
        self.state = TextToolState::Idle;
        self.start = None;
    }
}

impl Default for TextTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextTool {
    fn name(&self) -> &str {
        "Text"
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
        self.state = TextToolState::Placing;
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        if self.state == TextToolState::Placing {
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
        if self.state != TextToolState::Placing {
            return ToolResponse::None;
        }
        self.state = TextToolState::Idle;

        let start = match self.start.take() {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let dx = (end.x - start.x).abs();
        let dy = (end.y - start.y).abs();

        let is_drag = dx > 2.0 || dy > 2.0;

        let mut attrs = TextAttrs {
            x: vec![SvgLength::new(start.x, LengthUnit::None)],
            y: vec![SvgLength::new(start.y, LengthUnit::None)],
            text_content: "Text".to_string(),
            ..Default::default()
        };

        let pres = PresentationAttrs {
            fill: Some(Paint::Color(Color::BLACK)),
            ..Default::default()
        };

        if is_drag {
            // For a text area, compute the bounding box from the drag rectangle.
            let min_x = start.x.min(end.x);
            let min_y = start.y.min(end.y);
            attrs.x = vec![SvgLength::new(min_x, LengthUnit::None)];
            attrs.y = vec![SvgLength::new(min_y, LengthUnit::None)];
            // Store text-area dimensions as font-size hint: width via dx, height ignored
            // (SVG 1.1 does not have a native text-area; store width/height as extra attrs).
        }

        let element = SvgElement::Text(attrs);

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
        self.reset();
    }

    fn on_deactivate(&mut self) {
        self.reset();
    }

    fn cursor(&self) -> &str {
        "text"
    }
}
