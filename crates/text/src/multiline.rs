//! Multi-line text layout (task 6.5).
//!
//! Breaks text into lines that fit within a given width (or at explicit
//! newlines) and produces a [`MultiLineLayout`] containing per-line glyph
//! runs with correct `(x, y)` positioning and `text-anchor` alignment.

use crate::engine::{FontStyleKind, TextEngine};
use crate::layout::{PositionedGlyph, TextAnchor};
use crate::metrics::get_font_metrics;
use sivgo_math::Rect;

// ---------------------------------------------------------------------------
// PositionedLine
// ---------------------------------------------------------------------------

/// One line of glyphs produced by the multi-line layout engine.
#[derive(Debug, Clone)]
pub struct PositionedLine {
    /// Glyphs in this line, already positioned in user-space.
    pub glyphs: Vec<PositionedGlyph>,
    /// The baseline y-coordinate of this line.
    pub y: f64,
    /// The total advance width of the line (before anchor adjustment).
    pub width: f64,
}

// ---------------------------------------------------------------------------
// MultiLineLayout
// ---------------------------------------------------------------------------

/// The result of multi-line text layout.
#[derive(Debug, Clone)]
pub struct MultiLineLayout {
    /// Laid-out lines, each containing positioned glyphs.
    pub lines: Vec<PositionedLine>,
    /// Tight bounding box enclosing all lines.
    pub bounds: Rect,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lay out text into multiple lines.
///
/// Text is broken at:
/// 1. Explicit newline characters (`\n`).
/// 2. Word boundaries when the accumulated width exceeds `max_width`.
///    If `max_width` is `None` only explicit newlines cause breaks.
///
/// Each line is aligned according to `anchor` and stacked vertically starting
/// from `(x, y)` with the given `line_spacing_factor` (1.0 = normal single
/// spacing, 1.5 = 150 %, etc.).
///
/// # Arguments
/// * `engine`              -- The text shaping engine.
/// * `text`                -- Full input text (may contain `\n`).
/// * `x`                   -- X origin for the text block.
/// * `y`                   -- Y origin (baseline of the first line).
/// * `font_family`         -- CSS-style family name.
/// * `font_size`           -- Size in user-space units.
/// * `font_weight`         -- CSS numeric weight (400 = normal).
/// * `font_style`          -- Normal / Italic / Oblique.
/// * `anchor`              -- `text-anchor` applied to each line independently.
/// * `letter_spacing`      -- Extra space between adjacent glyphs.
/// * `word_spacing`        -- Extra space added before each whitespace glyph.
/// * `max_width`           -- Maximum line width for word-wrapping (`None` = no wrap).
/// * `line_spacing_factor` -- Multiplier on the font's line-height for line spacing.
#[allow(clippy::too_many_arguments)]
pub fn layout_multiline(
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
    max_width: Option<f64>,
    line_spacing_factor: f64,
) -> MultiLineLayout {
    if text.is_empty() {
        return MultiLineLayout {
            lines: Vec::new(),
            bounds: Rect::ZERO,
        };
    }

    let metrics = get_font_metrics(engine, font_family, font_size, font_weight, font_style);
    let line_height = metrics.line_height * line_spacing_factor;

    // Split on explicit newlines first, then word-wrap each paragraph.
    let paragraphs: Vec<&str> = text.split('\n').collect();

    let mut lines: Vec<PositionedLine> = Vec::new();
    let mut current_y = y;

    for paragraph in &paragraphs {
        let wrapped = wrap_paragraph(
            engine,
            paragraph,
            font_family,
            font_size,
            font_weight,
            font_style,
            letter_spacing,
            word_spacing,
            max_width,
        );

        for line_text in &wrapped {
            let positioned_line = shape_line(
                engine,
                line_text,
                x,
                current_y,
                font_family,
                font_size,
                font_weight,
                font_style,
                anchor,
                letter_spacing,
                word_spacing,
                &metrics,
            );
            lines.push(positioned_line);
            current_y += line_height;
        }
    }

    // Compute combined bounding box.
    let bounds = compute_bounds(&lines, &metrics);

    MultiLineLayout { lines, bounds }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Word-wrap a single paragraph (no embedded newlines) into line strings.
///
/// If `max_width` is `None`, returns the paragraph as a single line.
#[allow(clippy::too_many_arguments)]
fn wrap_paragraph(
    engine: &mut TextEngine,
    paragraph: &str,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    letter_spacing: f64,
    word_spacing: f64,
    max_width: Option<f64>,
) -> Vec<String> {
    let max_width = match max_width {
        Some(w) => w,
        None => return vec![paragraph.to_string()],
    };

    if paragraph.is_empty() {
        return vec![String::new()];
    }

    let words: Vec<&str> = paragraph.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }

    let mut result: Vec<String> = Vec::new();
    let mut current_line = String::new();

    for word in &words {
        if current_line.is_empty() {
            // First word on the line -- always accept it even if it exceeds
            // max_width (avoids infinite loops on very long words).
            current_line.push_str(word);
        } else {
            // Tentatively add a space + word.
            let trial = format!("{} {}", current_line, word);
            let trial_width = measure_line_width(
                engine,
                &trial,
                font_family,
                font_size,
                font_weight,
                font_style,
                letter_spacing,
                word_spacing,
            );

            if trial_width <= max_width {
                current_line = trial;
            } else {
                // Overflow: flush current line and start a new one.
                result.push(current_line);
                current_line = word.to_string();
            }
        }
    }

    // Flush the last line.
    if !current_line.is_empty() {
        result.push(current_line);
    }

    result
}

/// Measure text width using the engine (thin wrapper around shape_text).
fn measure_line_width(
    engine: &mut TextEngine,
    text: &str,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    letter_spacing: f64,
    word_spacing: f64,
) -> f64 {
    crate::layout::measure_text_width(
        engine,
        text,
        font_family,
        font_size,
        font_weight,
        font_style,
        letter_spacing,
        word_spacing,
    )
}

/// Shape a single line and produce a `PositionedLine`.
#[allow(clippy::too_many_arguments)]
fn shape_line(
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
    metrics: &crate::metrics::FontMetrics,
) -> PositionedLine {
    if text.is_empty() {
        return PositionedLine {
            glyphs: Vec::new(),
            y,
            width: 0.0,
        };
    }

    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);
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

