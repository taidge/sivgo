//! Status bar data model (11.1.4).
//!
//! A plain data structure holding all the information displayed in the
//! editor's bottom status bar.

/// All information displayed in the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBarInfo {
    /// Cursor X position in document coordinates.
    pub cursor_x: f64,
    /// Cursor Y position in document coordinates.
    pub cursor_y: f64,
    /// Current zoom level as a percentage (100.0 = 100%).
    pub zoom_percent: f64,
    /// Number of currently selected objects.
    pub selection_count: usize,
    /// Total number of objects in the document.
    pub object_count: usize,
    /// The currently active tool name.
    pub active_tool: String,
    /// Optional transient message (e.g. "Saved", "3 objects grouped").
    pub message: Option<String>,
}

impl Default for StatusBarInfo {
    fn default() -> Self {
        Self {
            cursor_x: 0.0,
            cursor_y: 0.0,
            zoom_percent: 100.0,
            selection_count: 0,
            object_count: 0,
            active_tool: "Select".to_string(),
            message: None,
        }
    }
}

impl StatusBarInfo {
    /// Format the cursor position as a display string.
    pub fn cursor_display(&self) -> String {
        format!("X: {:.1}  Y: {:.1}", self.cursor_x, self.cursor_y)
    }

    /// Format the zoom level as a display string.
    pub fn zoom_display(&self) -> String {
        format!("Zoom: {:.0}%", self.zoom_percent)
    }

    /// Format the selection info as a display string.
    pub fn selection_display(&self) -> String {
        match self.selection_count {
            0 => "No selection".to_string(),
            1 => "1 selected".to_string(),
            n => format!("{} selected", n),
        }
    }

    /// Update cursor position.
    pub fn set_cursor(&mut self, x: f64, y: f64) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    /// Set a transient message.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
    }

    /// Clear the transient message.
    pub fn clear_message(&mut self) {
        self.message = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_bar() {
        let info = StatusBarInfo::default();
        assert_eq!(info.zoom_percent, 100.0);
        assert_eq!(info.selection_count, 0);
        assert!(info.message.is_none());
    }

    #[test]
    fn cursor_display_format() {
        let mut info = StatusBarInfo::default();
        info.set_cursor(123.456, 789.012);
        assert_eq!(info.cursor_display(), "X: 123.5  Y: 789.0");
    }

    #[test]
    fn selection_display_variants() {
        let mut info = StatusBarInfo::default();
        assert_eq!(info.selection_display(), "No selection");
        info.selection_count = 1;
        assert_eq!(info.selection_display(), "1 selected");
        info.selection_count = 5;
        assert_eq!(info.selection_display(), "5 selected");
    }
}
