//! The `RenderBackend` trait -- an abstraction over 2D rendering surfaces.
#![allow(clippy::too_many_arguments)]

use sivgo_dom::{BlendMode, Color};
use sivgo_math::{BezPath, Rect, Transform2D};

use crate::types::{FillRule, RenderPaint, StrokeStyle};

/// A backend that can rasterize 2D shapes onto a pixel buffer.
///
/// Implementations include the CPU-based `TinySkiaBackend` and (in future)
/// GPU or Canvas2D backends.
pub trait RenderBackend {
    /// The width and height of the rendering surface in pixels.
    fn size(&self) -> (u32, u32);

    /// Clear the entire surface to a solid color.
    fn clear(&mut self, color: Color);

    // -- filled shapes -------------------------------------------------------

    /// Fill an axis-aligned rectangle.
    fn fill_rect(&mut self, rect: Rect, paint: &RenderPaint, transform: &Transform2D);

    /// Fill a general Bezier path.
    fn fill_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        fill_rule: FillRule,
        transform: &Transform2D,
    );

    /// Fill a circle (center + radius).
    fn fill_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    );

    /// Fill an ellipse (center + radii).
    fn fill_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    );

    // -- stroked shapes ------------------------------------------------------

    /// Stroke an axis-aligned rectangle.
    fn stroke_rect(
        &mut self,
        rect: Rect,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    );

    /// Stroke a general Bezier path.
    fn stroke_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    );

    /// Stroke a circle (center + radius).
    fn stroke_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    );

    /// Stroke an ellipse (center + radii).
    fn stroke_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    );

    // -- clipping / masking --------------------------------------------------

    /// Set a clip mask from a path. All subsequent drawing operations are
    /// restricted to the interior of this path until [`clear_clip`] is called.
    fn set_clip_path(&mut self, path: &BezPath, fill_rule: FillRule, transform: &Transform2D);

    /// Remove the current clip mask.
    fn clear_clip(&mut self);

    /// Set a luminance mask from raw RGBA pixel data.
    ///
    /// The `mask_pixels` contain premultiplied RGBA data of the same dimensions
    /// as this backend's surface. All subsequent drawing is multiplied by the
    /// luminance of these pixels.
    fn set_mask_from_pixmap_luminance(&mut self, mask_pixels: &[u8]);

    /// Remove the current mask.
    fn clear_mask(&mut self);

    // -- state management ----------------------------------------------------

    /// Set the global opacity for subsequent drawing operations.
    fn set_opacity(&mut self, opacity: f64);

    /// Save the current graphics state (opacity, clip, etc.) onto a stack.
    fn save(&mut self);

    /// Restore the most recently saved graphics state.
    fn restore(&mut self);

    // -- offscreen group rendering -------------------------------------------

    /// Begin rendering into an offscreen buffer of the same dimensions.
    ///
    /// All subsequent drawing operations target the offscreen buffer until
    /// [`pop_offscreen_with_opacity`] is called.
    fn push_offscreen(&mut self);

    /// Finish offscreen rendering and composite the offscreen buffer onto
    /// the previous surface with the given opacity.
    ///
    /// This must be paired with a preceding [`push_offscreen`] call.
    fn pop_offscreen_with_opacity(&mut self, opacity: f64);

    /// Finish offscreen rendering and composite the offscreen buffer onto
    /// the previous surface using the given SVG blend mode at full opacity.
    ///
    /// This must be paired with a preceding [`push_offscreen`] call.
    fn pop_offscreen_with_blend(&mut self, blend_mode: BlendMode);

    /// Pop the current offscreen buffer and return its raw **premultiplied RGBA**
    /// pixel data without compositing it. The previous surface becomes active again.
    ///
    /// Returns `(width, height, pixels)`. This is used for filter processing.
    fn pop_offscreen_raw(&mut self) -> (u32, u32, Vec<u8>);

    /// Composite raw premultiplied RGBA pixel data onto the current surface
    /// using source-over blending at full opacity.
    fn composite_pixels(&mut self, data: &[u8], width: u32, height: u32);

    // -- images --------------------------------------------------------------

    /// Draw a decoded RGBA image onto the surface.
    ///
    /// `data` is premultiplied RGBA pixel data of size `img_width * img_height * 4`.
    /// The image is drawn at position `(x, y)` scaled to `(w, h)` in user-space,
    /// then the `transform` is applied on top.
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
    );

    // -- pixel access --------------------------------------------------------

    /// Return the raw RGBA pixel data of the surface.
    fn pixels(&self) -> &[u8];
}
