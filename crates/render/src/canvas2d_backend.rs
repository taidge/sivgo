//! Canvas2D rendering backend for browser deployment.
//!
//! The backend records Canvas2D-compatible drawing commands and replays them
//! through TinySkia for deterministic pixels on native targets.  WASM callers
//! can consume the same command stream to drive `CanvasRenderingContext2D`.

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::command_backend::{
    RecordedRenderBackend, RecordedRenderCommand, RenderBackendCapabilities, RenderBackendStatus,
    RenderSurface,
};
use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// Rendering backend for HTML Canvas2D command streams.
pub struct Canvas2dBackend {
    inner: RecordedRenderBackend,
}

impl Canvas2dBackend {
    /// Create a new Canvas2D command backend with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: RecordedRenderBackend::new(RenderSurface::Canvas2d, width, height),
        }
    }

    /// Whether this backend is executing in a browser WASM build.
    pub fn is_browser(&self) -> bool {
        cfg!(target_arch = "wasm32")
    }

    /// Static capabilities for the current execution environment.
    pub fn capabilities(&self) -> RenderBackendCapabilities {
        self.inner.capabilities()
    }

    /// Runtime status including surface size and recorded command count.
    pub fn status(&self) -> RenderBackendStatus {
        self.inner.status()
    }

    /// Recorded browser drawing commands suitable for a Canvas2D adapter.
    pub fn commands(&self) -> &[RecordedRenderCommand] {
        self.inner.commands()
    }

    /// Clear the recorded command stream without changing pixels.
    pub fn clear_commands(&mut self) {
        self.inner.clear_commands();
    }
}

impl RenderBackend for Canvas2dBackend {
    fn size(&self) -> (u32, u32) {
        self.inner.size()
    }

    fn clear(&mut self, color: Color) {
        self.inner.clear(color);
    }

    fn fill_rect(&mut self, rect: Rect, paint: &RenderPaint, transform: &Transform2D) {
        self.inner.fill_rect(rect, paint, transform);
    }

    fn fill_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        fill_rule: FillRule,
        transform: &Transform2D,
    ) {
        self.inner.fill_path(path, paint, fill_rule, transform);
    }

    fn fill_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    ) {
        self.inner.fill_circle(cx, cy, r, paint, transform);
    }

    fn fill_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    ) {
        self.inner.fill_ellipse(cx, cy, rx, ry, paint, transform);
    }

    fn stroke_rect(
        &mut self,
        rect: Rect,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.inner.stroke_rect(rect, paint, stroke, transform);
    }

    fn stroke_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.inner.stroke_path(path, paint, stroke, transform);
    }

    fn stroke_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.inner
            .stroke_circle(cx, cy, r, paint, stroke, transform);
    }

    fn stroke_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.inner
            .stroke_ellipse(cx, cy, rx, ry, paint, stroke, transform);
    }

    fn set_clip_path(&mut self, path: &BezPath, fill_rule: FillRule, transform: &Transform2D) {
        self.inner.set_clip_path(path, fill_rule, transform);
    }

    fn clear_clip(&mut self) {
        self.inner.clear_clip();
    }

    fn set_mask_from_pixmap_luminance(&mut self, mask_pixels: &[u8]) {
        self.inner.set_mask_from_pixmap_luminance(mask_pixels);
    }

    fn clear_mask(&mut self) {
        self.inner.clear_mask();
    }

    fn set_opacity(&mut self, opacity: f64) {
        self.inner.set_opacity(opacity);
    }

    fn save(&mut self) {
        self.inner.save();
    }

    fn restore(&mut self) {
        self.inner.restore();
    }

    fn push_offscreen(&mut self) {
        self.inner.push_offscreen();
    }

    fn pop_offscreen_with_opacity(&mut self, opacity: f64) {
        self.inner.pop_offscreen_with_opacity(opacity);
    }

    fn pop_offscreen_with_blend(&mut self, blend_mode: BlendMode) {
        self.inner.pop_offscreen_with_blend(blend_mode);
    }

    fn pop_offscreen_raw(&mut self) -> (u32, u32, Vec<u8>) {
        self.inner.pop_offscreen_raw()
    }

    fn composite_pixels(&mut self, data: &[u8], width: u32, height: u32) {
        self.inner.composite_pixels(data, width, height);
    }

    fn draw_image(
        &mut self,
        data: &[u8],
        img_width: u32,
        img_height: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        transform: &Transform2D,
    ) {
        self.inner
            .draw_image(data, img_width, img_height, x, y, w, h, transform);
    }

    fn pixels(&self) -> &[u8] {
        self.inner.pixels()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::Color;
    use sivgo_math::{BezPath, Point};

    fn colored_pixels(pixels: &[u8]) -> usize {
        pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && (p[0] > 0 || p[1] > 0 || p[2] > 0))
            .count()
    }

    fn path() -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(2.0, 2.0));
        path.line_to(Point::new(22.0, 2.0));
        path.line_to(Point::new(22.0, 22.0));
        path.close();
        path
    }

    #[test]
    fn canvas2d_records_browser_commands_and_replays_non_rect_geometry() {
        let mut backend = Canvas2dBackend::new(32, 32);
        backend.clear(Color::TRANSPARENT);
        backend.fill_circle(
            16.0,
            16.0,
            8.0,
            &RenderPaint::Solid(Color::new(255, 0, 0, 255)),
            &Transform2D::identity(),
        );
        backend.stroke_path(
            &path(),
            &RenderPaint::Solid(Color::new(0, 0, 255, 255)),
            &StrokeStyle {
                width: 3.0,
                ..StrokeStyle::default()
            },
            &Transform2D::identity(),
        );

        assert!(colored_pixels(backend.pixels()) > 50);
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::FillCircle { .. })));
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::StrokePath { .. })));
    }

    #[test]
    fn canvas2d_replays_masks_images_and_offscreen_groups() {
        let mut backend = Canvas2dBackend::new(4, 4);
        backend.clear(Color::TRANSPARENT);
        let mut mask = vec![0u8; 4 * 4 * 4];
        for px in mask.chunks_exact_mut(4).take(8) {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
            px[3] = 255;
        }
        backend.set_mask_from_pixmap_luminance(&mask);
        backend.draw_image(
            &[255, 0, 0, 255],
            1,
            1,
            0.0,
            0.0,
            4.0,
            4.0,
            &Transform2D::identity(),
        );
        backend.clear_mask();
        backend.push_offscreen();
        backend.fill_rect(
            Rect::new(2.0, 2.0, 4.0, 4.0),
            &RenderPaint::Solid(Color::new(0, 255, 0, 255)),
            &Transform2D::identity(),
        );
        backend.pop_offscreen_with_opacity(1.0);

        assert!(colored_pixels(backend.pixels()) > 8);
        assert!(backend.commands().iter().any(|cmd| matches!(
            cmd,
            RecordedRenderCommand::SetMaskFromPixmapLuminance { .. }
        )));
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::DrawImage { .. })));
    }

    #[test]
    fn canvas2d_exposes_browser_surface_capabilities() {
        let backend = Canvas2dBackend::new(10, 10);
        let caps = backend.capabilities();
        assert_eq!(caps.surface, RenderSurface::Canvas2d);
        assert_eq!(caps.browser_api, Some("CanvasRenderingContext2D"));
        assert!(caps.command_stream);
        assert!(caps.deterministic_cpu_replay);
        assert_eq!(backend.status().recorded_commands, 0);
        assert_eq!(backend.is_browser(), cfg!(target_arch = "wasm32"));
    }
}
