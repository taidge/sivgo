//! Eraser tool: click on an element to delete it from the document.

use sivgo_dom::SvgDocument;
use sivgo_math::Point;

use crate::commands::RemoveNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::select::hit_test_at;
use crate::viewport::Viewport;

/// The eraser tool.
///
/// Click on an element to remove it from the document via a
/// [`RemoveNodeCommand`].
#[derive(Debug)]
pub struct EraserTool {
    _private: (),
}

impl EraserTool {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for EraserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for EraserTool {
    fn name(&self) -> &str {
        "Eraser"
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

        // Hit-test to find the element under the cursor.
        let hit_id = match hit_test_at(doc, doc_pt) {
            Some(id) => id,
            None => return ToolResponse::Handled,
        };

        let cmd = RemoveNodeCommand::new(hit_id);
        ToolResponse::ExecuteCommand(Box::new(cmd))
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
        _event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn cursor(&self) -> &str {
        "not-allowed"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
