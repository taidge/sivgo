//! Vertical text layout support.
//!
//! Implements CSS `writing-mode` for vertical text:
//! - `tb` / `tb-rl` / `vertical-rl`: top-to-bottom, lines progress right-to-left
//! - `tb-lr` / `vertical-lr`: top-to-bottom, lines progress left-to-right
//!
//! Also supports the `text-orientation` property which controls how individual
//! glyphs are oriented in vertical text.

use crate::engine::{FontStyleKind, TextEngine};
use crate::layout::{PositionedGlyph, TextAnchor, TextLayout};
use crate::metrics::get_font_metrics;
use sivgo_math::Rect;

// ---------------------------------------------------------------------------
// WritingMode
// ---------------------------------------------------------------------------

/// CSS `writing-mode` property values relevant to SVG text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    /// Horizontal left-to-right (default).
    #[default]
    HorizontalTb,
    /// Vertical, top-to-bottom, lines progress right-to-left (most CJK).
    VerticalRl,
    /// Vertical, top-to-bottom, lines progress left-to-right (Mongolian).
    VerticalLr,
}

impl WritingMode {
    /// Parse a CSS/SVG `writing-mode` string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "tb" | "tb-rl" | "vertical-rl" => WritingMode::VerticalRl,
            "tb-lr" | "vertical-lr" => WritingMode::VerticalLr,
            _ => WritingMode::HorizontalTb,
        }
    }

    /// Whether this writing mode is vertical.
    pub fn is_vertical(self) -> bool {
        matches!(self, WritingMode::VerticalRl | WritingMode::VerticalLr)
    }

    /// Whether lines progress right-to-left in this writing mode.
    pub fn is_rtl_line_progression(self) -> bool {
        matches!(self, WritingMode::VerticalRl)
    }
}

// ---------------------------------------------------------------------------
// TextOrientation
// ---------------------------------------------------------------------------

/// CSS `text-orientation` property values for vertical text.
///
/// Controls how individual glyphs are oriented in vertical writing modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextOrientation {
    /// Use the natural orientation of each character. Horizontal-only scripts
    /// are rotated 90 degrees clockwise, CJK and inherently vertical scripts
    /// remain upright.
    #[default]
    Mixed,
    /// All characters are displayed upright (no rotation).
    Upright,
    /// All characters are rotated 90 degrees clockwise (sideways).
    Sideways,
}

