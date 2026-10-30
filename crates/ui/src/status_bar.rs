//! Status bar component showing cursor position, zoom, selection info, and
//! object count.

use crate::controls::*;
use glory::prelude::*;

/// A horizontal status bar displayed at the bottom of the editor window.
///
/// Shows:
/// - Cursor position (x, y)
/// - Zoom level as a percentage
/// - Selection info (count and type description)
/// - Total object count in the document
pub struct StatusBar {
    pub cursor_position: Cage<(f64, f64)>,
    pub zoom: f64,
    pub selection_count: usize,
    pub selection_type: String,
    pub object_count: usize,
}

impl Widget for StatusBar {
    fn build(&mut self, scope: &Scope) -> Result {
        let panel = panel(scope, "status-bar", "")?;
        let cursor = self.cursor_position;
        text()
            .content(cursor.map(|(x, y)| format!("Cursor: {x:.1}, {y:.1}")))
            .show_in(&panel)?;
        text()
            .content(format!("Zoom: {:.0}%", self.zoom * 100.0))
            .show_in(&panel)?;
        text()
            .content(if self.selection_count == 0 {
                "No selection".into()
            } else {
                format!("{} {} selected", self.selection_count, self.selection_type)
            })
            .show_in(&panel)?;
        text()
            .content(format!(
                "{} {}",
                self.object_count,
                if self.object_count == 1 {
                    "object"
                } else {
                    "objects"
                }
            ))
            .show_in(&panel)
    }
}
