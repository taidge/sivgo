//! Text shaping engine backed by `cosmic-text`.
//!
//! Provides system font loading, text shaping, and access to glyph positions.

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics as CMetrics, Shaping, Style, Weight};

// ---------------------------------------------------------------------------
// FontStyleKind
// ---------------------------------------------------------------------------

/// Font style selector (maps to CSS `font-style`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyleKind {
    #[default]
    Normal,
    Italic,
    Oblique,
}

// ---------------------------------------------------------------------------
// GlyphPosition
// ---------------------------------------------------------------------------

/// A single glyph positioned in the output text layout.
#[derive(Debug, Clone)]
pub struct GlyphPosition {
    /// Horizontal offset from the text origin.
    pub x: f64,
    /// Vertical offset (baseline-relative).
    pub y: f64,
    /// Advance width of this glyph.
    pub w: f64,
    /// The glyph identifier within the font.
    pub glyph_id: u16,
    /// The fontdb font identifier.
    pub font_id: cosmic_text::fontdb::ID,
    /// Font size used for this glyph.
    pub font_size: f32,
    /// Font weight used for this glyph.
    pub font_weight: Weight,
}

// ---------------------------------------------------------------------------
// ShapedText
// ---------------------------------------------------------------------------

/// The result of shaping a text string: glyph positions and overall dimensions.
pub struct ShapedText {
    glyphs: Vec<GlyphPosition>,
    width: f64,
    height: f64,
    line_height: f64,
}

impl ShapedText {
    /// Total width of the shaped text.
    #[inline]
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Total height of the shaped text.
    #[inline]
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Line height used for layout.
    #[inline]
    pub fn line_height(&self) -> f64 {
        self.line_height
    }

    /// Positioned glyphs produced by shaping.
    #[inline]
    pub fn glyphs(&self) -> &[GlyphPosition] {
        &self.glyphs
    }
}

// ---------------------------------------------------------------------------
// TextEngine
// ---------------------------------------------------------------------------

/// The main text engine.
///
/// Wraps a `cosmic_text::FontSystem` and provides a convenient API for
/// shaping text, querying font metrics, and (eventually) extracting glyph
/// outlines.
pub struct TextEngine {
    font_system: FontSystem,
}

impl TextEngine {
    /// Create a new text engine, loading system fonts.
    ///
    /// This is relatively expensive (~1 s on release builds) because it scans
    /// all installed fonts. Typically you create one engine and reuse it.
    pub fn new() -> Self {
        let font_system = FontSystem::new();
        Self { font_system }
    }

    /// Borrow the inner `FontSystem` mutably.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Borrow the inner `FontSystem`.
    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    /// Shape a text string and return positioned glyphs plus overall metrics.
    ///
    /// # Arguments
    /// * `text`        -- The string to shape.
    /// * `font_family` -- CSS-like family name (`"serif"`, `"monospace"`, or a face name).
    /// * `font_size`   -- Size in user-space units (SVG `px`).
    /// * `font_weight` -- CSS numeric weight (100..900; 400 = normal, 700 = bold).
    /// * `font_style`  -- Normal / Italic / Oblique.
    pub fn shape_text(
        &mut self,
        text: &str,
        font_family: &str,
        font_size: f64,
        font_weight: u16,
        font_style: FontStyleKind,
    ) -> ShapedText {
        let fs = font_size as f32;
        let lh = fs * 1.2;
        let metrics = CMetrics::new(fs, lh);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        // No wrapping -- single-line for SVG text.
        buffer.set_size(&mut self.font_system, None, None);

        let family = match font_family {
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            "sans-serif" => Family::SansSerif,
            name => Family::Name(name),
        };

        let style = match font_style {
            FontStyleKind::Normal => Style::Normal,
            FontStyleKind::Italic => Style::Italic,
            FontStyleKind::Oblique => Style::Oblique,
        };

        let attrs = Attrs::new()
            .family(family)
            .weight(Weight(font_weight))
            .style(style);

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Collect glyph positions from layout runs.
        let mut glyphs = Vec::new();
        let mut total_w: f64 = 0.0;
        let mut total_h: f64 = 0.0;

        for run in buffer.layout_runs() {
            for g in run.glyphs.iter() {
                glyphs.push(GlyphPosition {
                    x: g.x as f64,
                    y: run.line_y as f64,
                    w: g.w as f64,
                    glyph_id: g.glyph_id,
                    font_id: g.font_id,
                    font_size: g.font_size,
                    font_weight: g.font_weight,
                });
            }
            let run_end = run.line_w as f64;
            if run_end > total_w {
                total_w = run_end;
            }
            let bottom = run.line_top as f64 + run.line_height as f64;
            if bottom > total_h {
                total_h = bottom;
            }
        }

        ShapedText {
            glyphs,
            width: total_w,
            height: total_h,
            line_height: lh as f64,
        }
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let _engine = TextEngine::new();
    }

    #[test]
    fn test_shape_hello() {
        let mut engine = TextEngine::new();
        let shaped = engine.shape_text("Hello", "sans-serif", 16.0, 400, FontStyleKind::Normal);
        assert!(
            shaped.width() > 0.0,
            "Shaped text should have positive width"
        );
        assert!(!shaped.glyphs().is_empty(), "Should have glyphs");
    }

    #[test]
    fn test_shape_empty_string() {
        let mut engine = TextEngine::new();
        let shaped = engine.shape_text("", "sans-serif", 16.0, 400, FontStyleKind::Normal);
        assert!(
            shaped.glyphs().is_empty(),
            "Empty string should produce no glyphs"
        );
    }

    #[test]
    fn test_larger_font_wider() {
        let mut engine = TextEngine::new();
        let small = engine.shape_text(
            "Hello World",
            "sans-serif",
            12.0,
            400,
            FontStyleKind::Normal,
        );
        let large = engine.shape_text(
            "Hello World",
            "sans-serif",
            24.0,
            400,
            FontStyleKind::Normal,
        );
        assert!(
            large.width() > small.width(),
            "Larger font should produce wider text"
        );
    }
}
