//! Render-specific types: paint, stroke style, fill rule.

use sivgo_dom::Color;
use sivgo_math::Transform2D;

/// Spread method for gradients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMethod {
    Pad,
    Reflect,
    Repeat,
}

/// Gradient coordinate units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientUnits {
    UserSpaceOnUse,
    ObjectBoundingBox,
}

/// A single stop in a gradient.
#[derive(Debug, Clone)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
    pub opacity: f64,
}

/// A paint used for filling or stroking shapes.
#[derive(Debug, Clone)]
pub enum RenderPaint {
    /// A solid color paint.
    Solid(Color),
    /// A linear gradient paint.
    LinearGradient {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: Vec<GradientStop>,
        spread: SpreadMethod,
        transform: Transform2D,
        units: GradientUnits,
    },
    /// A radial gradient paint.
    RadialGradient {
        cx: f64,
        cy: f64,
        r: f64,
        fx: f64,
        fy: f64,
        stops: Vec<GradientStop>,
        spread: SpreadMethod,
        transform: Transform2D,
        units: GradientUnits,
    },
    /// A tiled pattern paint.
    Pattern {
        /// Premultiplied RGBA pixel data for the pattern tile.
        pixels: Vec<u8>,
        /// Width of the pattern tile in pixels.
        tile_width: u32,
        /// Height of the pattern tile in pixels.
        tile_height: u32,
        /// The transform to apply to the pattern (patternTransform plus
        /// positioning offset). Maps from user-space to pattern-tile space.
        transform: Transform2D,
        /// Pattern opacity (0.0..=1.0).
        opacity: f64,
    },
    /// An SVG 2 mesh gradient, pre-rasterized into a pixmap.
    ///
    /// Because tiny-skia has no native mesh gradient shader, we rasterize
    /// the mesh into a pixel buffer and use it as a non-repeating pattern.
    MeshGradient {
        /// Premultiplied RGBA pixel data for the rasterized mesh.
        pixels: Vec<u8>,
        /// Width of the rasterized mesh in pixels.
        width: u32,
        /// Height of the rasterized mesh in pixels.
        height: u32,
        /// Transform from user-space into the mesh coordinate system.
        transform: Transform2D,
        /// Gradient units (objectBoundingBox or userSpaceOnUse).
        units: GradientUnits,
    },
}

impl RenderPaint {
    /// Create a solid-color paint.
    pub fn solid(color: Color) -> Self {
        RenderPaint::Solid(color)
    }

    /// Scale this paint's alpha by `opacity` (clamped to 0..=1).
    ///
    /// `fill-opacity` and `stroke-opacity` multiply the paint's own alpha
    /// rather than the element's overall opacity, so they have to be folded
    /// into the paint before it reaches the backend.
    #[must_use]
    pub fn with_opacity(self, opacity: f64) -> Self {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity >= 1.0 {
            return self;
        }
        // The cast saturates, so values stay within 0..=255.
        let scale = |a: u8| (f64::from(a) * opacity).round() as u8;
        match self {
            RenderPaint::Solid(c) => RenderPaint::Solid(Color::new(c.r, c.g, c.b, scale(c.a))),
            RenderPaint::LinearGradient {
                x1,
                y1,
                x2,
                y2,
                mut stops,
                spread,
                transform,
                units,
            } => {
                for stop in &mut stops {
                    stop.color.a = scale(stop.color.a);
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
                }
            }
            RenderPaint::RadialGradient {
                cx,
                cy,
                r,
                fx,
                fy,
                mut stops,
                spread,
                transform,
                units,
            } => {
                for stop in &mut stops {
                    stop.color.a = scale(stop.color.a);
                }
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
                }
            }
            RenderPaint::Pattern {
                pixels,
                tile_width,
                tile_height,
                transform,
                opacity: pattern_opacity,
            } => RenderPaint::Pattern {
                pixels,
                tile_width,
                tile_height,
                transform,
                opacity: pattern_opacity * opacity,
            },
            // The mesh is already rasterized; scaling it would mean rewriting
            // every pixel, which the callers do not need today.
            other @ RenderPaint::MeshGradient { .. } => other,
        }
    }
}

/// Describes how a path or shape should be stroked.
#[derive(Debug, Clone)]
pub struct StrokeStyle {
    pub width: f64,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f64,
    pub dash_array: Option<Vec<f64>>,
    pub dash_offset: f64,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            dash_array: None,
            dash_offset: 0.0,
        }
    }
}

/// Line cap style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Fill rule for determining the interior of a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
