//! Viewport management: pan, zoom, and coordinate conversion.

use sivgo_math::{Point, Rect, Transform2D, Vector};

/// Display mode controls how elements are rendered in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// Normal rendering with all fills, strokes, and filters.
    #[default]
    Normal,
    /// Outline-only rendering (no fills, thin strokes).
    Outline,
    /// Normal rendering but with all SVG filters disabled.
    NoFilters,
    /// Normal rendering but hairline strokes are rendered at a visible width.
    VisibleHairlines,
}

/// The viewport maps between screen (widget) coordinates and document
/// coordinates via a pan offset and zoom factor.
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Offset in screen pixels (document origin shifted by this amount).
    pub pan: Vector,
    /// Zoom factor (1.0 = 100%).
    pub zoom: f64,
    /// Screen/widget width in pixels.
    pub width: f64,
    /// Screen/widget height in pixels.
    pub height: f64,
    /// How elements are rendered in the viewport.
    pub display_mode: DisplayMode,
}

impl Viewport {
    /// Create a new viewport with the given screen dimensions.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            pan: Vector::ZERO,
            zoom: 1.0,
            width,
            height,
            display_mode: DisplayMode::default(),
        }
    }

    /// Convert a screen-space point to document-space.
    pub fn screen_to_document(&self, screen: Point) -> Point {
        Point::new(
            (screen.x - self.pan.x) / self.zoom,
            (screen.y - self.pan.y) / self.zoom,
        )
    }

    /// Convert a document-space point to screen-space.
    pub fn document_to_screen(&self, doc: Point) -> Point {
        Point::new(
            doc.x * self.zoom + self.pan.x,
            doc.y * self.zoom + self.pan.y,
        )
    }

    /// Zoom toward a screen-space center point by `factor`.
    ///
    /// The document point under `center` remains fixed.
    pub fn zoom_at(&mut self, center: Point, factor: f64) {
        // Document point under the cursor before zoom.
        let doc_pt = self.screen_to_document(center);
        self.zoom *= factor;
        // Adjust pan so that doc_pt maps back to center.
        self.pan = Vector::new(
            center.x - doc_pt.x * self.zoom,
            center.y - doc_pt.y * self.zoom,
        );
    }

    /// Pan by a delta in screen pixels.
    pub fn pan_by(&mut self, delta: Vector) {
        self.pan = self.pan + delta;
    }

    /// Adjust pan and zoom to fit `rect` (in document coordinates) into the
    /// viewport, with a small margin.
    pub fn fit_rect(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        let margin = 20.0; // pixels
        let available_w = (self.width - 2.0 * margin).max(1.0);
        let available_h = (self.height - 2.0 * margin).max(1.0);
        let scale_x = available_w / rect.width();
        let scale_y = available_h / rect.height();
        self.zoom = scale_x.min(scale_y);
        // Center the rect in the viewport.
        let center = rect.center();
        self.pan = Vector::new(
            self.width / 2.0 - center.x * self.zoom,
            self.height / 2.0 - center.y * self.zoom,
        );
    }

    /// Return the viewport transform as a [`Transform2D`].
    ///
    /// Applying this transform to a document point yields a screen point.
    pub fn to_transform(&self) -> Transform2D {
        Transform2D::translate(self.pan.x, self.pan.y).then(&Transform2D::scale(self.zoom))
    }

    /// Cycle through display modes: Normal -> Outline -> NoFilters ->
    /// VisibleHairlines -> Normal.
    pub fn cycle_display_mode(&mut self) {
        self.display_mode = match self.display_mode {
            DisplayMode::Normal => DisplayMode::Outline,
            DisplayMode::Outline => DisplayMode::NoFilters,
            DisplayMode::NoFilters => DisplayMode::VisibleHairlines,
            DisplayMode::VisibleHairlines => DisplayMode::Normal,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_viewport() {
        let vp = Viewport::new(800.0, 600.0);
        let p = Point::new(100.0, 200.0);
        assert_eq!(vp.screen_to_document(p), p);
        assert_eq!(vp.document_to_screen(p), p);
    }

    #[test]
    fn zoom_conversion() {
        let mut vp = Viewport::new(800.0, 600.0);
        vp.zoom = 2.0;
        let screen = Point::new(200.0, 100.0);
        let doc = vp.screen_to_document(screen);
        assert!((doc.x - 100.0).abs() < 1e-10);
        assert!((doc.y - 50.0).abs() < 1e-10);

        let back = vp.document_to_screen(doc);
        assert!((back.x - screen.x).abs() < 1e-10);
        assert!((back.y - screen.y).abs() < 1e-10);
    }

    #[test]
    fn pan_conversion() {
        let mut vp = Viewport::new(800.0, 600.0);
        vp.pan = Vector::new(50.0, 100.0);
        let screen = Point::new(150.0, 200.0);
        let doc = vp.screen_to_document(screen);
        assert!((doc.x - 100.0).abs() < 1e-10);
        assert!((doc.y - 100.0).abs() < 1e-10);
    }

    #[test]
    fn zoom_at_preserves_point() {
        let mut vp = Viewport::new(800.0, 600.0);
        let center = Point::new(400.0, 300.0);
        let doc_before = vp.screen_to_document(center);
        vp.zoom_at(center, 2.0);
        let doc_after = vp.screen_to_document(center);
        assert!((doc_before.x - doc_after.x).abs() < 1e-10);
        assert!((doc_before.y - doc_after.y).abs() < 1e-10);
    }

    #[test]
    fn fit_rect_centers() {
        let mut vp = Viewport::new(800.0, 600.0);
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        vp.fit_rect(rect);
        // The center of the rect should map to the center of the viewport.
        let center_screen = vp.document_to_screen(rect.center());
        assert!((center_screen.x - 400.0).abs() < 1e-10);
        assert!((center_screen.y - 300.0).abs() < 1e-10);
    }
}
