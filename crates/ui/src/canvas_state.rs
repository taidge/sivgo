//! Canvas view state data model (11.1.1).
//!
//! Pure data structures for canvas dimensions, invalidation tracking, and
//! resize handling.

/// Tracks the dimensions and redraw state of the canvas viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    /// Width of the canvas in device-independent pixels.
    pub width: f64,
    /// Height of the canvas in device-independent pixels.
    pub height: f64,
    /// Whether the canvas needs to be redrawn.
    pub needs_redraw: bool,
}

impl CanvasState {
    /// Create a new canvas state with the given dimensions.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            needs_redraw: true,
        }
    }

    /// Handle a resize event.  Updates the dimensions and marks the canvas as
    /// needing a redraw.  Returns `true` if the size actually changed.
    pub fn resize(&mut self, new_width: f64, new_height: f64) -> bool {
        if (self.width - new_width).abs() < f64::EPSILON
            && (self.height - new_height).abs() < f64::EPSILON
        {
            return false;
        }
        self.width = new_width;
        self.height = new_height;
        self.needs_redraw = true;
        true
    }

    /// Mark the canvas as needing a redraw (e.g. after document changes).
    pub fn invalidate(&mut self) {
        self.needs_redraw = true;
    }

    /// Mark the canvas as up-to-date (called after rendering).
    pub fn mark_clean(&mut self) {
        self.needs_redraw = false;
    }

    /// Return the canvas area in square pixels.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Return the aspect ratio (width / height).  Returns 1.0 if height is
    /// zero.
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0.0 {
            1.0
        } else {
            self.width / self.height
        }
    }
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new(800.0, 600.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_canvas_needs_redraw() {
        let canvas = CanvasState::new(1024.0, 768.0);
        assert!(canvas.needs_redraw);
        assert!((canvas.width - 1024.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut canvas = CanvasState::new(800.0, 600.0);
        canvas.mark_clean();
        assert!(!canvas.needs_redraw);

        let changed = canvas.resize(1024.0, 768.0);
        assert!(changed);
        assert!(canvas.needs_redraw);
        assert!((canvas.width - 1024.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resize_same_size_no_change() {
        let mut canvas = CanvasState::new(800.0, 600.0);
        canvas.mark_clean();
        let changed = canvas.resize(800.0, 600.0);
        assert!(!changed);
        assert!(!canvas.needs_redraw);
    }
}
