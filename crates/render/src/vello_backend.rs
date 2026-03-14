//! Vello GPU-based rendering backend (stub).
//!
//! This module provides a `VelloBackend` that implements `RenderBackend`
//! for GPU-accelerated 2D vector rendering via `vello` + `wgpu`.
//!
//! Currently a compile-ready stub that falls back to software rasterization.
//! A full implementation would create a `vello::Scene`, record drawing
//! commands, and render via wgpu.

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// GPU rendering backend using Vello + wgpu.
///
/// In production this would hold a `vello::Scene` and a wgpu device/queue.
/// For now it renders into a CPU pixel buffer so the `RenderBackend` trait
/// is fully satisfied without a GPU context.
pub struct VelloBackend {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    opacity_stack: Vec<f64>,
    current_opacity: f64,
    offscreen_stack: Vec<Vec<u8>>,
}

impl VelloBackend {
    /// Create a new Vello backend with the given dimensions.
    ///
    /// In a full implementation this would accept a wgpu `Device` and `Queue`.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
            opacity_stack: Vec::new(),
            current_opacity: 1.0,
            offscreen_stack: Vec::new(),
        }
    }

    /// Whether the backend has GPU acceleration available.
    ///
    /// Returns `false` in the current stub implementation.
    pub fn is_gpu_accelerated(&self) -> bool {
        false
    }
}

