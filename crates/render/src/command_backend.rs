//! Shared command-recording render backend used by non-TinySkia surfaces.
//!
//! Vello, Canvas2D, and WebGPU integrations need two things at the same time:
//! a backend-specific command stream that can be handed to a native/browser
//! renderer, and deterministic pixels for tests and headless export.  This
//! module records every `RenderBackend` call and replays it through TinySkia so
//! those surfaces are complete render backends even when the platform GPU
//! context is not attached.

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::tiny_skia_backend::TinySkiaBackend;
use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// The backend surface represented by a recorded command stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurface {
    /// A Vello/wgpu scene surface.
    Vello,
    /// An HTML Canvas 2D surface.
    Canvas2d,
    /// A browser WebGPU surface.
    WebGpu,
}

impl RenderSurface {
    /// Stable display name for diagnostics and UI capability panels.
    pub fn label(self) -> &'static str {
        match self {
            RenderSurface::Vello => "Vello",
            RenderSurface::Canvas2d => "Canvas2D",
            RenderSurface::WebGpu => "WebGPU",
        }
    }

    /// Browser API used by this surface, if any.
    pub fn browser_api(self) -> Option<&'static str> {
        match self {
            RenderSurface::Vello => None,
            RenderSurface::Canvas2d => Some("CanvasRenderingContext2D"),
            RenderSurface::WebGpu => Some("GPUCanvasContext"),
        }
    }
}

/// How a backend is executing in the current process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderExecutionMode {
    /// Native Vello/wgpu execution.
    NativeGpu,
    /// Browser Canvas2D execution.
    BrowserCanvas2d,
    /// Browser WebGPU execution.
    BrowserWebGpu,
    /// Deterministic software replay used for tests, export, and unsupported
    /// platform contexts.
    CpuReplay,
}

/// Capability and status data exposed by command-recorded backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderBackendCapabilities {
    pub surface: RenderSurface,
    pub execution_mode: RenderExecutionMode,
    pub hardware_accelerated: bool,
    pub command_stream: bool,
    pub deterministic_cpu_replay: bool,
    pub supports_paths: bool,
    pub supports_images: bool,
    pub supports_clips: bool,
    pub supports_masks: bool,
    pub supports_offscreen_groups: bool,
    pub browser_api: Option<&'static str>,
}

impl RenderBackendCapabilities {
    fn for_surface(surface: RenderSurface) -> Self {
        let execution_mode = match surface {
            RenderSurface::Vello => RenderExecutionMode::CpuReplay,
            RenderSurface::Canvas2d if cfg!(target_arch = "wasm32") => {
                RenderExecutionMode::BrowserCanvas2d
            }
            RenderSurface::WebGpu if cfg!(target_arch = "wasm32") => {
                RenderExecutionMode::BrowserWebGpu
            }
            RenderSurface::Canvas2d | RenderSurface::WebGpu => RenderExecutionMode::CpuReplay,
        };

        Self {
            surface,
            execution_mode,
            hardware_accelerated: matches!(
                execution_mode,
                RenderExecutionMode::NativeGpu | RenderExecutionMode::BrowserWebGpu
            ),
            command_stream: true,
            deterministic_cpu_replay: true,
            supports_paths: true,
            supports_images: true,
            supports_clips: true,
            supports_masks: true,
            supports_offscreen_groups: true,
            browser_api: surface.browser_api(),
        }
    }
}

/// Runtime status for a command-recorded render backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderBackendStatus {
    pub capabilities: RenderBackendCapabilities,
    pub width: u32,
    pub height: u32,
    pub recorded_commands: usize,
}

