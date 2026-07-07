//! SVG text layout.
//!
//! Computes positioned glyphs for `<text>` elements, respecting SVG text
//! positioning attributes (`x`, `y`, `text-anchor`, etc.).

use crate::engine::{FontStyleKind, TextEngine};
use crate::metrics::get_font_metrics;
use sivgo_math::Rect;

// ---------------------------------------------------------------------------
// TextAnchor
// ---------------------------------------------------------------------------

/// SVG `text-anchor` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

// ---------------------------------------------------------------------------
// PositionedGlyph
// ---------------------------------------------------------------------------

/// A glyph placed at a concrete position in user-space.
#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    /// X position in user-space.
    pub x: f64,
    /// Y position in user-space (baseline).
    pub y: f64,
    /// Advance width of this glyph.
    pub advance: f64,
    /// The original character that produced this glyph.
    pub character: char,
    /// Glyph identifier within the font.
    pub glyph_id: u16,
    /// fontdb font identifier.
    pub font_id: cosmic_text::fontdb::ID,
    /// Font size.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: cosmic_text::Weight,
}

// ---------------------------------------------------------------------------
// TextLayout
// ---------------------------------------------------------------------------

/// The result of laying out SVG text.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Positioned glyphs ready for rendering or path conversion.
    pub glyphs: Vec<PositionedGlyph>,
    /// Tight bounding box of the laid-out text.
    pub bounds: Rect,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lay out a text string for an SVG `<text>` element.
///
/// This combines cosmic-text shaping with SVG text positioning rules:
///
/// 1. Shape the text with the given font parameters.
/// 2. Apply `text-anchor` offset.
/// 3. Position all glyphs relative to the supplied `(x, y)` origin.
///
/// The returned [`TextLayout`] contains the final positioned glyphs and a
/// bounding rectangle.
#[allow(clippy::too_many_arguments)]
pub fn layout_svg_text(
    engine: &mut TextEngine,
    text: &str,
    x: f64,
    y: f64,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    anchor: TextAnchor,
    letter_spacing: f64,
    word_spacing: f64,
) -> TextLayout {
    if text.is_empty() {
        return TextLayout {
            glyphs: Vec::new(),
            bounds: Rect::ZERO,
        };
    }

    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);

    // Get metrics for proper baseline placement.
    let metrics = get_font_metrics(engine, font_family, font_size, font_weight, font_style);

    // Build positioned glyphs, applying letter-spacing and word-spacing.
    let chars: Vec<char> = text.chars().collect();
    let shaped_glyphs = shaped.glyphs();

    let mut positioned = Vec::with_capacity(shaped_glyphs.len());
    let mut extra_offset = 0.0_f64;

    for (i, sg) in shaped_glyphs.iter().enumerate() {
        let ch = if i < chars.len() {
            chars[i]
        } else {
            '\u{FFFD}'
        };

        // Apply word-spacing: extra space added before each space character
        // (except the first glyph — consistent with SVG spec behavior).
        if i > 0 && ch.is_whitespace() {
            extra_offset += word_spacing;
        }

        positioned.push(PositionedGlyph {
            x: x + sg.x + extra_offset,
            y: y + sg.y - metrics.ascent,
            advance: sg.w,
            character: ch,
            glyph_id: sg.glyph_id,
            font_id: sg.font_id,
            font_size: sg.font_size,
            font_weight: sg.font_weight,
        });

        // Apply letter-spacing: added after each character (not after the last).
        if i < shaped_glyphs.len() - 1 {
            extra_offset += letter_spacing;
        }
    }

    // Compute effective width including spacing adjustments.
    let width = if positioned.is_empty() {
        0.0
    } else {
        let last = positioned.last().unwrap();
        (last.x + last.advance) - positioned[0].x
    };

    // Apply text-anchor offset (must be computed after spacing adjustment).
    let anchor_offset = match anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -width / 2.0,
        TextAnchor::End => -width,
    };

    if anchor_offset != 0.0 {
        for g in &mut positioned {
            g.x += anchor_offset;
        }
    }

    // Compute bounding box.
    let bounds = if positioned.is_empty() {
        Rect::ZERO
    } else {
        let min_x = positioned.iter().map(|g| g.x).fold(f64::INFINITY, f64::min);
        let max_x = positioned
            .iter()
            .map(|g| g.x + g.advance)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = y - metrics.ascent;
        let max_y = y + metrics.descent;
        Rect::new(min_x, min_y, max_x, max_y)
    };

    TextLayout {
        glyphs: positioned,
        bounds,
    }
}