impl TextOrientation {
    /// Parse a CSS `text-orientation` string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "upright" => TextOrientation::Upright,
            "sideways" | "sideways-right" => TextOrientation::Sideways,
            _ => TextOrientation::Mixed,
        }
    }

    /// Determine the rotation angle (in degrees) for a glyph in vertical mode.
    ///
    /// Returns 90.0 for glyphs that should be rotated sideways, 0.0 for upright.
    pub fn glyph_rotation(self, ch: char) -> f64 {
        match self {
            TextOrientation::Upright => 0.0,
            TextOrientation::Sideways => 90.0,
            TextOrientation::Mixed => {
                if is_naturally_vertical(ch) {
                    0.0
                } else {
                    90.0
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VerticalGlyph
// ---------------------------------------------------------------------------

/// A glyph positioned for vertical text, with an additional rotation field.
#[derive(Debug, Clone)]
pub struct VerticalGlyph {
    /// The positioned glyph (x/y are in vertical coordinate space).
    pub glyph: PositionedGlyph,
    /// Rotation angle in degrees to apply to this glyph (0 or 90).
    pub rotation: f64,
}

// ---------------------------------------------------------------------------
// VerticalTextLayout
// ---------------------------------------------------------------------------

/// Result of laying out vertical text.
#[derive(Debug, Clone)]
pub struct VerticalTextLayout {
    /// Positioned glyphs with rotation information.
    pub glyphs: Vec<VerticalGlyph>,
    /// Bounding box of the laid-out text.
    pub bounds: Rect,
    /// The writing mode used.
    pub writing_mode: WritingMode,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lay out text in a vertical writing mode.
///
/// Glyphs are arranged top-to-bottom. Each glyph receives a rotation angle
/// based on the `text_orientation` setting.
///
/// - For `VerticalRl`, the inline direction is top-to-bottom and line
///   progression is right-to-left.
/// - For `VerticalLr`, the inline direction is top-to-bottom and line
///   progression is left-to-right.
///
/// The `line_index` parameter is used for multi-line text to offset lines
/// in the block direction.
#[allow(clippy::too_many_arguments)]
pub fn layout_vertical_text(
    engine: &mut TextEngine,
    text: &str,
    x: f64,
    y: f64,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    anchor: TextAnchor,
    writing_mode: WritingMode,
    text_orientation: TextOrientation,
    line_index: i32,
) -> VerticalTextLayout {
    if text.is_empty() || !writing_mode.is_vertical() {
        return VerticalTextLayout {
            glyphs: Vec::new(),
            bounds: Rect::ZERO,
            writing_mode,
        };
    }

    // Shape text horizontally to get advance widths and glyph IDs.
    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);
    let _metrics = get_font_metrics(engine, font_family, font_size, font_weight, font_style);

    let chars: Vec<char> = text.chars().collect();
    let shaped_glyphs = shaped.glyphs();

    // In vertical mode, glyphs are stacked top-to-bottom.
    // The advance in the block (vertical) direction is approximately the
    // font size for each glyph.
    let glyph_advance_y = font_size;

    // Compute line offset in the block-perpendicular (horizontal) direction.
    let line_spacing = font_size * 1.2;
    let line_offset_x = if writing_mode.is_rtl_line_progression() {
        -(line_index as f64) * line_spacing
    } else {
        line_index as f64 * line_spacing
    };

    let mut vertical_glyphs = Vec::with_capacity(shaped_glyphs.len());
    let mut cursor_y = y;

    for (i, sg) in shaped_glyphs.iter().enumerate() {
        let ch = if i < chars.len() {
            chars[i]
        } else {
            '\u{FFFD}'
        };
        let rotation = text_orientation.glyph_rotation(ch);

        // In vertical mode, x is the center of the column and y advances downward.
        // The glyph x is offset from the baseline center.
        let glyph_x = x + line_offset_x;
        let glyph_y = cursor_y;

        vertical_glyphs.push(VerticalGlyph {
            glyph: PositionedGlyph {
                x: glyph_x,
                y: glyph_y,
                advance: sg.w,
                character: ch,
                glyph_id: sg.glyph_id,
                font_id: sg.font_id,
                font_size: sg.font_size,
                font_weight: sg.font_weight,
            },
            rotation,
        });

        cursor_y += glyph_advance_y;
    }

    // Compute total height for anchor adjustment.
    let total_height = if vertical_glyphs.is_empty() {
        0.0
    } else {
        let last = &vertical_glyphs.last().unwrap().glyph;
        (last.y + glyph_advance_y) - vertical_glyphs[0].glyph.y
    };

    // Apply text-anchor in the inline (vertical) direction.
    let anchor_offset = match anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -total_height / 2.0,
        TextAnchor::End => -total_height,
    };

    if anchor_offset != 0.0 {
        for vg in &mut vertical_glyphs {
            vg.glyph.y += anchor_offset;
        }
    }

    // Compute bounding box.
    let bounds = if vertical_glyphs.is_empty() {
        Rect::ZERO
    } else {
        let half_width = font_size / 2.0;
        let min_x = vertical_glyphs
            .iter()
            .map(|vg| vg.glyph.x - half_width)
            .fold(f64::INFINITY, f64::min);
        let max_x = vertical_glyphs
            .iter()
            .map(|vg| vg.glyph.x + half_width)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = vertical_glyphs
            .iter()
            .map(|vg| vg.glyph.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = vertical_glyphs
            .iter()
            .map(|vg| vg.glyph.y + glyph_advance_y)
            .fold(f64::NEG_INFINITY, f64::max);
        Rect::new(min_x, min_y, max_x, max_y)
    };

    VerticalTextLayout {
        glyphs: vertical_glyphs,
        bounds,
        writing_mode,
    }
}

/// Convert a `VerticalTextLayout` into a flat `TextLayout` for rendering.
///
/// The rotation information is lost in this conversion but the glyph positions
/// are preserved. Use this when you need to pass vertical text through
/// a rendering pipeline that expects `TextLayout`.
pub fn vertical_layout_to_text_layout(vertical: &VerticalTextLayout) -> TextLayout {
    let glyphs: Vec<PositionedGlyph> = vertical.glyphs.iter().map(|vg| vg.glyph.clone()).collect();

    TextLayout {
        glyphs,
        bounds: vertical.bounds,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check whether a character is naturally vertical (CJK ideographs, etc.).
///
/// Characters that are naturally vertical do not get rotated in `Mixed`
/// text-orientation mode. This is a simplified check covering the main
/// CJK Unified Ideographs blocks, CJK compatibility, Hangul, Katakana,
/// Hiragana, and some punctuation used in vertical text.
fn is_naturally_vertical(ch: char) -> bool {
    let cp = ch as u32;
    matches!(cp,
        // CJK Unified Ideographs
        0x4E00..=0x9FFF |
        // CJK Extension A
        0x3400..=0x4DBF |
        // CJK Extension B
        0x20000..=0x2A6DF |
        // CJK Compatibility Ideographs
        0xF900..=0xFAFF |
        // Hiragana
        0x3040..=0x309F |
        // Katakana
        0x30A0..=0x30FF |
        // Katakana Phonetic Extensions
        0x31F0..=0x31FF |
        // CJK Symbols and Punctuation
        0x3000..=0x303F |
        // Hangul Syllables
        0xAC00..=0xD7AF |
        // Hangul Jamo
        0x1100..=0x11FF |
        // CJK Radicals Supplement
        0x2E80..=0x2EFF |
        // Kangxi Radicals
        0x2F00..=0x2FDF |
        // Ideographic Description Characters
        0x2FF0..=0x2FFF |
        // Bopomofo
        0x3100..=0x312F |
        // Bopomofo Extended
        0x31A0..=0x31BF |
        // CJK Strokes
        0x31C0..=0x31EF |
        // Enclosed CJK Letters and Months
        0x3200..=0x32FF |
        // CJK Compatibility
        0x3300..=0x33FF |
        // Vertical forms
        0xFE10..=0xFE1F |
        // CJK Compatibility Forms
        0xFE30..=0xFE4F |
        // Halfwidth and Fullwidth Forms (fullwidth range)
        0xFF01..=0xFF60 |
        // Yi Syllables
        0xA000..=0xA48F |
        // Yi Radicals
        0xA490..=0xA4CF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // WritingMode
    // -----------------------------------------------------------------------

    #[test]
    fn test_writing_mode_parse() {
        assert_eq!(
            WritingMode::parse("horizontal-tb"),
            WritingMode::HorizontalTb
        );
        assert_eq!(WritingMode::parse("tb"), WritingMode::VerticalRl);
        assert_eq!(WritingMode::parse("tb-rl"), WritingMode::VerticalRl);
        assert_eq!(WritingMode::parse("vertical-rl"), WritingMode::VerticalRl);
        assert_eq!(WritingMode::parse("tb-lr"), WritingMode::VerticalLr);
        assert_eq!(WritingMode::parse("vertical-lr"), WritingMode::VerticalLr);
        assert_eq!(WritingMode::parse("unknown"), WritingMode::HorizontalTb);
    }

    #[test]
    fn test_writing_mode_is_vertical() {
        assert!(!WritingMode::HorizontalTb.is_vertical());
        assert!(WritingMode::VerticalRl.is_vertical());
        assert!(WritingMode::VerticalLr.is_vertical());
    }

    #[test]
    fn test_writing_mode_line_progression() {
        assert!(!WritingMode::HorizontalTb.is_rtl_line_progression());
        assert!(WritingMode::VerticalRl.is_rtl_line_progression());
        assert!(!WritingMode::VerticalLr.is_rtl_line_progression());
    }

    // -----------------------------------------------------------------------
    // TextOrientation
    // -----------------------------------------------------------------------

    #[test]
    fn test_text_orientation_parse() {
        assert_eq!(TextOrientation::parse("mixed"), TextOrientation::Mixed);
        assert_eq!(TextOrientation::parse("upright"), TextOrientation::Upright);
        assert_eq!(
            TextOrientation::parse("sideways"),
            TextOrientation::Sideways
        );
        assert_eq!(
            TextOrientation::parse("sideways-right"),
            TextOrientation::Sideways
        );
        assert_eq!(TextOrientation::parse("unknown"), TextOrientation::Mixed);
    }

    #[test]
    fn test_text_orientation_glyph_rotation_upright() {
        let orient = TextOrientation::Upright;
        assert_eq!(orient.glyph_rotation('A'), 0.0);
        assert_eq!(orient.glyph_rotation('\u{4E00}'), 0.0); // CJK
    }

    #[test]
    fn test_text_orientation_glyph_rotation_sideways() {
        let orient = TextOrientation::Sideways;
        assert_eq!(orient.glyph_rotation('A'), 90.0);
        assert_eq!(orient.glyph_rotation('\u{4E00}'), 90.0); // CJK
    }

    #[test]
    fn test_text_orientation_glyph_rotation_mixed() {
        let orient = TextOrientation::Mixed;
        // Latin script should be rotated.
        assert_eq!(orient.glyph_rotation('A'), 90.0);
        assert_eq!(orient.glyph_rotation('z'), 90.0);
        // CJK ideograph should remain upright.
        assert_eq!(orient.glyph_rotation('\u{4E00}'), 0.0);
        // Hiragana should remain upright.
        assert_eq!(orient.glyph_rotation('\u{3042}'), 0.0); // あ
    }

    // -----------------------------------------------------------------------
    // is_naturally_vertical
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_naturally_vertical() {
        // CJK ideographs are naturally vertical.
        assert!(is_naturally_vertical('\u{4E00}')); // 一
        assert!(is_naturally_vertical('\u{9FFF}'));
        // Hiragana
        assert!(is_naturally_vertical('\u{3042}')); // あ
                                                    // Katakana
        assert!(is_naturally_vertical('\u{30A2}')); // ア
                                                    // Latin is NOT naturally vertical.
        assert!(!is_naturally_vertical('A'));
        assert!(!is_naturally_vertical('z'));
        assert!(!is_naturally_vertical('0'));
    }

    // -----------------------------------------------------------------------
    // Vertical layout
    // -----------------------------------------------------------------------

    #[test]
    fn test_vertical_layout_empty() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "",
            100.0,
            100.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn test_vertical_layout_horizontal_mode_returns_empty() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "Hello",
            100.0,
            100.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::HorizontalTb,
            TextOrientation::Mixed,
            0,
        );
        assert!(
            layout.glyphs.is_empty(),
            "Horizontal mode should return empty from vertical layout"
        );
    }

    #[test]
    fn test_vertical_layout_produces_glyphs() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "Hello",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        assert_eq!(layout.glyphs.len(), 5, "Should have 5 glyphs for 'Hello'");
    }

    #[test]
    fn test_vertical_layout_glyphs_progress_downward() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "ABC",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        assert!(layout.glyphs.len() >= 3);
        // Each subsequent glyph should be below the previous one.
        for i in 1..layout.glyphs.len() {
            assert!(
                layout.glyphs[i].glyph.y > layout.glyphs[i - 1].glyph.y,
                "Glyph {} should be below glyph {}: {} vs {}",
                i,
                i - 1,
                layout.glyphs[i].glyph.y,
                layout.glyphs[i - 1].glyph.y
            );
        }
    }

    #[test]
    fn test_vertical_layout_latin_rotation_mixed() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "AB",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        // Latin characters in mixed mode should be rotated 90 degrees.
        for vg in &layout.glyphs {
            assert_eq!(
                vg.rotation, 90.0,
                "Latin glyphs should be rotated in mixed mode"
            );
        }
    }

    #[test]
    fn test_vertical_layout_upright_no_rotation() {
        let mut engine = TextEngine::new();
        let layout = layout_vertical_text(
            &mut engine,
            "AB",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Upright,
            0,
        );
        for vg in &layout.glyphs {
            assert_eq!(vg.rotation, 0.0, "Upright mode should have no rotation");
        }
    }

    #[test]
    fn test_vertical_layout_line_progression_rl() {
        let mut engine = TextEngine::new();
        let line0 = layout_vertical_text(
            &mut engine,
            "A",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        let line1 = layout_vertical_text(
            &mut engine,
            "B",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            1,
        );
        // In vertical-rl, line 1 should be to the LEFT of line 0.
        assert!(
            line1.glyphs[0].glyph.x < line0.glyphs[0].glyph.x,
            "Line 1 should be left of line 0 in vertical-rl: {} vs {}",
            line1.glyphs[0].glyph.x,
            line0.glyphs[0].glyph.x
        );
    }

    #[test]
    fn test_vertical_layout_line_progression_lr() {
        let mut engine = TextEngine::new();
        let line0 = layout_vertical_text(
            &mut engine,
            "A",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalLr,
            TextOrientation::Mixed,
            0,
        );
        let line1 = layout_vertical_text(
            &mut engine,
            "B",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalLr,
            TextOrientation::Mixed,
            1,
        );
        // In vertical-lr, line 1 should be to the RIGHT of line 0.
        assert!(
            line1.glyphs[0].glyph.x > line0.glyphs[0].glyph.x,
            "Line 1 should be right of line 0 in vertical-lr: {} vs {}",
            line1.glyphs[0].glyph.x,
            line0.glyphs[0].glyph.x
        );
    }

    #[test]
    fn test_vertical_layout_anchor_middle() {
        let mut engine = TextEngine::new();
        let start = layout_vertical_text(
            &mut engine,
            "ABC",
            100.0,
            100.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        let middle = layout_vertical_text(
            &mut engine,
            "ABC",
            100.0,
            100.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Middle,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        // Middle-anchored first glyph should be above the start-anchored first glyph.
        if !start.glyphs.is_empty() && !middle.glyphs.is_empty() {
            assert!(
                middle.glyphs[0].glyph.y < start.glyphs[0].glyph.y,
                "Middle anchor should shift glyphs upward"
            );
        }
    }

    #[test]
    fn test_vertical_layout_to_text_layout_conversion() {
        let mut engine = TextEngine::new();
        let vertical = layout_vertical_text(
            &mut engine,
            "AB",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            WritingMode::VerticalRl,
            TextOrientation::Mixed,
            0,
        );
        let text_layout = vertical_layout_to_text_layout(&vertical);
        assert_eq!(text_layout.glyphs.len(), vertical.glyphs.len());
        assert_eq!(text_layout.bounds, vertical.bounds);
    }
}
