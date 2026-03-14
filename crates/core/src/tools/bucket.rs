//! Bucket fill tool: click on an element to change its fill color.
//!
//! On click, hit-test to find the element under the cursor and emit a
//! [`ModifyAttributeCommand`] that sets the fill to the tool's current color.

use sivgo_dom::types::{Color, Paint};
use sivgo_dom::SvgDocument;
use sivgo_math::Point;

use crate::commands::ModifyAttributeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::select::hit_test_at;
use crate::viewport::Viewport;

/// The bucket fill tool.
///
/// Click on an element to set its fill to [`current_color`](Self::current_color).
#[derive(Debug)]
pub struct BucketFillTool {
    /// The color that will be applied as fill on click.
    current_color: Color,
}

impl BucketFillTool {
    pub fn new() -> Self {
        Self {
            current_color: Color::BLACK,
        }
    }

    /// Create a bucket fill tool with a specific initial color.
    #[allow(dead_code)]
    pub fn with_color(color: Color) -> Self {
        Self {
            current_color: color,
        }
    }

    /// Set the current fill color.
    #[allow(dead_code)]
    pub fn set_color(&mut self, color: Color) {
        self.current_color = color;
    }

    /// Return the current fill color.
    #[allow(dead_code)]
    pub fn current_color(&self) -> Color {
        self.current_color
    }
}

impl Default for BucketFillTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for BucketFillTool {
    fn name(&self) -> &str {
        "Bucket Fill"
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

        let node = match doc.get_node(hit_id) {
            Some(n) => n,
            None => return ToolResponse::Handled,
        };

        let old_attrs = node.presentation.clone();
        let mut new_attrs = old_attrs.clone();
        new_attrs.fill = Some(Paint::Color(self.current_color));

        let cmd = ModifyAttributeCommand::new(hit_id, old_attrs, new_attrs);
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
