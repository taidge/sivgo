//! CPU-based rendering backend using `tiny-skia`.

use tiny_skia::{
    FillRule as TsFillRule, LineCap as TsLineCap, LineJoin as TsLineJoin, Paint as TsPaint,
    PathBuilder, Pixmap, Stroke as TsStroke, Transform as TsTransform,
};

use sivgo_dom::{BlendMode as DomBlendMode, Color};
use sivgo_math::{BezPath, PathSegment, Rect, Transform2D};

use crate::backend::RenderBackend;
use crate::types::{
    FillRule, GradientStop as RenderGradientStop, GradientUnits, LineCap, LineJoin, RenderPaint,
    SpreadMethod, StrokeStyle,
};

fn anti_aliased_paint<'a>() -> TsPaint<'a> {
    TsPaint {
        anti_alias: true,
        ..TsPaint::default()
    }
}

// ---------------------------------------------------------------------------
// Graphics state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct GraphicsState {
    opacity: f64,
    clip_mask: Option<tiny_skia::Mask>,
    luminance_mask: Option<tiny_skia::Mask>,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            clip_mask: None,
            luminance_mask: None,
        }
    }
}

// ---------------------------------------------------------------------------
// TinySkiaBackend
// ---------------------------------------------------------------------------

/// A CPU-rasterized rendering backend backed by `tiny_skia::Pixmap`.
pub struct TinySkiaBackend {
    pixmap: Pixmap,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    /// Stack of saved pixmaps for offscreen group rendering.
    offscreen_stack: Vec<Pixmap>,
}

