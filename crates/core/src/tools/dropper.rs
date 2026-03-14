//! Dropper (eyedropper/color picker) tool: click on an element to pick its
//! fill color and apply it to the currently selected element.

use sivgo_dom::SvgDocument;
use sivgo_math::Point;

use crate::commands::ModifyAttributeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::select::hit_test_at;
use crate::viewport::Viewport;

/// The dropper (eyedropper) tool.
///
/// Click on a canvas element to sample its fill color, then apply that fill
/// to the currently selected element via a [`ModifyAttributeCommand`].
#[derive(Debug)]
pub struct DropperTool {
    _private: (),
}

impl DropperTool {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for DropperTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for DropperTool {
    fn name(&self) -> &str {
        "Dropper"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
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

        // Get the fill paint from the hit element.
        let sampled_fill = match doc.get_node(hit_id) {
            Some(node) => node.presentation.fill.clone(),
            None => return ToolResponse::Handled,
        };

        // Apply the sampled fill to the primary selected element.
        let target_id = match selection.primary() {
            Some(id) => id,
            None => return ToolResponse::Handled,
        };

        // Don't apply to the same element we sampled from.
        if target_id == hit_id {
            return ToolResponse::Handled;
        }

        let target_node = match doc.get_node(target_id) {
            Some(n) => n,
            None => return ToolResponse::Handled,
        };

        let old_attrs = target_node.presentation.clone();
        let mut new_attrs = old_attrs.clone();
        new_attrs.fill = sampled_fill;

        let cmd = ModifyAttributeCommand::new(target_id, old_attrs, new_attrs);
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
        "crosshair"
    }
}
