//! Pen tool: click to add points, building a path.

use sivgo_dom::{PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point};

use crate::commands::AddNodeCommand;
use crate::input::{Key, KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};

/// A basic pen tool that places points to build a path.
#[derive(Debug)]
pub struct PenTool {
    /// Points accumulated so far.
    points: Vec<Point>,
    /// Minimum distance to the first point to auto-close.
    close_threshold: f64,
}

impl PenTool {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            close_threshold: 5.0,
        }
    }

    /// Attempt to finalize the current path.  Returns a command if there are
    /// enough points, or None.
    fn finalize(&mut self, doc: &SvgDocument) -> ToolResponse {
        if self.points.len() < 2 {
            self.points.clear();
            return ToolResponse::None;
        }

        let mut path = BezPath::new();
        path.move_to(self.points[0]);
        for &pt in &self.points[1..] {
            path.line_to(pt);
        }

        let element = SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        });

        let pres = PresentationAttrs {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(SvgLength::new(2.0, LengthUnit::None)),
            ..Default::default()
        };

        let parent = match doc.root() {
            Some(r) => r,
            None => {
                self.points.clear();
                return ToolResponse::None;
            }
        };

        self.points.clear();
        let cmd = AddNodeCommand::new(parent, element, pres, None);
        ToolResponse::ExecuteCommand(Box::new(cmd))
    }
}

impl Default for PenTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for PenTool {
    fn name(&self) -> &str {
        "Pen"
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

        // If close to the first point, close the path.
        if self.points.len() >= 2 {
            if let Some(&first) = self.points.first() {
                if doc_pt.distance(first) < self.close_threshold / viewport.zoom {
                    return self.finalize(doc);
                }
            }
        }

        self.points.push(doc_pt);
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_mouse_up(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        match event.key {
            Key::Enter => self.finalize(doc),
            Key::Escape => {
                self.points.clear();
                ToolResponse::Handled
            }
            _ => ToolResponse::None,
        }
    }

    fn on_activate(&mut self) {
        self.points.clear();
    }

    fn on_deactivate(&mut self) {
        self.points.clear();
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }
}