impl TinySkiaBackend {
    /// Create a new backend with a blank RGBA pixmap of the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let pixmap =
            Pixmap::new(width, height).expect("TinySkiaBackend: width and height must be > 0");
        Self {
            pixmap,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            offscreen_stack: Vec::new(),
        }
    }

    /// Borrow the underlying pixmap.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    // -- internal helpers ----------------------------------------------------

    /// Build a `tiny_skia::Paint` from our `RenderPaint`, applying current opacity.
    ///
    /// For gradient paints the `shape_bbox` is needed when the gradient uses
    /// `objectBoundingBox` units. Pass `None` for solid colours or when the
    /// bbox is not available (the gradient will fall back to identity mapping).
    fn make_paint<'a>(
        &self,
        render_paint: &'a RenderPaint,
        shape_bbox: Option<Rect>,
    ) -> TsPaint<'a> {
        match render_paint {
            RenderPaint::Solid(c) => {
                let alpha = (c.a as f64 / 255.0) * self.state.opacity;
                let mut paint = anti_aliased_paint();
                paint.set_color_rgba8(c.r, c.g, c.b, (alpha * 255.0).round() as u8);
                paint
            }
            RenderPaint::LinearGradient {
                x1,
                y1,
                x2,
                y2,
                stops,
                spread,
                transform,
                units,
            } => self.make_linear_gradient_paint(
                *x1, *y1, *x2, *y2, stops, *spread, transform, *units, shape_bbox,
            ),
            RenderPaint::RadialGradient {
                cx,
                cy,
                r,
                fx,
                fy,
                stops,
                spread,
                transform,
                units,
            } => self.make_radial_gradient_paint(
                *cx, *cy, *r, *fx, *fy, stops, *spread, transform, *units, shape_bbox,
            ),
            RenderPaint::Pattern {
                pixels,
                tile_width,
                tile_height,
                transform,
                opacity,
            } => self.make_pattern_paint(pixels, *tile_width, *tile_height, transform, *opacity),
            RenderPaint::MeshGradient {
                pixels,
                width,
                height,
                transform,
                units,
            } => self
                .make_mesh_gradient_paint(pixels, *width, *height, transform, *units, shape_bbox),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_linear_gradient_paint<'a>(
        &self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: &[RenderGradientStop],
        spread: SpreadMethod,
        grad_transform: &Transform2D,
        units: GradientUnits,
        shape_bbox: Option<Rect>,
    ) -> TsPaint<'a> {
        let ts_stops = convert_gradient_stops(stops, self.state.opacity);
        let spread_mode = convert_spread_mode(spread);

        // Compute the actual start/end points.
        let (start, end, extra_ts) = match units {
            GradientUnits::ObjectBoundingBox => {
                if let Some(bbox) = shape_bbox {
                    let bw = bbox.width();
                    let bh = bbox.height();
                    (
                        tiny_skia::Point::from_xy(
                            (bbox.x0 + x1 * bw) as f32,
                            (bbox.y0 + y1 * bh) as f32,
                        ),
                        tiny_skia::Point::from_xy(
                            (bbox.x0 + x2 * bw) as f32,
                            (bbox.y0 + y2 * bh) as f32,
                        ),
                        transform_to_tiny_skia(grad_transform),
                    )
                } else {
                    // Fallback: treat as userSpaceOnUse.
                    (
                        tiny_skia::Point::from_xy(x1 as f32, y1 as f32),
                        tiny_skia::Point::from_xy(x2 as f32, y2 as f32),
                        transform_to_tiny_skia(grad_transform),
                    )
                }
            }
            GradientUnits::UserSpaceOnUse => (
                tiny_skia::Point::from_xy(x1 as f32, y1 as f32),
                tiny_skia::Point::from_xy(x2 as f32, y2 as f32),
                transform_to_tiny_skia(grad_transform),
            ),
        };

        let shader = tiny_skia::LinearGradient::new(start, end, ts_stops, spread_mode, extra_ts);

        let mut paint = anti_aliased_paint();
        if let Some(shader) = shader {
            paint.shader = shader;
        }
        paint
    }

    #[allow(clippy::too_many_arguments)]
    fn make_radial_gradient_paint<'a>(
        &self,
        cx: f64,
        cy: f64,
        r: f64,
        fx: f64,
        fy: f64,
        stops: &[RenderGradientStop],
        spread: SpreadMethod,
        grad_transform: &Transform2D,
        units: GradientUnits,
        shape_bbox: Option<Rect>,
    ) -> TsPaint<'a> {
        let ts_stops = convert_gradient_stops(stops, self.state.opacity);
        let spread_mode = convert_spread_mode(spread);

        let (center, focal, radius, extra_ts) = match units {
            GradientUnits::ObjectBoundingBox => {
                if let Some(bbox) = shape_bbox {
                    let bw = bbox.width();
                    let bh = bbox.height();
                    (
                        tiny_skia::Point::from_xy(
                            (bbox.x0 + cx * bw) as f32,
                            (bbox.y0 + cy * bh) as f32,
                        ),
                        tiny_skia::Point::from_xy(
                            (bbox.x0 + fx * bw) as f32,
                            (bbox.y0 + fy * bh) as f32,
                        ),
                        (r * bw.max(bh)) as f32,
                        transform_to_tiny_skia(grad_transform),
                    )
                } else {
                    (
                        tiny_skia::Point::from_xy(cx as f32, cy as f32),
                        tiny_skia::Point::from_xy(fx as f32, fy as f32),
                        r as f32,
                        transform_to_tiny_skia(grad_transform),
                    )
                }
            }
            GradientUnits::UserSpaceOnUse => (
                tiny_skia::Point::from_xy(cx as f32, cy as f32),
                tiny_skia::Point::from_xy(fx as f32, fy as f32),
                r as f32,
                transform_to_tiny_skia(grad_transform),
            ),
        };

        let shader = tiny_skia::RadialGradient::new(
            focal,
            0.0,
            center,
            radius,
            ts_stops,
            spread_mode,
            extra_ts,
        );

        let mut paint = anti_aliased_paint();
        if let Some(shader) = shader {
            paint.shader = shader;
        }
        paint
    }

    /// Build a `tiny_skia::Paint` from a `RenderPaint::Pattern`.
    fn make_pattern_paint<'a>(
        &self,
        pixels: &'a [u8],
        tile_width: u32,
        tile_height: u32,
        pattern_transform: &Transform2D,
        opacity: f64,
    ) -> TsPaint<'a> {
        let mut paint = anti_aliased_paint();
        if tile_width == 0 || tile_height == 0 {
            return paint;
        }
        let expected_len = tile_width as usize * tile_height as usize * 4;
        if pixels.len() != expected_len {
            return paint;
        }
        let Some(pixmap_ref) = tiny_skia::PixmapRef::from_bytes(pixels, tile_width, tile_height)
        else {
            return paint;
        };
        let combined_opacity = (opacity * self.state.opacity).clamp(0.0, 1.0) as f32;
        let ts = transform_to_tiny_skia(pattern_transform);
        paint.shader = tiny_skia::Pattern::new(
            pixmap_ref,
            tiny_skia::SpreadMode::Repeat,
            tiny_skia::FilterQuality::Bilinear,
            combined_opacity,
            ts,
        );
        paint
    }

    /// Build a `tiny_skia::Paint` from a `RenderPaint::MeshGradient`.
    ///
    /// The mesh gradient has been pre-rasterized into a pixel buffer. We wrap
    /// it as a non-repeating pattern (using `SpreadMode::Pad` which clamps to
    /// edge pixels), positioned according to the gradient transform and units.
    #[allow(clippy::too_many_arguments)]
    fn make_mesh_gradient_paint<'a>(
        &self,
        pixels: &'a [u8],
        width: u32,
        height: u32,
        mesh_transform: &Transform2D,
        units: GradientUnits,
        shape_bbox: Option<Rect>,
    ) -> TsPaint<'a> {
        let mut paint = anti_aliased_paint();

        if width == 0 || height == 0 {
            return paint;
        }
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return paint;
        }

        let Some(pixmap_ref) = tiny_skia::PixmapRef::from_bytes(pixels, width, height) else {
            return paint;
        };

        // For objectBoundingBox units, scale the mesh coordinates by the
        // bounding box. The mesh_transform already contains the gradient's
        // offset + gradientTransform.
        let final_transform = match units {
            GradientUnits::ObjectBoundingBox => {
                if let Some(bbox) = shape_bbox {
                    let scale =
                        Transform2D::new([bbox.width(), 0.0, 0.0, bbox.height(), bbox.x0, bbox.y0]);
                    mesh_transform.then(&scale)
                } else {
                    *mesh_transform
                }
            }
            GradientUnits::UserSpaceOnUse => *mesh_transform,
        };

        let ts = transform_to_tiny_skia(&final_transform);
        let combined_opacity = self.state.opacity.clamp(0.0, 1.0) as f32;

        // Use Pad (clamp) mode so the mesh doesn't tile/repeat.
        paint.shader = tiny_skia::Pattern::new(
            pixmap_ref,
            tiny_skia::SpreadMode::Pad,
            tiny_skia::FilterQuality::Bilinear,
            combined_opacity,
            ts,
        );
        paint
    }

    /// Build a `tiny_skia::Stroke` from our `StrokeStyle`.
    fn make_stroke(style: &StrokeStyle) -> TsStroke {
        let mut stroke = TsStroke {
            width: style.width as f32,
            miter_limit: style.miter_limit as f32,
            line_cap: convert_line_cap(style.line_cap),
            line_join: convert_line_join(style.line_join),
            dash: None,
        };
        if let Some(ref dashes) = style.dash_array {
            if !dashes.is_empty() {
                let dash_array: Vec<f32> = dashes.iter().map(|d| *d as f32).collect();
                stroke.dash = tiny_skia::StrokeDash::new(dash_array, style.dash_offset as f32);
            }
        }
        stroke
    }

    /// Compute a rough axis-aligned bounding box for a rectangle after transform.
    fn rect_bbox(rect: &Rect, transform: &Transform2D) -> Rect {
        transform.transform_rect(*rect)
    }

    /// Compute a rough axis-aligned bounding box for an ellipse.
    fn ellipse_bbox(cx: f64, cy: f64, rx: f64, ry: f64, transform: &Transform2D) -> Rect {
        let r = Rect::new(cx - rx, cy - ry, cx + rx, cy + ry);
        transform.transform_rect(r)
    }
}

// ---------------------------------------------------------------------------
// RenderBackend implementation
// ---------------------------------------------------------------------------

impl RenderBackend for TinySkiaBackend {
    fn size(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }

    fn clear(&mut self, color: Color) {
        self.pixmap.fill(tiny_skia::Color::from_rgba8(
            color.r, color.g, color.b, color.a,
        ));
    }

    // -- fills ---------------------------------------------------------------