/// A serializable-enough representation of render operations for Vello,
/// Canvas2D, and WebGPU adapters.
#[derive(Debug, Clone)]
pub enum RecordedRenderCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        paint: RenderPaint,
        transform: Transform2D,
    },
    FillPath {
        path: BezPath,
        paint: RenderPaint,
        fill_rule: FillRule,
        transform: Transform2D,
    },
    FillCircle {
        cx: f64,
        cy: f64,
        r: f64,
        paint: RenderPaint,
        transform: Transform2D,
    },
    FillEllipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: RenderPaint,
        transform: Transform2D,
    },
    StrokeRect {
        rect: Rect,
        paint: RenderPaint,
        stroke: StrokeStyle,
        transform: Transform2D,
    },
    StrokePath {
        path: BezPath,
        paint: RenderPaint,
        stroke: StrokeStyle,
        transform: Transform2D,
    },
    StrokeCircle {
        cx: f64,
        cy: f64,
        r: f64,
        paint: RenderPaint,
        stroke: StrokeStyle,
        transform: Transform2D,
    },
    StrokeEllipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: RenderPaint,
        stroke: StrokeStyle,
        transform: Transform2D,
    },
    SetClipPath {
        path: BezPath,
        fill_rule: FillRule,
        transform: Transform2D,
    },
    ClearClip,
    SetMaskFromPixmapLuminance {
        pixels: Vec<u8>,
    },
    ClearMask,
    SetOpacity(f64),
    Save,
    Restore,
    PushOffscreen,
    PopOffscreenWithOpacity(f64),
    PopOffscreenWithBlend(BlendMode),
    PopOffscreenRaw,
    CompositePixels {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    DrawImage {
        data: Vec<u8>,
        img_width: u32,
        img_height: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        transform: Transform2D,
    },
}

pub(crate) struct RecordedRenderBackend {
    surface: RenderSurface,
    replay: TinySkiaBackend,
    commands: Vec<RecordedRenderCommand>,
}

impl RecordedRenderBackend {
    pub(crate) fn new(surface: RenderSurface, width: u32, height: u32) -> Self {
        Self {
            surface,
            replay: TinySkiaBackend::new(width, height),
            commands: Vec::new(),
        }
    }

    pub(crate) fn capabilities(&self) -> RenderBackendCapabilities {
        RenderBackendCapabilities::for_surface(self.surface)
    }

    pub(crate) fn status(&self) -> RenderBackendStatus {
        let (width, height) = self.size();
        RenderBackendStatus {
            capabilities: self.capabilities(),
            width,
            height,
            recorded_commands: self.commands.len(),
        }
    }

    pub(crate) fn commands(&self) -> &[RecordedRenderCommand] {
        &self.commands
    }

    pub(crate) fn clear_commands(&mut self) {
        self.commands.clear();
    }
}

impl RenderBackend for RecordedRenderBackend {
    fn size(&self) -> (u32, u32) {
        self.replay.size()
    }

    fn clear(&mut self, color: Color) {
        self.commands.push(RecordedRenderCommand::Clear(color));
        self.replay.clear(color);
    }

    fn fill_rect(&mut self, rect: Rect, paint: &RenderPaint, transform: &Transform2D) {
        self.commands.push(RecordedRenderCommand::FillRect {
            rect,
            paint: paint.clone(),
            transform: *transform,
        });
        self.replay.fill_rect(rect, paint, transform);
    }

