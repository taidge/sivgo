//! Font metrics extraction.
//!
//! Queries a font for typographic metrics such as ascent, descent, x-height,
//! cap-height, and underline position.

use crate::engine::{FontStyleKind, TextEngine};

// ---------------------------------------------------------------------------
// FontMetrics
// ---------------------------------------------------------------------------

/// Typographic metrics for a specific font at a given size.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Distance from the baseline to the top of the tallest ascender (positive up).
    pub ascent: f64,
    /// Distance from the baseline to the bottom of the deepest descender (positive down, stored as positive value).
    pub descent: f64,
    /// Line height (ascent + descent + leading).
    pub line_height: f64,
    /// Height of the lowercase letter 'x' (0.0 if unavailable).
    pub x_height: f64,
    /// Height of a capital letter (0.0 if unavailable).
    pub cap_height: f64,
    /// Recommended underline offset from baseline (positive down).
    pub underline_offset: f64,
    /// Recommended underline thickness.
    pub underline_thickness: f64,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            ascent: 0.0,
            descent: 0.0,
            line_height: 0.0,
            x_height: 0.0,
            cap_height: 0.0,
            underline_offset: 0.0,
            underline_thickness: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Obtain typographic metrics for the given font parameters.
///
/// This shapes a tiny probe string to resolve the font, then queries the
/// underlying font tables via `swash`.
pub fn get_font_metrics(
    engine: &mut TextEngine,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
) -> FontMetrics {
    // Shape a probe character so cosmic-text resolves the actual font face.
    let shaped = engine.shape_text("Xp", font_family, font_size, font_weight, font_style);

    if let Some(glyph) = shaped.glyphs().first() {
        let font_id = glyph.font_id;
        let weight = glyph.font_weight;
        let fs = engine.font_system_mut();
        if let Some(font) = fs.get_font(font_id, weight) {
            let swash_ref = font.as_swash();
            let raw_metrics = swash_ref.metrics(&[]);
            let scale = font_size / raw_metrics.units_per_em as f64;

            return FontMetrics {
                ascent: raw_metrics.ascent as f64 * scale,
                descent: raw_metrics.descent.abs() as f64 * scale,
                line_height: (raw_metrics.ascent + raw_metrics.descent.abs() + raw_metrics.leading)
                    as f64
                    * scale,
                x_height: raw_metrics.x_height as f64 * scale,
                cap_height: raw_metrics.cap_height as f64 * scale,
                underline_offset: raw_metrics.underline_offset.abs() as f64 * scale,
                underline_thickness: raw_metrics.stroke_size as f64 * scale,
            };
        }
    }

    // Fallback: synthetic metrics based on font size alone.
    fallback_metrics(font_size)
}

/// Produce reasonable synthetic metrics when no font is available.
pub(crate) fn fallback_metrics(font_size: f64) -> FontMetrics {
    FontMetrics {
        ascent: font_size * 0.8,
        descent: font_size * 0.2,
        line_height: font_size * 1.2,
        x_height: font_size * 0.5,
        cap_height: font_size * 0.7,
        underline_offset: font_size * 0.1,
        underline_thickness: font_size * 0.05,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_reasonable() {
        let mut engine = TextEngine::new();
        let m = get_font_metrics(&mut engine, "sans-serif", 16.0, 400, FontStyleKind::Normal);
        assert!(m.ascent > 0.0, "Ascent should be positive");
        assert!(m.descent > 0.0, "Descent magnitude should be positive");
        assert!(m.line_height >= m.ascent, "Line height should be >= ascent");
    }

    #[test]
    fn test_metrics_scales_with_size() {
        let mut engine = TextEngine::new();
        let m12 = get_font_metrics(&mut engine, "sans-serif", 12.0, 400, FontStyleKind::Normal);
        let m24 = get_font_metrics(&mut engine, "sans-serif", 24.0, 400, FontStyleKind::Normal);
        assert!(
            m24.ascent > m12.ascent,
            "Larger font should have larger ascent"
        );
    }

    #[test]
    fn test_fallback_metrics() {
        let m = fallback_metrics(20.0);
        assert!((m.ascent - 16.0).abs() < 0.01);
        assert!((m.descent - 4.0).abs() < 0.01);
        assert!((m.line_height - 24.0).abs() < 0.01);
    }
}