        if i < shaped_glyphs.len() - 1 {
            extra_offset += letter_spacing;
        }
    }

    // Compute width.
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

    PositionedLine {
        glyphs: positioned,
        y,
        width,
    }
}

/// Compute the combined bounding box for all lines.
fn compute_bounds(lines: &[PositionedLine], metrics: &crate::metrics::FontMetrics) -> Rect {
    if lines.is_empty() {
        return Rect::ZERO;
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for line in lines {
        for g in &line.glyphs {
            if g.x < min_x {
                min_x = g.x;
            }
            if g.x + g.advance > max_x {
                max_x = g.x + g.advance;
            }
        }
        let line_top = line.y - metrics.ascent;
        let line_bottom = line.y + metrics.descent;
        if line_top < min_y {
            min_y = line_top;
        }
        if line_bottom > max_y {
            max_y = line_bottom;
        }
    }

    if min_x > max_x || min_y > max_y {
        Rect::ZERO
    } else {
        Rect::new(min_x, min_y, max_x, max_y)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> TextEngine {
        TextEngine::new()
    }

    #[test]
    fn test_empty_text() {
        let mut engine = make_engine();
        let layout = layout_multiline(
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
            None,
            1.0,
        );
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn test_single_line_no_wrap() {
        let mut engine = make_engine();
        let layout = layout_multiline(
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
            None,
            1.0,
        );
        assert_eq!(layout.lines.len(), 1);
        assert!(!layout.lines[0].glyphs.is_empty());
    }

    #[test]
    fn test_explicit_newlines() {
        let mut engine = make_engine();
        let layout = layout_multiline(
            &mut engine,
            "Line1\nLine2\nLine3",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        assert_eq!(layout.lines.len(), 3, "Should produce 3 lines");
        // Each line should have glyphs.
        for (i, line) in layout.lines.iter().enumerate() {
            assert!(!line.glyphs.is_empty(), "Line {} should have glyphs", i);
        }
    }

    #[test]
    fn test_lines_y_increases() {
        let mut engine = make_engine();
        let layout = layout_multiline(
            &mut engine,
            "A\nB\nC",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        assert_eq!(layout.lines.len(), 3);
        assert!(
            layout.lines[1].y > layout.lines[0].y,
            "Second line should be below first"
        );
        assert!(
            layout.lines[2].y > layout.lines[1].y,
            "Third line should be below second"
        );
    }

    #[test]
    fn test_word_wrap() {
        let mut engine = make_engine();
        // Use a narrow width to force wrapping.
        let layout = layout_multiline(
            &mut engine,
            "Hello World Test Wrap",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            Some(60.0), // very narrow
            1.0,
        );
        assert!(
            layout.lines.len() > 1,
            "With narrow max_width, text should wrap: got {} lines",
            layout.lines.len()
        );
    }

    #[test]
    fn test_anchor_middle_per_line() {
        let mut engine = make_engine();
        let start_layout = layout_multiline(
            &mut engine,
            "Hello\nWorld",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        let mid_layout = layout_multiline(
            &mut engine,
            "Hello\nWorld",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Middle,
            0.0,
            0.0,
            None,
            1.0,
        );

        // With Middle anchor each line's first glyph should be left of the
        // Start-anchored equivalent.
        for i in 0..2 {
            if !start_layout.lines[i].glyphs.is_empty() && !mid_layout.lines[i].glyphs.is_empty() {
                assert!(
                    mid_layout.lines[i].glyphs[0].x < start_layout.lines[i].glyphs[0].x,
                    "Line {} middle-anchor first glyph should be further left",
                    i
                );
            }
        }
    }

    #[test]
    fn test_anchor_end_per_line() {
        let mut engine = make_engine();
        let start_layout = layout_multiline(
            &mut engine,
            "Hello\nWorld",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        let end_layout = layout_multiline(
            &mut engine,
            "Hello\nWorld",
            100.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::End,
            0.0,
            0.0,
            None,
            1.0,
        );

        for i in 0..2 {
            if !start_layout.lines[i].glyphs.is_empty() && !end_layout.lines[i].glyphs.is_empty() {
                assert!(
                    end_layout.lines[i].glyphs[0].x < start_layout.lines[i].glyphs[0].x,
                    "Line {} end-anchor first glyph should be further left",
                    i
                );
            }
        }
    }

    #[test]
    fn test_bounds_enclose_all_lines() {
        let mut engine = make_engine();
        let layout = layout_multiline(
            &mut engine,
            "Line1\nLine2",
            10.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        let b = &layout.bounds;
        for line in &layout.lines {
            for g in &line.glyphs {
                assert!(g.x >= b.x0 - 0.01, "glyph x {} < bounds x0 {}", g.x, b.x0);
                assert!(
                    g.x + g.advance <= b.x1 + 0.01,
                    "glyph right {} > bounds x1 {}",
                    g.x + g.advance,
                    b.x1
                );
            }
        }
    }

    #[test]
    fn test_long_word_does_not_loop() {
        // Even when the word is wider than max_width it should not cause an
        // infinite loop -- it just gets placed on its own line.
        let mut engine = make_engine();
        let layout = layout_multiline(
            &mut engine,
            "Supercalifragilisticexpialidocious",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            Some(10.0), // absurdly narrow
            1.0,
        );
        assert_eq!(
            layout.lines.len(),
            1,
            "Single long word should produce one line"
        );
    }

    #[test]
    fn test_line_spacing_factor() {
        let mut engine = make_engine();
        let layout_1x = layout_multiline(
            &mut engine,
            "A\nB",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        let layout_2x = layout_multiline(
            &mut engine,
            "A\nB",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            2.0,
        );
        let gap_1x = layout_1x.lines[1].y - layout_1x.lines[0].y;
        let gap_2x = layout_2x.lines[1].y - layout_2x.lines[0].y;
        assert!(
            (gap_2x - 2.0 * gap_1x).abs() < 0.01,
            "2x spacing gap {} should be double 1x gap {}",
            gap_2x,
            gap_1x
        );
    }

    #[test]
    fn test_empty_line_between_newlines() {
        let mut engine = make_engine();
        let layout = layout_multiline(
            &mut engine,
            "A\n\nB",
            0.0,
            50.0,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
            None,
            1.0,
        );
        // Should produce 3 lines: "A", "", "B"
        assert_eq!(
            layout.lines.len(),
            3,
            "Should have 3 lines including empty middle line"
        );
        assert!(
            layout.lines[1].glyphs.is_empty(),
            "Middle line should be empty"
        );
    }
}