/// Lay out a text string with per-character SVG positioning arrays.
///
/// This implements the SVG text positioning model where each character can have
/// an absolute x/y position and/or a relative dx/dy offset:
///
/// - `x_list[i]`: if present, sets the absolute x position of the *i*-th character.
/// - `y_list[i]`: if present, sets the absolute y position of the *i*-th character.
/// - `dx_list[i]`: if present, adds a **cumulative** horizontal offset starting at
///   the *i*-th character.
/// - `dy_list[i]`: if present, adds a **cumulative** vertical offset starting at
///   the *i*-th character.
///
/// Characters beyond the end of any list simply retain the position produced by
/// normal shaping.
///
/// The returned [`TextLayout`] contains the final positioned glyphs and a bounding
/// rectangle.
#[allow(clippy::too_many_arguments)]
pub fn layout_svg_text_positioned(
    engine: &mut TextEngine,
    text: &str,
    base_x: f64,
    base_y: f64,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    anchor: TextAnchor,
    letter_spacing: f64,
    word_spacing: f64,
    x_list: &[f64],
    y_list: &[f64],
    dx_list: &[f64],
    dy_list: &[f64],
) -> TextLayout {
    if text.is_empty() {
        return TextLayout {
            glyphs: Vec::new(),
            bounds: Rect::ZERO,
        };
    }

    // Fast path: if all positioning lists are trivially empty or single-element,
    // delegate to the simpler function.
    if x_list.len() <= 1 && y_list.len() <= 1 && dx_list.is_empty() && dy_list.is_empty() {
        let x = x_list.first().copied().unwrap_or(base_x);
        let y = y_list.first().copied().unwrap_or(base_y);
        return layout_svg_text(
            engine,
            text,
            x,
            y,
            font_family,
            font_size,
            font_weight,
            font_style,
            anchor,
            letter_spacing,
            word_spacing,
        );
    }

    // Shape the text to get baseline glyph advances.
    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);
    let metrics = get_font_metrics(engine, font_family, font_size, font_weight, font_style);

    let chars: Vec<char> = text.chars().collect();
    let shaped_glyphs = shaped.glyphs();

    // Build positioned glyphs by walking forward with a cursor.
    //
    // SVG text positioning rules (simplified):
    //   - If x_list[i] exists, set cursor_x = x_list[i] (absolute).
    //   - If y_list[i] exists, set cursor_y = y_list[i] (absolute).
    //   - If dx_list[i] exists, cursor_x += dx_list[i] (cumulative shift).
    //   - If dy_list[i] exists, cursor_y += dy_list[i] (cumulative shift).
    //   - Place glyph at (cursor_x, cursor_y), then advance cursor_x by the
    //     glyph advance + letter-spacing (+ word-spacing for whitespace).
    let mut cursor_x = x_list.first().copied().unwrap_or(base_x);
    let mut cursor_y = y_list.first().copied().unwrap_or(base_y);

    let mut positioned = Vec::with_capacity(shaped_glyphs.len());

    for (i, sg) in shaped_glyphs.iter().enumerate() {
        let ch = if i < chars.len() {
            chars[i]
        } else {
            '\u{FFFD}'
        };

        // Apply absolute positioning if available for this character.
        if i < x_list.len() {
            cursor_x = x_list[i];
        }
        if i < y_list.len() {
            cursor_y = y_list[i];
        }

        // Apply cumulative dx/dy offsets.
        if i < dx_list.len() {
            cursor_x += dx_list[i];
        }
        if i < dy_list.len() {
            cursor_y += dy_list[i];
        }

        // Word spacing: extra space before whitespace characters (not the first).
        if i > 0 && ch.is_whitespace() {
            cursor_x += word_spacing;
        }

        positioned.push(PositionedGlyph {
            x: cursor_x,
            y: cursor_y + sg.y - metrics.ascent,
            advance: sg.w,
            character: ch,
            glyph_id: sg.glyph_id,
            font_id: sg.font_id,
            font_size: sg.font_size,
            font_weight: sg.font_weight,
        });

        // Advance cursor by glyph width + letter spacing.
        cursor_x += sg.w;
        if i < shaped_glyphs.len() - 1 {
            cursor_x += letter_spacing;
        }
    }

    // Compute effective width for text-anchor.
    let width = if positioned.is_empty() {
        0.0
    } else {
        let last = positioned.last().unwrap();
        (last.x + last.advance) - positioned[0].x
    };

    // Apply text-anchor offset.
    let anchor_offset = match anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -width / 2.0,
        TextAnchor::End => -width,
    };

    if anchor_offset != 0.0 {
        for g in &mut positioned {
            g.x += anchor_offset;
        }
    }

    // Compute bounding box.
    let bounds = if positioned.is_empty() {
        Rect::ZERO
    } else {
        let min_x = positioned.iter().map(|g| g.x).fold(f64::INFINITY, f64::min);
        let max_x = positioned
            .iter()
            .map(|g| g.x + g.advance)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positioned.iter().map(|g| g.y).fold(f64::INFINITY, f64::min);
        let max_y = positioned
            .iter()
            .map(|g| g.y + metrics.ascent + metrics.descent)
            .fold(f64::NEG_INFINITY, f64::max);
        Rect::new(min_x, min_y, max_x, max_y)
    };

    TextLayout {
        glyphs: positioned,
        bounds,
    }
}