    fn fill_rect(&mut self, rect: Rect, paint: &RenderPaint, transform: &Transform2D) {
        let Some(path) = rect_to_tiny_skia_path(&rect) else {
            return;
        };
        let bbox = Self::rect_bbox(&rect, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .fill_path(&path, &ts_paint, TsFillRule::Winding, ts_transform, mask);
    }

    fn fill_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        fill_rule: FillRule,
        transform: &Transform2D,
    ) {
        let Some(ts_path) = bez_path_to_tiny_skia(path) else {
            return;
        };
        let bbox = compute_bezpath_bbox(path, transform);
        let ts_paint = self.make_paint(paint, bbox);
        let ts_transform = transform_to_tiny_skia(transform);
        let ts_fill_rule = match fill_rule {
            FillRule::NonZero => TsFillRule::Winding,
            FillRule::EvenOdd => TsFillRule::EvenOdd,
        };
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .fill_path(&ts_path, &ts_paint, ts_fill_rule, ts_transform, mask);
    }

    fn fill_circle(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        paint: &RenderPaint,
        transform: &Transform2D,
    ) {
        let Some(path) = ellipse_to_tiny_skia_path(cx, cy, r, r) else {
            return;
        };
        let bbox = Self::ellipse_bbox(cx, cy, r, r, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .fill_path(&path, &ts_paint, TsFillRule::Winding, ts_transform, mask);
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
        let Some(path) = ellipse_to_tiny_skia_path(cx, cy, rx, ry) else {
            return;
        };
        let bbox = Self::ellipse_bbox(cx, cy, rx, ry, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .fill_path(&path, &ts_paint, TsFillRule::Winding, ts_transform, mask);
    }

    // -- strokes -------------------------------------------------------------

    fn stroke_rect(
        &mut self,
        rect: Rect,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        let Some(path) = rect_to_tiny_skia_path(&rect) else {
            return;
        };
        let bbox = Self::rect_bbox(&rect, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_stroke = Self::make_stroke(stroke);
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .stroke_path(&path, &ts_paint, &ts_stroke, ts_transform, mask);
    }

    fn stroke_path(
        &mut self,
        path: &BezPath,
        paint: &RenderPaint,
        stroke: &StrokeStyle,
        transform: &Transform2D,
    ) {
        let Some(ts_path) = bez_path_to_tiny_skia(path) else {
            return;
        };
        let bbox = compute_bezpath_bbox(path, transform);
        let ts_paint = self.make_paint(paint, bbox);
        let ts_stroke = Self::make_stroke(stroke);
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .stroke_path(&ts_path, &ts_paint, &ts_stroke, ts_transform, mask);
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
        let Some(path) = ellipse_to_tiny_skia_path(cx, cy, r, r) else {
            return;
        };
        let bbox = Self::ellipse_bbox(cx, cy, r, r, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_stroke = Self::make_stroke(stroke);
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .stroke_path(&path, &ts_paint, &ts_stroke, ts_transform, mask);
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
        let Some(path) = ellipse_to_tiny_skia_path(cx, cy, rx, ry) else {
            return;
        };
        let bbox = Self::ellipse_bbox(cx, cy, rx, ry, transform);
        let ts_paint = self.make_paint(paint, Some(bbox));
        let ts_stroke = Self::make_stroke(stroke);
        let ts_transform = transform_to_tiny_skia(transform);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        self.pixmap
            .stroke_path(&path, &ts_paint, &ts_stroke, ts_transform, mask);
    }

    // -- clipping / masking --------------------------------------------------

    fn set_clip_path(&mut self, path: &BezPath, fill_rule: FillRule, transform: &Transform2D) {
        let (w, h) = self.size();
        let Some(ts_path) = bez_path_to_tiny_skia(path) else {
            return;
        };
        let ts_fill_rule = match fill_rule {
            FillRule::NonZero => TsFillRule::Winding,
            FillRule::EvenOdd => TsFillRule::EvenOdd,
        };
        let ts_transform = transform_to_tiny_skia(transform);

        let mut mask = match tiny_skia::Mask::new(w, h) {
            Some(m) => m,
            None => return,
        };
        // Fill the mask to all-white first, then intersect with the path.
        // `Mask::new` initializes to 0 (all blocked), so we use `fill_path`.
        mask.fill_path(&ts_path, ts_fill_rule, true, ts_transform);

        // If there's already a clip mask, intersect.
        if let Some(ref existing) = self.state.clip_mask {
            let data = mask.data_mut();
            let existing_data = existing.data();
            for (m, e) in data.iter_mut().zip(existing_data.iter()) {
                // Multiply: both must be visible.
                *m = ((*m as u16 * *e as u16 + 127) / 255) as u8;
            }
        }

        self.state.clip_mask = Some(mask);
    }

    fn clear_clip(&mut self) {
        self.state.clip_mask = None;
    }

    fn set_mask_from_pixmap_luminance(&mut self, mask_pixels: &[u8]) {
        let (w, h) = self.size();
        let expected_len = w as usize * h as usize * 4;
        if mask_pixels.len() != expected_len {
            return;
        }

        // Create a Pixmap from the provided data, then convert to a luminance mask.
        let pixmap_ref = tiny_skia::PixmapRef::from_bytes(mask_pixels, w, h);
        if let Some(pixmap_ref) = pixmap_ref {
            let mask = tiny_skia::Mask::from_pixmap(pixmap_ref, tiny_skia::MaskType::Luminance);
            self.state.luminance_mask = Some(mask);
        }
    }

    fn clear_mask(&mut self) {
        self.state.luminance_mask = None;
    }

    // -- state ---------------------------------------------------------------

    fn set_opacity(&mut self, opacity: f64) {
        self.state.opacity = opacity.clamp(0.0, 1.0);
    }

    fn save(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    fn restore(&mut self) {
        if let Some(prev) = self.state_stack.pop() {
            self.state = prev;
        }
    }

    fn push_offscreen(&mut self) {
        let (w, h) = self.size();
        // Create a fresh transparent pixmap for offscreen rendering.
        let offscreen = Pixmap::new(w, h).expect("offscreen pixmap: size must be > 0");
        // Swap: the new transparent pixmap becomes the active drawing target,
        // and the previous (main) pixmap is saved on the stack.
        let main_pixmap = std::mem::replace(&mut self.pixmap, offscreen);
        self.offscreen_stack.push(main_pixmap);
    }

    fn pop_offscreen_with_opacity(&mut self, opacity: f64) {
        let Some(mut main_pixmap) = self.offscreen_stack.pop() else {
            return;
        };
        // Composite the offscreen buffer onto the main pixmap with the given opacity.
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        main_pixmap.draw_pixmap(
            0,
            0,
            self.pixmap.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity: opacity as f32,
                blend_mode: tiny_skia::BlendMode::SourceOver,
                quality: tiny_skia::FilterQuality::Nearest,
            },
            TsTransform::identity(),
            mask,
        );
        // Restore the main pixmap as the active drawing target.
        self.pixmap = main_pixmap;
    }

    fn pop_offscreen_with_blend(&mut self, blend_mode: sivgo_dom::BlendMode) {
        let Some(mut main_pixmap) = self.offscreen_stack.pop() else {
            return;
        };
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        main_pixmap.draw_pixmap(
            0,
            0,
            self.pixmap.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity: 1.0,
                blend_mode: convert_blend_mode(blend_mode),
                quality: tiny_skia::FilterQuality::Nearest,
            },
            TsTransform::identity(),
            mask,
        );
        self.pixmap = main_pixmap;
    }

    fn pop_offscreen_raw(&mut self) -> (u32, u32, Vec<u8>) {
        let Some(main_pixmap) = self.offscreen_stack.pop() else {
            let (w, h) = self.size();
            return (w, h, vec![0u8; (w * h * 4) as usize]);
        };
        let w = self.pixmap.width();
        let h = self.pixmap.height();
        let pixels = self.pixmap.data().to_vec();
        self.pixmap = main_pixmap;
        (w, h, pixels)
    }

    fn composite_pixels(&mut self, data: &[u8], width: u32, height: u32) {
        let expected = width as usize * height as usize * 4;
        if data.len() != expected || width == 0 || height == 0 {
            return;
        }
        let Some(src) = Pixmap::from_vec(
            data.to_vec(),
            tiny_skia::IntSize::from_wh(width, height).unwrap(),
        ) else {
            return;
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            src.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity: 1.0,
                blend_mode: tiny_skia::BlendMode::SourceOver,
                quality: tiny_skia::FilterQuality::Nearest,
            },
            TsTransform::identity(),
            None,
        );
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
        // Validate input data length.
        let expected = img_width as usize * img_height as usize * 4;
        if data.len() != expected || img_width == 0 || img_height == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }

        // Build a temporary Pixmap from the decoded RGBA data.
        let Some(src_pixmap) = Pixmap::from_vec(
            data.to_vec(),
            tiny_skia::IntSize::from_wh(img_width, img_height).unwrap(),
        ) else {
            return;
        };

        // Compute the transform that maps source pixels to device pixels:
        //   1. Scale source pixels to target size (w, h)
        //   2. Translate to position (x, y) in user-space
        //   3. Apply the document/parent transform to reach device-space
        let sx = w / img_width as f64;
        let sy = h / img_height as f64;
        let combined = Transform2D::scale_xy(sx, sy)
            .then(&Transform2D::translate(x, y))
            .then(transform);

        let ts_transform = transform_to_tiny_skia(&combined);
        let mask = self
            .state
            .clip_mask
            .as_ref()
            .or(self.state.luminance_mask.as_ref());
        let opacity = self.state.opacity;

        self.pixmap.draw_pixmap(
            0,
            0,
            src_pixmap.as_ref(),
            &tiny_skia::PixmapPaint {
                opacity: opacity as f32,
                blend_mode: tiny_skia::BlendMode::SourceOver,
                quality: tiny_skia::FilterQuality::Bilinear,
            },
            ts_transform,
            mask,
        );
    }

    fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert an `sivgo_math::Transform2D` to a `tiny_skia::Transform`.
pub fn transform_to_tiny_skia(t: &Transform2D) -> TsTransform {
    let c = t.as_coeffs();
    // kurbo stores [a, b, c, d, e, f] in the same order as SVG matrix(a,b,c,d,e,f).
    // tiny_skia::Transform uses sx=a, kx=c, ky=b, sy=d, tx=e, ty=f.
    TsTransform::from_row(
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        c[4] as f32,
        c[5] as f32,
    )
}

/// Convert an `sivgo_math::BezPath` to a `tiny_skia::Path`.
///
/// Returns `None` if the path is empty or cannot be built.
pub fn bez_path_to_tiny_skia(path: &BezPath) -> Option<tiny_skia::Path> {
    let segments = path.elements();
    if segments.is_empty() {
        return None;
    }

    let mut pb = PathBuilder::new();
    for seg in &segments {
        match seg {
            PathSegment::MoveTo(p) => {
                pb.move_to(p.x as f32, p.y as f32);
            }
            PathSegment::LineTo(p) => {
                pb.line_to(p.x as f32, p.y as f32);
            }
            PathSegment::QuadTo(ctrl, end) => {
                pb.quad_to(ctrl.x as f32, ctrl.y as f32, end.x as f32, end.y as f32);
            }
            PathSegment::CurveTo(c1, c2, end) => {
                pb.cubic_to(
                    c1.x as f32,
                    c1.y as f32,
                    c2.x as f32,
                    c2.y as f32,
                    end.x as f32,
                    end.y as f32,
                );
            }
            PathSegment::ClosePath => {
                pb.close();
            }
        }
    }
    pb.finish()
}

/// Build a `tiny_skia::Path` for an axis-aligned rectangle.
fn rect_to_tiny_skia_path(r: &Rect) -> Option<tiny_skia::Path> {
    let ts_rect = tiny_skia::Rect::from_ltrb(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32)?;
    Some(PathBuilder::from_rect(ts_rect))
}

/// Build a `tiny_skia::Path` for an ellipse using the standard 4-cubic-Bezier
/// approximation.
fn ellipse_to_tiny_skia_path(cx: f64, cy: f64, rx: f64, ry: f64) -> Option<tiny_skia::Path> {
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }

    // Magic number for circular approximation: (4/3)(sqrt(2) - 1) ~ 0.5522847498
    const K: f64 = 0.5522847498;
    let kx = rx * K;
    let ky = ry * K;

    let mut pb = PathBuilder::new();
    // Start at the right-most point
    pb.move_to((cx + rx) as f32, cy as f32);
    // Top-right quarter
    pb.cubic_to(
        (cx + rx) as f32,
        (cy - ky) as f32,
        (cx + kx) as f32,
        (cy - ry) as f32,
        cx as f32,
        (cy - ry) as f32,
    );
    // Top-left quarter
    pb.cubic_to(
        (cx - kx) as f32,
        (cy - ry) as f32,
        (cx - rx) as f32,
        (cy - ky) as f32,
        (cx - rx) as f32,
        cy as f32,
    );
    // Bottom-left quarter
    pb.cubic_to(
        (cx - rx) as f32,
        (cy + ky) as f32,
        (cx - kx) as f32,
        (cy + ry) as f32,
        cx as f32,
        (cy + ry) as f32,
    );
    // Bottom-right quarter
    pb.cubic_to(
        (cx + kx) as f32,
        (cy + ry) as f32,
        (cx + rx) as f32,
        (cy + ky) as f32,
        (cx + rx) as f32,
        cy as f32,
    );
    pb.close();
    pb.finish()
}

/// Convert our `LineCap` to `tiny_skia::LineCap`.
fn convert_line_cap(cap: LineCap) -> TsLineCap {
    match cap {
        LineCap::Butt => TsLineCap::Butt,
        LineCap::Round => TsLineCap::Round,
        LineCap::Square => TsLineCap::Square,
    }
}

/// Convert our `LineJoin` to `tiny_skia::LineJoin`.
fn convert_line_join(join: LineJoin) -> TsLineJoin {
    match join {
        LineJoin::Miter => TsLineJoin::Miter,
        LineJoin::Round => TsLineJoin::Round,
        LineJoin::Bevel => TsLineJoin::Bevel,
    }
}

/// Convert render gradient stops to tiny-skia stops, applying opacity.
fn convert_gradient_stops(
    stops: &[RenderGradientStop],
    global_opacity: f64,
) -> Vec<tiny_skia::GradientStop> {
    stops
        .iter()
        .map(|s| {
            let a = (s.color.a as f64 / 255.0) * s.opacity * global_opacity;
            let color = tiny_skia::Color::from_rgba8(
                s.color.r,
                s.color.g,
                s.color.b,
                (a * 255.0).round() as u8,
            );
            tiny_skia::GradientStop::new(s.offset as f32, color)
        })
        .collect()
}

/// Convert our `SpreadMethod` to `tiny_skia::SpreadMode`.
fn convert_spread_mode(spread: SpreadMethod) -> tiny_skia::SpreadMode {
    match spread {
        SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    }
}

/// Compute a rough bounding box for a BezPath (untransformed min/max), then
/// apply the transform.
fn compute_bezpath_bbox(path: &BezPath, transform: &Transform2D) -> Option<Rect> {
    let segments = path.elements();
    if segments.is_empty() {
        return None;
    }
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for seg in &segments {
        let points: &[sivgo_math::Point] = match seg {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => std::slice::from_ref(p),
            PathSegment::QuadTo(a, b) => {
                // handle individually below
                if a.x < min_x {
                    min_x = a.x;
                }
                if a.y < min_y {
                    min_y = a.y;
                }
                if a.x > max_x {
                    max_x = a.x;
                }
                if a.y > max_y {
                    max_y = a.y;
                }
                std::slice::from_ref(b)
            }
            PathSegment::CurveTo(a, b, c) => {
                for p in [a, b] {
                    if p.x < min_x {
                        min_x = p.x;
                    }
                    if p.y < min_y {
                        min_y = p.y;
                    }
                    if p.x > max_x {
                        max_x = p.x;
                    }
                    if p.y > max_y {
                        max_y = p.y;
                    }
                }
                std::slice::from_ref(c)
            }
            PathSegment::ClosePath => continue,
        };
        for p in points {
            if p.x < min_x {
                min_x = p.x;
            }
            if p.y < min_y {
                min_y = p.y;
            }
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y > max_y {
                max_y = p.y;
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return None;
    }
    let raw = Rect::new(min_x, min_y, max_x, max_y);
    Some(transform.transform_rect(raw))
}

/// Convert the DOM `BlendMode` to the corresponding `tiny_skia::BlendMode`.
fn convert_blend_mode(mode: DomBlendMode) -> tiny_skia::BlendMode {
    match mode {
        DomBlendMode::Normal => tiny_skia::BlendMode::SourceOver,
        DomBlendMode::Multiply => tiny_skia::BlendMode::Multiply,
        DomBlendMode::Screen => tiny_skia::BlendMode::Screen,
        DomBlendMode::Overlay => tiny_skia::BlendMode::Overlay,
        DomBlendMode::Darken => tiny_skia::BlendMode::Darken,
        DomBlendMode::Lighten => tiny_skia::BlendMode::Lighten,
        DomBlendMode::ColorDodge => tiny_skia::BlendMode::ColorDodge,
        DomBlendMode::ColorBurn => tiny_skia::BlendMode::ColorBurn,
        DomBlendMode::HardLight => tiny_skia::BlendMode::HardLight,
        DomBlendMode::SoftLight => tiny_skia::BlendMode::SoftLight,
        DomBlendMode::Difference => tiny_skia::BlendMode::Difference,
        DomBlendMode::Exclusion => tiny_skia::BlendMode::Exclusion,
        DomBlendMode::Hue => tiny_skia::BlendMode::Hue,
        DomBlendMode::Saturation => tiny_skia::BlendMode::Saturation,
        DomBlendMode::Color => tiny_skia::BlendMode::Color,
        DomBlendMode::Luminosity => tiny_skia::BlendMode::Luminosity,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    #[test]
    fn backend_create_and_clear() {
        let mut backend = TinySkiaBackend::new(100, 100);
        assert_eq!(backend.size(), (100, 100));

        backend.clear(Color::WHITE);
        let px = backend.pixels();
        // First pixel should be opaque white (RGBA premultiplied)
        assert_eq!(px[0], 255); // R
        assert_eq!(px[1], 255); // G
        assert_eq!(px[2], 255); // B
        assert_eq!(px[3], 255); // A
    }

    #[test]
    fn fill_rect_produces_pixels() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        let rect = Rect::new(10.0, 10.0, 50.0, 50.0);
        let paint = RenderPaint::solid(Color::RED);
        let transform = Transform2D::identity();

        backend.fill_rect(rect, &paint, &transform);

        // Sample a pixel inside the rect (center ~30, 30)
        let idx = (30 * 100 + 30) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 255, "red channel should be 255");
        assert_eq!(px[idx + 1], 0, "green channel should be 0");
        assert_eq!(px[idx + 2], 0, "blue channel should be 0");
        assert_eq!(px[idx + 3], 255, "alpha channel should be 255");
    }

    #[test]
    fn fill_circle_produces_pixels() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        let paint = RenderPaint::solid(Color::BLUE);
        let transform = Transform2D::identity();
        backend.fill_circle(50.0, 50.0, 20.0, &paint, &transform);

        // Sample center pixel
        let idx = (50 * 100 + 50) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 0, "red should be 0");
        assert_eq!(px[idx + 1], 0, "green should be 0");
        assert_eq!(px[idx + 2], 255, "blue should be 255");
        assert_eq!(px[idx + 3], 255, "alpha should be 255");
    }

