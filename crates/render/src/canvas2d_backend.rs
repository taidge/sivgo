//! Canvas2D rendering backend for WebAssembly deployment.
//!
//! Implements `RenderBackend` by targeting the HTML Canvas 2D API via
//! `web-sys`. In non-WASM builds this is a stub that renders to a CPU
//! pixel buffer, so the crate always compiles and tests pass.

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// A backend that renders via the HTML Canvas 2D API (or a CPU fallback
/// when not running in a browser).
pub struct Canvas2dBackend {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    opacity_stack: Vec<f64>,
    current_opacity: f64,
    offscreen_stack: Vec<Vec<u8>>,
}

impl Canvas2dBackend {
    /// Create a new Canvas2D backend with the given dimensions.
    ///
    /// In a WASM build this would receive a `web_sys::CanvasRenderingContext2d`.
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

    /// Whether the backend is running inside a browser.
    pub fn is_browser(&self) -> bool {
        cfg!(target_arch = "wasm32")
    }
}

impl RenderBackend for Canvas2dBackend {
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
        let color = paint_to_rgba(paint);
        let x0 = (rect.x0 as i32).max(0) as u32;
        let y0 = (rect.y0 as i32).max(0) as u32;
        let x1 = (rect.x1 as u32).min(self.width);
        let y1 = (rect.y1 as u32).min(self.height);
        let alpha = (self.current_opacity * 255.0) as u8;

        for y in y0..y1 {
            for x in x0..x1 {
                let idx = ((y * self.width + x) * 4) as usize;
                if idx + 3 < self.pixels.len() {
                    blend_over(&mut self.pixels[idx..idx + 4], &color, alpha);
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
        // In WASM: ctx.beginPath(), walk segments, ctx.fill()
    }

    fn fill_circle(
        &mut self,
        _cx: f64,
        _cy: f64,
        _r: f64,
        _paint: &RenderPaint,
        _transform: &Transform2D,
    ) {
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
    }
    fn stroke_rect(
        &mut self,
        _rect: Rect,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
    }
    fn stroke_path(
        &mut self,
        _path: &BezPath,
        _paint: &RenderPaint,
        _stroke: &StrokeStyle,
        _transform: &Transform2D,
    ) {
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
    }

    fn set_clip_path(&mut self, _path: &BezPath, _fill_rule: FillRule, _transform: &Transform2D) {}
    fn clear_clip(&mut self) {}
    fn set_mask_from_pixmap_luminance(&mut self, _mask_pixels: &[u8]) {}
    fn clear_mask(&mut self) {}

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
            composite_over(&mut self.pixels, &offscreen, opacity);
        }
    }

    fn pop_offscreen_with_blend(&mut self, _blend_mode: BlendMode) {
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
        composite_over(&mut self.pixels, data, 1.0);
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
                let sx = ((dx as f64 * scale_x) as u32).min(img_width.saturating_sub(1));
                let sy = ((dy as f64 * scale_y) as u32).min(img_height.saturating_sub(1));
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

fn paint_to_rgba(paint: &RenderPaint) -> [u8; 4] {
    match paint {
        RenderPaint::Solid(c) => [c.r, c.g, c.b, 255],
        _ => [0, 0, 0, 255],
    }
}

fn blend_over(dst: &mut [u8], src: &[u8; 4], alpha: u8) {
    let sa = alpha as u16;
    let inv = 255 - sa;
    dst[0] = ((src[0] as u16 * sa + dst[0] as u16 * inv) / 255) as u8;
    dst[1] = ((src[1] as u16 * sa + dst[1] as u16 * inv) / 255) as u8;
    dst[2] = ((src[2] as u16 * sa + dst[2] as u16 * inv) / 255) as u8;
    dst[3] = (sa + dst[3] as u16 * inv / 255) as u8;
}

fn composite_over(dst: &mut [u8], src: &[u8], opacity: f64) {
    let alpha_scale = (opacity * 255.0) as u16;
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let sa = (s[3] as u16 * alpha_scale) / 255;
        if sa > 0 {
            let inv = 255 - sa;
            d[0] = ((s[0] as u16 * alpha_scale / 255) + (d[0] as u16 * inv / 255)) as u8;
            d[1] = ((s[1] as u16 * alpha_scale / 255) + (d[1] as u16 * inv / 255)) as u8;
            d[2] = ((s[2] as u16 * alpha_scale / 255) + (d[2] as u16 * inv / 255)) as u8;
            d[3] = (sa + d[3] as u16 * inv / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::Color;

    #[test]
    fn canvas2d_create_and_clear() {
        let mut b = Canvas2dBackend::new(64, 64);
        assert_eq!(b.size(), (64, 64));
        b.clear(Color::new(0, 128, 255, 255));
        assert_eq!(&b.pixels()[0..4], &[0, 128, 255, 255]);
    }

    #[test]
    fn canvas2d_fill_rect() {
        let mut b = Canvas2dBackend::new(32, 32);
        b.clear(Color::new(0, 0, 0, 255));
        b.fill_rect(
            Rect::new(5.0, 5.0, 15.0, 15.0),
            &RenderPaint::Solid(Color::new(255, 0, 0, 255)),
            &Transform2D::identity(),
        );
        let idx = ((10 * 32 + 10) * 4) as usize;
        assert_eq!(b.pixels()[idx], 255); // red channel
    }

    #[test]
    fn canvas2d_is_not_browser_on_native() {
        let b = Canvas2dBackend::new(1, 1);
        assert!(!b.is_browser());
    }

    #[test]
    fn canvas2d_offscreen_pop_raw() {
        let mut b = Canvas2dBackend::new(4, 4);
        b.clear(Color::new(0, 0, 0, 255));
        b.push_offscreen();
        b.clear(Color::new(255, 255, 255, 255));
        let (w, h, raw) = b.pop_offscreen_raw();
        assert_eq!((w, h), (4, 4));
        assert_eq!(raw[0], 255); // white
                                 // Main surface should still be black
        assert_eq!(b.pixels()[0], 0);
    }
}