/// Measure the width of a text string without computing a full layout.
///
/// This is a lightweight wrapper around shaping for scenarios where only the
/// total advance width is needed (e.g., for `textLength` or hit testing).
#[allow(clippy::too_many_arguments)]
pub fn measure_text_width(
    engine: &mut TextEngine,
    text: &str,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    letter_spacing: f64,
    word_spacing: f64,
) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);
    let base_width = shaped.width();

    // Add letter spacing: (n-1) gaps for n characters.
    let char_count = text.chars().count();
    let extra_letter = if char_count > 1 {
        letter_spacing * (char_count - 1) as f64
    } else {
        0.0
    };

    // Add word spacing: one extra per whitespace character.
    let space_count = text.chars().filter(|c| c.is_whitespace()).count();
    let extra_word = word_spacing * space_count as f64;

    base_width + extra_letter + extra_word
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> TextEngine {
        TextEngine::new()
    }

    #[test]
    fn test_layout_empty() {
        let mut engine = make_engine();
        let layout = layout_svg_text(
            &mut engine,
            "",
            100.0,
            200.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn test_layout_has_glyphs() {
        let mut engine = make_engine();
        let layout = layout_svg_text(
            &mut engine,
            "Hello",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        assert!(!layout.glyphs.is_empty());
        assert!(!layout.bounds.is_empty());
    }

    #[test]
    fn test_anchor_middle_centers() {
        let mut engine = make_engine();
        let start_layout = layout_svg_text(
            &mut engine,
            "Test",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let mid_layout = layout_svg_text(
            &mut engine,
            "Test",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Middle,
            0.0,
            0.0,
        );

        if !start_layout.glyphs.is_empty() && !mid_layout.glyphs.is_empty() {
            assert!(
                mid_layout.glyphs[0].x < start_layout.glyphs[0].x,
                "Middle-anchored first glyph should be to the left"
            );
        }
    }

    #[test]
    fn test_anchor_end_shifts_left() {
        let mut engine = make_engine();
        let start_layout = layout_svg_text(
            &mut engine,
            "Test",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let end_layout = layout_svg_text(
            &mut engine,
            "Test",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::End,
            0.0,
            0.0,
        );

        if !start_layout.glyphs.is_empty() && !end_layout.glyphs.is_empty() {
            assert!(
                end_layout.glyphs[0].x < start_layout.glyphs[0].x,
                "End-anchored first glyph should be further left"
            );
        }
    }

    #[test]
    fn test_measure_text_width() {
        let mut engine = make_engine();
        let w = measure_text_width(
            &mut engine,
            "Hello World",
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
        );
        assert!(w > 0.0, "Measured width should be positive");
    }

    #[test]
    fn test_letter_spacing_widens_text() {
        let mut engine = make_engine();
        let normal = layout_svg_text(
            &mut engine,
            "Hello",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let spaced = layout_svg_text(
            &mut engine,
            "Hello",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            5.0,
            0.0,
        );

        if !normal.glyphs.is_empty() && !spaced.glyphs.is_empty() {
            assert!(
                spaced.bounds.width() > normal.bounds.width(),
                "Letter-spaced text should be wider: {} vs {}",
                spaced.bounds.width(),
                normal.bounds.width()
            );
        }
    }

    #[test]
    fn test_word_spacing_widens_text() {
        let mut engine = make_engine();
        let normal = layout_svg_text(
            &mut engine,
            "Hello World",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let spaced = layout_svg_text(
            &mut engine,
            "Hello World",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            20.0,
        );

        if !normal.glyphs.is_empty() && !spaced.glyphs.is_empty() {
            assert!(
                spaced.bounds.width() > normal.bounds.width(),
                "Word-spaced text should be wider: {} vs {}",
                spaced.bounds.width(),
                normal.bounds.width()
            );
        }
    }

    #[test]
    fn test_measure_width_with_spacing() {
        let mut engine = make_engine();
        let normal = measure_text_width(
            &mut engine,
            "A B",
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
        );
        let with_letter = measure_text_width(
            &mut engine,
            "A B",
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            10.0,
            0.0,
        );
        let with_word = measure_text_width(
            &mut engine,
            "A B",
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            10.0,
        );
        assert!(with_letter > normal);
        assert!(with_word > normal);
    }

    // -----------------------------------------------------------------------
    // Tests for layout_svg_text_positioned
    // -----------------------------------------------------------------------

    #[test]
    fn test_positioned_empty_lists_delegates() {
        let mut engine = make_engine();
        // With empty lists, should behave identically to layout_svg_text.
        let normal = layout_svg_text(
            &mut engine,
            "ABC",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let positioned = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[],
            &[],
            &[],
            &[],
        );
        assert_eq!(normal.glyphs.len(), positioned.glyphs.len());
        for (a, b) in normal.glyphs.iter().zip(positioned.glyphs.iter()) {
            assert!((a.x - b.x).abs() < 0.01, "x mismatch: {} vs {}", a.x, b.x);
            assert!((a.y - b.y).abs() < 0.01, "y mismatch: {} vs {}", a.y, b.y);
        }
    }

    #[test]
    fn test_positioned_single_x_y_delegates() {
        let mut engine = make_engine();
        // Single x/y list element should behave like layout_svg_text(x, y, ...).
        let normal = layout_svg_text(
            &mut engine,
            "ABC",
            20.0,
            60.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let positioned = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            0.0,
            0.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[20.0],
            &[60.0],
            &[],
            &[],
        );
        assert_eq!(normal.glyphs.len(), positioned.glyphs.len());
        for (a, b) in normal.glyphs.iter().zip(positioned.glyphs.iter()) {
            assert!((a.x - b.x).abs() < 0.01, "x mismatch: {} vs {}", a.x, b.x);
            assert!((a.y - b.y).abs() < 0.01, "y mismatch: {} vs {}", a.y, b.y);
        }
    }

    #[test]
    fn test_positioned_x_array_places_each_char() {
        let mut engine = make_engine();
        // <text x="10 50 90">ABC</text> — each character at a specific x.
        let layout = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0, 50.0, 90.0],
            &[50.0],
            &[],
            &[],
        );
        assert_eq!(layout.glyphs.len(), 3);
        assert!(
            (layout.glyphs[0].x - 10.0).abs() < 0.01,
            "A should be at x=10, got {}",
            layout.glyphs[0].x
        );
        assert!(
            (layout.glyphs[1].x - 50.0).abs() < 0.01,
            "B should be at x=50, got {}",
            layout.glyphs[1].x
        );
        assert!(
            (layout.glyphs[2].x - 90.0).abs() < 0.01,
            "C should be at x=90, got {}",
            layout.glyphs[2].x
        );
    }

    #[test]
    fn test_positioned_y_array_places_each_char() {
        let mut engine = make_engine();
        // <text x="10" y="20 40 60">ABC</text> — each character at a different y.
        let layout = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            10.0,
            0.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0],
            &[20.0, 40.0, 60.0],
            &[],
            &[],
        );
        assert_eq!(layout.glyphs.len(), 3);
        // The y differences between characters should match the y_list differences.
        let dy_01 = layout.glyphs[1].y - layout.glyphs[0].y;
        let dy_12 = layout.glyphs[2].y - layout.glyphs[1].y;
        assert!(
            (dy_01 - 20.0).abs() < 0.01,
            "y gap between chars 0 and 1 should be 20, got {}",
            dy_01
        );
        assert!(
            (dy_12 - 20.0).abs() < 0.01,
            "y gap between chars 1 and 2 should be 20, got {}",
            dy_12
        );
    }

    #[test]
    fn test_positioned_dx_cumulative_offsets() {
        let mut engine = make_engine();
        // dx offsets are cumulative: dx="5 10 15" means:
        //   char 0: +5, char 1: +5+10=+15, char 2: +5+10+15=+30
        let base = layout_svg_text(
            &mut engine,
            "ABC",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let shifted = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0],
            &[50.0],
            &[5.0, 10.0, 15.0],
            &[],
        );
        assert_eq!(base.glyphs.len(), shifted.glyphs.len());
        if !base.glyphs.is_empty() {
            // First char shifted by 5.
            let dx0 = shifted.glyphs[0].x - base.glyphs[0].x;
            assert!(
                (dx0 - 5.0).abs() < 0.01,
                "char 0 dx should be ~5, got {}",
                dx0
            );
            // Second char shifted by 5+10=15 (cumulative).
            let dx1 = shifted.glyphs[1].x - base.glyphs[1].x;
            assert!(
                (dx1 - 15.0).abs() < 0.01,
                "char 1 dx should be ~15, got {}",
                dx1
            );
            // Third char shifted by 5+10+15=30 (cumulative).
            let dx2 = shifted.glyphs[2].x - base.glyphs[2].x;
            assert!(
                (dx2 - 30.0).abs() < 0.01,
                "char 2 dx should be ~30, got {}",
                dx2
            );
        }
    }

    #[test]
    fn test_positioned_dy_cumulative_offsets() {
        let mut engine = make_engine();
        let base = layout_svg_text(
            &mut engine,
            "AB",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        let shifted = layout_svg_text_positioned(
            &mut engine,
            "AB",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0],
            &[50.0],
            &[],
            &[3.0, 7.0],
        );
        if !base.glyphs.is_empty() {
            let dy0 = shifted.glyphs[0].y - base.glyphs[0].y;
            assert!(
                (dy0 - 3.0).abs() < 0.01,
                "char 0 dy should be ~3, got {}",
                dy0
            );
            let dy1 = shifted.glyphs[1].y - base.glyphs[1].y;
            assert!(
                (dy1 - 10.0).abs() < 0.01,
                "char 1 dy should be ~10, got {}",
                dy1
            );
        }
    }

    #[test]
    fn test_positioned_partial_x_list_reflows() {
        let mut engine = make_engine();
        // x_list shorter than text: first 2 chars positioned, third reflows.
        // <text x="10 100">ABC</text>
        let layout = layout_svg_text_positioned(
            &mut engine,
            "ABC",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0, 100.0],
            &[50.0],
            &[],
            &[],
        );
        assert_eq!(layout.glyphs.len(), 3);
        assert!((layout.glyphs[0].x - 10.0).abs() < 0.01);
        assert!((layout.glyphs[1].x - 100.0).abs() < 0.01);
        // Third char should be after second char's advance.
        assert!(
            layout.glyphs[2].x > layout.glyphs[1].x,
            "Third char should be after second: {} vs {}",
            layout.glyphs[2].x,
            layout.glyphs[1].x
        );
    }

    #[test]
    fn test_positioned_empty_text() {
        let mut engine = make_engine();
        let layout = layout_svg_text_positioned(
            &mut engine,
            "",
            0.0,
            0.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0, 20.0],
            &[30.0],
            &[5.0],
            &[],
        );
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn test_positioned_combined_x_and_dx() {
        let mut engine = make_engine();
        // x positions set each char absolutely, then dx adds per-character offsets.
        // When an absolute x is given, it resets the cursor; dx[i] shifts from that.
        let layout = layout_svg_text_positioned(
            &mut engine,
            "AB",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            &[10.0, 50.0],
            &[50.0],
            &[2.0, 3.0],
            &[],
        );
        assert_eq!(layout.glyphs.len(), 2);
        // char 0: x=10, then dx[0]=2 is added => 12
        assert!(
            (layout.glyphs[0].x - 12.0).abs() < 0.01,
            "A should be at x=12, got {}",
            layout.glyphs[0].x
        );
        // char 1: x=50 (absolute override), then dx[1]=3 is added => 53
        assert!(
            (layout.glyphs[1].x - 53.0).abs() < 0.01,
            "B should be at x=53, got {}",
            layout.glyphs[1].x
        );
    }
}