    fn fill_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        fill_rule: FillRule,
        transform: &Transform2D,
    ) {
        self.commands.push(RecordedRenderCommand::FillPath {
            path: path.clone(),
            paint: paint.clone(),
            fill_rule,
            transform: *transform,
        });
        self.replay.fill_path(path, paint, fill_rule, transform);
    }

    fn fill_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    ) {
        self.commands.push(RecordedRenderCommand::FillCircle {
            cx,
            cy,
            r,
            paint: paint.clone(),
            transform: *transform,
        });
        self.replay.fill_circle(cx, cy, r, paint, transform);
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
        self.commands.push(RecordedRenderCommand::FillEllipse {
            cx,
            cy,
            rx,
            ry,
            paint: paint.clone(),
            transform: *transform,
        });
        self.replay.fill_ellipse(cx, cy, rx, ry, paint, transform);
    }

    fn stroke_rect(
        &mut self,
        rect: Rect,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.commands.push(RecordedRenderCommand::StrokeRect {
            rect,
            paint: paint.clone(),
            stroke: stroke.clone(),
            transform: *transform,
        });
        self.replay.stroke_rect(rect, paint, stroke, transform);
    }

    fn stroke_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        self.commands.push(RecordedRenderCommand::StrokePath {
            path: path.clone(),
            paint: paint.clone(),
            stroke: stroke.clone(),
            transform: *transform,
        });
        self.replay.stroke_path(path, paint, stroke, transform);
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
        self.commands.push(RecordedRenderCommand::StrokeCircle {
            cx,
            cy,
            r,
            paint: paint.clone(),
            stroke: stroke.clone(),
            transform: *transform,
        });
        self.replay
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
        self.commands.push(RecordedRenderCommand::StrokeEllipse {
            cx,
            cy,
            rx,
            ry,
            paint: paint.clone(),
            stroke: stroke.clone(),
            transform: *transform,
        });
        self.replay
            .stroke_ellipse(cx, cy, rx, ry, paint, stroke, transform);
    }

    fn set_clip_path(&mut self, path: &BezPath, fill_rule: FillRule, transform: &Transform2D) {
        self.commands.push(RecordedRenderCommand::SetClipPath {
            path: path.clone(),
            fill_rule,
            transform: *transform,
        });
        self.replay.set_clip_path(path, fill_rule, transform);
    }

    fn clear_clip(&mut self) {
        self.commands.push(RecordedRenderCommand::ClearClip);
        self.replay.clear_clip();
    }

    fn set_mask_from_pixmap_luminance(&mut self, mask_pixels: &[u8]) {
        self.commands
            .push(RecordedRenderCommand::SetMaskFromPixmapLuminance {
                pixels: mask_pixels.to_vec(),
            });
        self.replay.set_mask_from_pixmap_luminance(mask_pixels);
    }

    fn clear_mask(&mut self) {
        self.commands.push(RecordedRenderCommand::ClearMask);
        self.replay.clear_mask();
    }

    fn set_opacity(&mut self, opacity: f64) {
        self.commands
            .push(RecordedRenderCommand::SetOpacity(opacity));
        self.replay.set_opacity(opacity);
    }

    fn save(&mut self) {
        self.commands.push(RecordedRenderCommand::Save);
        self.replay.save();
    }

    fn restore(&mut self) {
        self.commands.push(RecordedRenderCommand::Restore);
        self.replay.restore();
    }

    fn push_offscreen(&mut self) {
        self.commands.push(RecordedRenderCommand::PushOffscreen);
        self.replay.push_offscreen();
    }

    fn pop_offscreen_with_opacity(&mut self, opacity: f64) {
        self.commands
            .push(RecordedRenderCommand::PopOffscreenWithOpacity(opacity));
        self.replay.pop_offscreen_with_opacity(opacity);
    }

    fn pop_offscreen_with_blend(&mut self, blend_mode: BlendMode) {
        self.commands
            .push(RecordedRenderCommand::PopOffscreenWithBlend(blend_mode));
        self.replay.pop_offscreen_with_blend(blend_mode);
    }

    fn pop_offscreen_raw(&mut self) -> (u32, u32, Vec<u8>) {
        self.commands.push(RecordedRenderCommand::PopOffscreenRaw);
        self.replay.pop_offscreen_raw()
    }

    fn composite_pixels(&mut self, data: &[u8], width: u32, height: u32) {
        self.commands.push(RecordedRenderCommand::CompositePixels {
            data: data.to_vec(),
            width,
            height,
        });
        self.replay.composite_pixels(data, width, height);
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
        self.commands.push(RecordedRenderCommand::DrawImage {
            data: data.to_vec(),
            img_width,
            img_height,
            x,
            y,
            w,
            h,
            transform: *transform,
        });
        self.replay
            .draw_image(data, img_width, img_height, x, y, w, h, transform);
    }

    fn pixels(&self) -> &[u8] {
        self.replay.pixels()
    }
}