impl RenderBackend for VelloBackend {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn clear(&mut self, color: Color) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = color.r;
            chunk[1] = color.g;
            chunk[2] = color.b;
            chunk[3] = 255;
        }
    }

    fn fill_rect(&mut self, rect: Rect, paint: &RenderPaint, _transform: &Transform2D) {
        let color = paint_to_color(paint);
        let x0 = (rect.x0 as i32).max(0) as u32;
        let y0 = (rect.y0 as i32).max(0) as u32;
        let x1 = (rect.x1 as u32).min(self.width);
        let y1 = (rect.y1 as u32).min(self.height);
        let alpha = (self.current_opacity * 255.0) as u8;

        for y in y0..y1 {
            for x in x0..x1 {
                let idx = ((y * self.width + x) * 4) as usize;
                if idx + 3 < self.pixels.len() {
                    blend_pixel(&mut self.pixels[idx..idx + 4], &color, alpha);
                }
            }
        }
    }

    fn fill_path(
        &mut self,
        _path: &BezPath,
        _paint: &RenderPaint,
        _fill_rule: FillRule,
        _transform: &Transform2D,
    ) {
        // Stub: full implementation would flatten path and scanline-fill
    }

    fn fill_circle(
        &mut self,
        _cx: f64,
        _cy: f64,
        _r: f64,
        _paint: &RenderPaint,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn fill_ellipse(
        &mut self,
        _cx: f64,
        _cy: f64,
        _rx: f64,
        _ry: f64,
        _paint: &RenderPaint,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn stroke_rect(
        &mut self,
        _rect: Rect,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn stroke_path(
        &mut self,
        _path: &BezPath,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn stroke_circle(
        &mut self,
        _cx: f64,
        _cy: f64,
        _r: f64,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn stroke_ellipse(
        &mut self,
        _cx: f64,
        _cy: f64,
        _rx: f64,
        _ry: f64,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
        // Stub
    }

    fn set_clip_path(&mut self, _path: &BezPath, _fill_rule: FillRule, _transform: &Transform2D) {
        // Stub
    }

    fn clear_clip(&mut self) {
        // Stub
    }

    fn set_mask_from_pixmap_luminance(&mut self, _mask_pixels: &[u8]) {
        // Stub
    }

    fn clear_mask(&mut self) {
        // Stub
    }

    fn set_opacity(&mut self, opacity: f64) {
        self.current_opacity = opacity.clamp(0.0, 1.0);
    }

    fn save(&mut self) {
        self.opacity_stack.push(self.current_opacity);
    }

    fn restore(&mut self) {
        if let Some(op) = self.opacity_stack.pop() {
            self.current_opacity = op;
        }
    }

    fn push_offscreen(&mut self) {
        let buf = vec![0u8; self.pixels.len()];
        self.offscreen_stack
            .push(std::mem::replace(&mut self.pixels, buf));
    }

    fn pop_offscreen_with_opacity(&mut self, opacity: f64) {
        if let Some(prev) = self.offscreen_stack.pop() {
            let offscreen = std::mem::replace(&mut self.pixels, prev);
            let alpha = (opacity * 255.0) as u16;
            for (dst, src) in self
                .pixels
                .chunks_exact_mut(4)
                .zip(offscreen.chunks_exact(4))
            {
                let sa = (src[3] as u16 * alpha) / 255;
                if sa > 0 {
                    let inv = 255 - sa as u8;
                    dst[0] =
                        ((src[0] as u16 * alpha / 255) + (dst[0] as u16 * inv as u16 / 255)) as u8;
                    dst[1] =
                        ((src[1] as u16 * alpha / 255) + (dst[1] as u16 * inv as u16 / 255)) as u8;
                    dst[2] =
                        ((src[2] as u16 * alpha / 255) + (dst[2] as u16 * inv as u16 / 255)) as u8;
                    dst[3] = (sa + (dst[3] as u16 * inv as u16 / 255)) as u8;
                }
            }
        }
    }

    fn pop_offscreen_with_blend(&mut self, _blend_mode: BlendMode) {
        // Simplified: composite with normal blending
        self.pop_offscreen_with_opacity(1.0);
    }

    fn pop_offscreen_raw(&mut self) -> (u32, u32, Vec<u8>) {
        if let Some(prev) = self.offscreen_stack.pop() {
            let offscreen = std::mem::replace(&mut self.pixels, prev);
            (self.width, self.height, offscreen)
        } else {
            (self.width, self.height, self.pixels.clone())
        }
    }

    fn composite_pixels(&mut self, data: &[u8], _width: u32, _height: u32) {
        for (dst, src) in self.pixels.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
            let sa = src[3] as u16;
            if sa > 0 {
                let inv = 255 - sa as u8;
                dst[0] = (src[0] as u16 + dst[0] as u16 * inv as u16 / 255) as u8;
                dst[1] = (src[1] as u16 + dst[1] as u16 * inv as u16 / 255) as u8;
                dst[2] = (src[2] as u16 + dst[2] as u16 * inv as u16 / 255) as u8;
                dst[3] = (sa + dst[3] as u16 * inv as u16 / 255) as u8;
            }
        }
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
        _transform: &Transform2D,
    ) {
        let x0 = x as i32;
        let y0 = y as i32;
        let scale_x = img_width as f64 / w;
        let scale_y = img_height as f64 / h;

        for dy in 0..h as u32 {
            for dx in 0..w as u32 {
                let px = x0 + dx as i32;
                let py = y0 + dy as i32;
                if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
                    continue;
                }
                let sx = ((dx as f64 * scale_x) as u32).min(img_width - 1);
                let sy = ((dy as f64 * scale_y) as u32).min(img_height - 1);
                let src_idx = ((sy * img_width + sx) * 4) as usize;
                let dst_idx = ((py as u32 * self.width + px as u32) * 4) as usize;
                if src_idx + 3 < data.len() && dst_idx + 3 < self.pixels.len() {
                    self.pixels[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
                }
            }
        }
    }

    fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// Extract a solid RGBA color from a RenderPaint.
fn paint_to_color(paint: &RenderPaint) -> [u8; 4] {
    match paint {
        RenderPaint::Solid(c) => [c.r, c.g, c.b, 255],
        _ => [0, 0, 0, 255],
    }
}

/// Alpha-blend a source color onto a destination pixel.
fn blend_pixel(dst: &mut [u8], src: &[u8; 4], alpha: u8) {
    let sa = alpha as u16;
    let inv = 255 - sa;
    dst[0] = ((src[0] as u16 * sa + dst[0] as u16 * inv) / 255) as u8;
    dst[1] = ((src[1] as u16 * sa + dst[1] as u16 * inv) / 255) as u8;
    dst[2] = ((src[2] as u16 * sa + dst[2] as u16 * inv) / 255) as u8;
    dst[3] = (sa + dst[3] as u16 * inv / 255) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::Color;

    #[test]
    fn vello_backend_create_and_clear() {
        let mut backend = VelloBackend::new(100, 100);
        assert_eq!(backend.size(), (100, 100));
        backend.clear(Color::new(255, 0, 0, 255));
        assert_eq!(&backend.pixels()[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn vello_backend_fill_rect() {
        let mut backend = VelloBackend::new(50, 50);
        backend.clear(Color::new(0, 0, 0, 255));
        let rect = Rect::new(10.0, 10.0, 20.0, 20.0);
        let paint = RenderPaint::Solid(Color::new(255, 255, 255, 255));
        backend.fill_rect(rect, &paint, &Transform2D::identity());
        // Pixel at (15, 15) should be white
        let idx = ((15 * 50 + 15) * 4) as usize;
        assert_eq!(backend.pixels()[idx], 255);
    }

    #[test]
    fn vello_backend_is_not_gpu() {
        let backend = VelloBackend::new(10, 10);
        assert!(!backend.is_gpu_accelerated());
    }

    #[test]
    fn vello_backend_save_restore_opacity() {
        let mut backend = VelloBackend::new(10, 10);
        backend.set_opacity(0.5);
        backend.save();
        backend.set_opacity(0.2);
        backend.restore();
        assert!((backend.current_opacity - 0.5).abs() < 1e-10);
    }

    #[test]
    fn vello_backend_offscreen_roundtrip() {
        let mut backend = VelloBackend::new(10, 10);
        backend.clear(Color::new(0, 0, 0, 255));
        backend.push_offscreen();
        backend.clear(Color::new(255, 0, 0, 255));
        backend.pop_offscreen_with_opacity(1.0);
        // Should now see red
        assert_eq!(backend.pixels()[0], 255);
    }
}
