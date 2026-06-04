//! Vello-style rendering backend.
//!
//! The backend records a Vello/wgpu scene command stream and performs
//! deterministic TinySkia replay for pixel output.  That makes the surface
//! usable for tests, export, and headless operation while still exposing the
//! scene commands needed by a native Vello adapter.

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::command_backend::{
    RecordedRenderBackend, RecordedRenderCommand, RenderBackendCapabilities, RenderBackendStatus,
    RenderSurface,
};
use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// Rendering backend for Vello/wgpu scenes with deterministic CPU replay.
pub struct VelloBackend {
    inner: RecordedRenderBackend,
}

impl VelloBackend {
    /// Create a new Vello command backend with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: RecordedRenderBackend::new(RenderSurface::Vello, width, height),
        }
    }

    /// Whether this instance is currently backed by hardware GPU execution.
    pub fn is_gpu_accelerated(&self) -> bool {
        self.capabilities().hardware_accelerated
    }

    /// Static capabilities for the current execution environment.
    pub fn capabilities(&self) -> RenderBackendCapabilities {
        self.inner.capabilities()
    }

    /// Runtime status including surface size and recorded command count.
    pub fn status(&self) -> RenderBackendStatus {
        self.inner.status()
    }

    /// Recorded scene commands suitable for a Vello adapter.
    pub fn commands(&self) -> &[RecordedRenderCommand] {
        self.inner.commands()
    }

    /// Clear the recorded command stream without changing pixels.
    pub fn clear_commands(&mut self) {
        self.inner.clear_commands();
    }
}

impl RenderBackend for VelloBackend {
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
    use crate::command_backend::RenderExecutionMode;
    use sivgo_dom::Color;
    use sivgo_math::{BezPath, Point};

    fn nontransparent_pixels(pixels: &[u8]) -> usize {
        pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    fn triangular_path() -> BezPath {
        let mut path = BezPath::new();
        path.move_to(Point::new(4.0, 4.0));
        path.line_to(Point::new(28.0, 4.0));
        path.line_to(Point::new(16.0, 28.0));
        path.close();
        path
    }

    #[test]
    fn vello_records_scene_commands_and_replays_paths() {
        let mut backend = VelloBackend::new(32, 32);
        backend.clear(Color::TRANSPARENT);
        backend.fill_path(
            &triangular_path(),
            &RenderPaint::Solid(Color::new(255, 0, 0, 255)),
            FillRule::NonZero,
            &Transform2D::identity(),
        );

        assert!(nontransparent_pixels(backend.pixels()) > 40);
        assert!(matches!(
            backend.commands().last(),
            Some(RecordedRenderCommand::FillPath { .. })
        ));
        assert_eq!(backend.status().recorded_commands, 2);
    }

    #[test]
    fn vello_replays_strokes_clips_images_and_offscreen_groups() {
        let mut backend = VelloBackend::new(32, 32);
        backend.clear(Color::TRANSPARENT);
        backend.set_clip_path(
            &triangular_path(),
            FillRule::NonZero,
            &Transform2D::identity(),
        );
        backend.stroke_circle(
            16.0,
            16.0,
            9.0,
            &RenderPaint::Solid(Color::new(0, 0, 255, 255)),
            &StrokeStyle {
                width: 4.0,
                ..StrokeStyle::default()
            },
            &Transform2D::identity(),
        );
        backend.clear_clip();
        backend.push_offscreen();
        backend.draw_image(
            &[0, 255, 0, 255],
            1,
            1,
            12.0,
            12.0,
            8.0,
            8.0,
            &Transform2D::identity(),
        );
        backend.pop_offscreen_with_opacity(0.75);

        assert!(nontransparent_pixels(backend.pixels()) > 20);
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::StrokeCircle { .. })));
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::DrawImage { .. })));
        assert!(backend
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RecordedRenderCommand::PopOffscreenWithOpacity(_))));
    }

    #[test]
    fn vello_exposes_cpu_replay_capabilities() {
        let backend = VelloBackend::new(10, 10);
        let caps = backend.capabilities();
        assert_eq!(caps.surface, RenderSurface::Vello);
        assert_eq!(caps.execution_mode, RenderExecutionMode::CpuReplay);
        assert!(caps.command_stream);
        assert!(caps.deterministic_cpu_replay);
        assert!(caps.supports_paths);
        assert!(!backend.is_gpu_accelerated());
    }
}