    #[test]
    fn fill_ellipse_produces_pixels() {
        let mut backend = TinySkiaBackend::new(200, 100);
        backend.clear(Color::WHITE);

        let paint = RenderPaint::solid(Color::GREEN);
        let transform = Transform2D::identity();
        backend.fill_ellipse(100.0, 50.0, 80.0, 30.0, &paint, &transform);

        // Sample center pixel
        let idx = (50 * 200 + 100) * 4;
        let px = backend.pixels();
        // sivgo_dom GREEN is (0, 128, 0)
        assert_eq!(px[idx], 0, "red should be 0");
        assert_eq!(px[idx + 1], 128, "green should be 128");
        assert_eq!(px[idx + 2], 0, "blue should be 0");
        assert_eq!(px[idx + 3], 255, "alpha should be 255");
    }

    #[test]
    fn stroke_rect_produces_pixels() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        let rect = Rect::new(10.0, 10.0, 90.0, 90.0);
        let paint = RenderPaint::solid(Color::BLACK);
        let stroke = StrokeStyle {
            width: 2.0,
            ..StrokeStyle::default()
        };
        let transform = Transform2D::identity();

        backend.stroke_rect(rect, &paint, &stroke, &transform);

        // Sample a pixel on the top edge of the rect (~y=10, x=50)
        let idx = (10 * 100 + 50) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 0, "should be black");
        assert_eq!(px[idx + 1], 0);
        assert_eq!(px[idx + 2], 0);
        assert_eq!(px[idx + 3], 255);
    }

    #[test]
    fn fill_path_triangle() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 90.0));
        path.line_to(Point::new(50.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();

        let paint = RenderPaint::solid(Color::RED);
        let transform = Transform2D::identity();
        backend.fill_path(&path, &paint, FillRule::NonZero, &transform);

        // Center-ish pixel should be red
        let idx = (50 * 100 + 50) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 255);
        assert_eq!(px[idx + 1], 0);
        assert_eq!(px[idx + 2], 0);
        assert_eq!(px[idx + 3], 255);
    }

    #[test]
    fn save_restore_opacity() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        backend.save();
        backend.set_opacity(0.5);

        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let paint = RenderPaint::solid(Color::BLACK);
        backend.fill_rect(rect, &paint, &Transform2D::identity());

        backend.restore();

        // After restore, opacity should be back to 1.0
        let rect2 = Rect::new(40.0, 40.0, 60.0, 60.0);
        let paint2 = RenderPaint::solid(Color::RED);
        backend.fill_rect(rect2, &paint2, &Transform2D::identity());

        // The inner red rect at (50, 50) should be fully opaque red
        let idx = (50 * 100 + 50) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 255);
        assert_eq!(px[idx + 3], 255);
    }

    #[test]
    fn transform_translates_rect() {
        let mut backend = TinySkiaBackend::new(200, 200);
        backend.clear(Color::WHITE);

        // Draw a rect at (0,0)-(20,20) with a translate(50, 50) transform
        let rect = Rect::new(0.0, 0.0, 20.0, 20.0);
        let paint = RenderPaint::solid(Color::RED);
        let transform = Transform2D::translate(50.0, 50.0);
        backend.fill_rect(rect, &paint, &transform);

        // The rect should appear at (50,50)-(70,70)
        // Pixel at (60, 60) should be red
        let idx = (60 * 200 + 60) * 4;
        let px = backend.pixels();
        assert_eq!(px[idx], 255, "translated rect center should be red");

        // Pixel at (10, 10) should still be white (untouched)
        let idx2 = (10 * 200 + 10) * 4;
        assert_eq!(px[idx2], 255);
        assert_eq!(px[idx2 + 1], 255);
        assert_eq!(px[idx2 + 2], 255);
    }

    #[test]
    fn bez_path_conversion_empty() {
        let path = BezPath::new();
        assert!(bez_path_to_tiny_skia(&path).is_none());
    }

    #[test]
    fn transform_identity_conversion() {
        let t = Transform2D::identity();
        let ts = transform_to_tiny_skia(&t);
        assert_eq!(ts, TsTransform::identity());
    }

    #[test]
    fn linear_gradient_paint_renders() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        let paint = RenderPaint::LinearGradient {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 0.0,
            stops: vec![
                RenderGradientStop {
                    offset: 0.0,
                    color: Color::RED,
                    opacity: 1.0,
                },
                RenderGradientStop {
                    offset: 1.0,
                    color: Color::BLUE,
                    opacity: 1.0,
                },
            ],
            spread: SpreadMethod::Pad,
            transform: Transform2D::identity(),
            units: GradientUnits::UserSpaceOnUse,
        };
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        backend.fill_rect(rect, &paint, &Transform2D::identity());

        let px = backend.pixels();
        // Left edge should be reddish
        let left_idx = (50 * 100 + 5) * 4;
        assert!(px[left_idx] > 200, "left should have high red");
        assert!(px[left_idx + 2] < 50, "left should have low blue");

        // Right edge should be blueish
        let right_idx = (50 * 100 + 95) * 4;
        assert!(px[right_idx] < 50, "right should have low red");
        assert!(px[right_idx + 2] > 200, "right should have high blue");
    }

    #[test]
    fn clip_path_restricts_drawing() {
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        // Set a circular clip in the center.
        let mut clip = BezPath::new();
        clip.move_to(Point::new(50.0, 20.0));
        clip.line_to(Point::new(80.0, 80.0));
        clip.line_to(Point::new(20.0, 80.0));
        clip.close();

        backend.set_clip_path(&clip, FillRule::NonZero, &Transform2D::identity());

        // Fill the entire canvas red.
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let paint = RenderPaint::solid(Color::RED);
        backend.fill_rect(rect, &paint, &Transform2D::identity());

        let px = backend.pixels();
        // Center of triangle (50, 60) should be red.
        let center_idx = (60 * 100 + 50) * 4;
        assert_eq!(px[center_idx], 255, "inside clip should be red");
        assert_eq!(px[center_idx + 1], 0);

        // Corner (5, 5) should still be white (outside clip).
        let corner_idx = (5 * 100 + 5) * 4;
        assert_eq!(px[corner_idx], 255, "outside clip should be white R");
        assert_eq!(px[corner_idx + 1], 255, "outside clip should be white G");
        assert_eq!(px[corner_idx + 2], 255, "outside clip should be white B");

        backend.clear_clip();
    }

    #[test]
    fn push_pop_offscreen_with_opacity() {
        // Test that offscreen rendering composites the buffer with the
        // given opacity, rather than applying opacity per-element.
        //
        // Setup: white background, two overlapping red rects drawn into
        // an offscreen buffer, then composited at 50% opacity.
        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(Color::WHITE);

        // Push offscreen: subsequent drawing goes to a transparent buffer.
        backend.push_offscreen();

        // Draw two overlapping red rectangles at full opacity into the
        // offscreen buffer.
        let paint = RenderPaint::solid(Color::RED);
        let identity = Transform2D::identity();
        backend.fill_rect(Rect::new(10.0, 10.0, 60.0, 60.0), &paint, &identity);
        backend.fill_rect(Rect::new(40.0, 40.0, 90.0, 90.0), &paint, &identity);

        // Pop offscreen and composite at 50% opacity.
        backend.pop_offscreen_with_opacity(0.5);

        let px = backend.pixels();

        // In the overlap region (50, 50): the offscreen buffer has a
        // solid red pixel (from two overlapping red rects). This is then
        // composited at 50% opacity onto white, giving approximately:
        //   R = 255 * 0.5 + 255 * 0.5 = 255  (but it's 128 red over white)
        //   Actually: src_over blend with 50% opacity red onto white:
        //     R = 255*0.5 + 255*(1-0.5) = 127.5 + 127.5 = 255 ... wait
        //   tiny-skia uses premultiplied alpha. With opacity=0.5:
        //     src is red (255,0,0,255) from buffer, composited at 0.5 opacity
        //     result R = 255*0.5 + 255*(1-0.5) = ~255
        //   Hmm, let me think more carefully:
        //   draw_pixmap with opacity=0.5 means the source pixmap alpha is
        //   multiplied by 0.5. So the source becomes (128,0,0,128) in premul.
        //   Then src_over: R = 128 + 255*(1 - 128/255) = 128 + 127 = 255
        //   G = 0 + 255*(127/255) = 127
        //   Actually the standard src_over formula with premul alpha:
        //   out = src + dst * (1 - src_a)
        //   src_a = 128/255 ≈ 0.502
        //   R_out = 128 + 255 * (1 - 0.502) ≈ 128 + 127 = 255
        //   G_out = 0 + 255 * (1 - 0.502) ≈ 127
        //   B_out = 0 + 255 * (1 - 0.502) ≈ 127
        //
        // The key assertion: the overlap region should look the SAME as
        // the non-overlap region, because both were solid red in the
        // offscreen buffer before compositing.
        let overlap_idx = (50 * 100 + 50) * 4;
        let non_overlap_idx = (30 * 100 + 30) * 4; // inside first rect only

        // Both regions should have the same pixel values since they were
        // both solid red in the offscreen buffer.
        assert_eq!(
            px[overlap_idx], px[non_overlap_idx],
            "overlap and non-overlap R should match"
        );
        assert_eq!(
            px[overlap_idx + 1],
            px[non_overlap_idx + 1],
            "overlap and non-overlap G should match"
        );
        assert_eq!(
            px[overlap_idx + 2],
            px[non_overlap_idx + 2],
            "overlap and non-overlap B should match"
        );
        assert_eq!(
            px[overlap_idx + 3],
            px[non_overlap_idx + 3],
            "overlap and non-overlap A should match"
        );

        // The non-overlap red area should not be fully opaque red
        // (it was blended at 50%).
        // R channel: should be around 255 (red blended onto white)
        // G and B channels: should be around 127 (white showing through)
        assert!(
            px[non_overlap_idx + 1] > 100 && px[non_overlap_idx + 1] < 160,
            "G should be ~127 from 50% blend, got {}",
            px[non_overlap_idx + 1]
        );
        assert!(
            px[non_overlap_idx + 2] > 100 && px[non_overlap_idx + 2] < 160,
            "B should be ~127 from 50% blend, got {}",
            px[non_overlap_idx + 2]
        );

        // Outside both rects should still be white.
        let outside_idx = (5 * 100 + 5) * 4;
        assert_eq!(px[outside_idx], 255, "outside should be white R");
        assert_eq!(px[outside_idx + 1], 255, "outside should be white G");
        assert_eq!(px[outside_idx + 2], 255, "outside should be white B");
    }

    #[test]
    fn offscreen_vs_per_element_opacity_differ() {
        // Demonstrate that offscreen group opacity produces different results
        // from per-element opacity when elements overlap.
        let identity = Transform2D::identity();
        let paint = RenderPaint::solid(Color::RED);

        // Method 1: Per-element opacity (the old, incorrect way for groups).
        let mut per_elem = TinySkiaBackend::new(100, 100);
        per_elem.clear(Color::WHITE);
        per_elem.save();
        per_elem.set_opacity(0.5);
        per_elem.fill_rect(Rect::new(10.0, 10.0, 60.0, 60.0), &paint, &identity);
        per_elem.fill_rect(Rect::new(40.0, 40.0, 90.0, 90.0), &paint, &identity);
        per_elem.restore();

        // Method 2: Offscreen group opacity (the correct way).
        let mut offscreen = TinySkiaBackend::new(100, 100);
        offscreen.clear(Color::WHITE);
        offscreen.push_offscreen();
        offscreen.fill_rect(Rect::new(10.0, 10.0, 60.0, 60.0), &paint, &identity);
        offscreen.fill_rect(Rect::new(40.0, 40.0, 90.0, 90.0), &paint, &identity);
        offscreen.pop_offscreen_with_opacity(0.5);

        let per_px = per_elem.pixels();
        let off_px = offscreen.pixels();

        // In the overlap region, per-element opacity should produce a
        // darker (more red) pixel than offscreen, because two 50%-opaque
        // red rects stacked create more red than one 50%-opaque solid red.
        let overlap_idx = (50 * 100 + 50) * 4;
        let non_overlap_idx = (30 * 100 + 30) * 4;

        // Per-element: overlap should differ from non-overlap.
        assert_ne!(
            per_px[overlap_idx + 1],
            per_px[non_overlap_idx + 1],
            "per-element: overlap G should differ from non-overlap G"
        );

        // Offscreen: overlap should equal non-overlap (both solid red in buffer).
        assert_eq!(
            off_px[overlap_idx + 1],
            off_px[non_overlap_idx + 1],
            "offscreen: overlap G should equal non-overlap G"
        );
    }

    #[test]
    fn pop_offscreen_with_blend_multiply() {
        // Test that pop_offscreen_with_blend composites using the specified
        // blend mode instead of the default source-over.
        //
        // Multiply blend: result = src * dst (per channel, normalised).
        // With white background (255,255,255) and red foreground (255,0,0):
        //   R = 255*255/255 = 255
        //   G = 0*255/255 = 0
        //   B = 0*255/255 = 0
        // So Multiply of red onto white should give pure red -- same as
        // source-over. But Multiply of red onto green is different:
        //   R = 255*0/255 = 0
        //   G = 0*128/255 = 0
        //   B = 0*0/255 = 0 → black
        //
        // We'll compare Multiply vs SourceOver (Normal) to show they differ.

        let identity = Transform2D::identity();
        let green_paint = RenderPaint::solid(Color {
            r: 0,
            g: 200,
            b: 0,
            a: 255,
        });
        let _white_paint = RenderPaint::solid(Color::WHITE);

        // Method 1: Render red rect with Normal blend (source-over) onto green bg.
        let mut normal = TinySkiaBackend::new(100, 100);
        normal.clear(Color::WHITE);
        normal.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &green_paint, &identity);
        normal.push_offscreen();
        normal.fill_rect(
            Rect::new(20.0, 20.0, 80.0, 80.0),
            &RenderPaint::solid(Color::RED),
            &identity,
        );
        normal.pop_offscreen_with_blend(DomBlendMode::Normal);

        // Method 2: Render red rect with Multiply blend onto green bg.
        let mut multiply = TinySkiaBackend::new(100, 100);
        multiply.clear(Color::WHITE);
        multiply.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &green_paint, &identity);
        multiply.push_offscreen();
        multiply.fill_rect(
            Rect::new(20.0, 20.0, 80.0, 80.0),
            &RenderPaint::solid(Color::RED),
            &identity,
        );
        multiply.pop_offscreen_with_blend(DomBlendMode::Multiply);

        let normal_px = normal.pixels();
        let mult_px = multiply.pixels();

        // In the red rect region (50, 50):
        // Normal (source-over): red covers green → (255, 0, 0)
        // Multiply: R = min(255*0/255, ...) → the red channel of src (255) * dst R (0) / 255 = 0
        //   Actually multiply of a fully opaque red rect on green:
        //   src = (255, 0, 0, 255), dst = (0, 200, 0, 255)
        //   result R = 255*0/255 = 0
        //   result G = 0*200/255 = 0
        //   result B = 0*0/255 = 0
        //   → black (or near-black)
        let idx = (50 * 100 + 50) * 4;

        // Normal: should be red (255, 0, 0)
        assert_eq!(normal_px[idx], 255, "normal R should be 255");
        assert_eq!(normal_px[idx + 1], 0, "normal G should be 0");

        // Multiply: should be very different from normal -- near black.
        // The red channel should be 0 (since dst green has R=0).
        assert!(
            mult_px[idx] < 10,
            "multiply R should be near 0, got {}",
            mult_px[idx]
        );
        assert!(
            mult_px[idx + 1] < 10,
            "multiply G should be near 0, got {}",
            mult_px[idx + 1]
        );

        // Outside the red rect (5, 5) should be the same for both (green bg).
        let out_idx = (5 * 100 + 5) * 4;
        assert_eq!(
            normal_px[out_idx], mult_px[out_idx],
            "outside rect: R should match"
        );
        assert_eq!(
            normal_px[out_idx + 1],
            mult_px[out_idx + 1],
            "outside rect: G should match"
        );
    }

    #[test]
    fn pop_offscreen_with_blend_screen() {
        // Screen blend: result = src + dst - src * dst
        // With dark blue background and dark red foreground, screen should
        // produce a lighter result than source-over.
        let identity = Transform2D::identity();
        let bg_color = Color {
            r: 0,
            g: 0,
            b: 150,
            a: 255,
        };
        let fg_color = Color {
            r: 150,
            g: 0,
            b: 0,
            a: 255,
        };

        let mut backend = TinySkiaBackend::new(100, 100);
        backend.clear(bg_color);
        backend.push_offscreen();
        backend.fill_rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            &RenderPaint::solid(fg_color),
            &identity,
        );
        backend.pop_offscreen_with_blend(DomBlendMode::Screen);

        let px = backend.pixels();
        let idx = (50 * 100 + 50) * 4;
        // Screen: R = 150 + 0 - 150*0/255 = 150
        //         B = 0 + 150 - 0*150/255 = 150
        // Both channels should be present (unlike source-over which would just
        // show the foreground).
        assert!(
            px[idx] > 100,
            "screen R should be substantial, got {}",
            px[idx]
        );
        assert!(
            px[idx + 2] > 100,
            "screen B should be substantial, got {}",
            px[idx + 2]
        );
    }
}
