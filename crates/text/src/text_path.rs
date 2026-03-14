//! Text-on-path layout (task 6.8).
//!
//! Positions glyphs along an arbitrary `BezPath`, rotating each glyph to
//! follow the path tangent. Implements the SVG `<textPath>` layout model.

use crate::engine::{FontStyleKind, TextEngine};
use crate::layout::PositionedGlyph;
use crate::metrics::get_font_metrics;
#[cfg(test)]
use sivgo_math::Point;
use sivgo_math::{BezPath, Rect};

// ---------------------------------------------------------------------------
// TextPathSpacing
// ---------------------------------------------------------------------------

/// SVG `spacing` attribute for `<textPath>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextPathSpacing {
    /// Adjust spacing between glyphs so that they are distributed evenly.
    #[default]
    Auto,
    /// Use exact glyph advances -- glyphs may overlap on tight curves.
    Exact,
}

// ---------------------------------------------------------------------------
// StartOffset
// ---------------------------------------------------------------------------

/// The `startOffset` attribute on `<textPath>`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartOffset {
    /// An absolute length (user units).
    Length(f64),
    /// A percentage of the total path length.
    Percentage(f64),
}

impl Default for StartOffset {
    fn default() -> Self {
        StartOffset::Length(0.0)
    }
}

// ---------------------------------------------------------------------------
// PathGlyph
// ---------------------------------------------------------------------------

/// A glyph positioned on a path.
///
/// Extends `PositionedGlyph` with a rotation angle so the glyph can be
/// rendered aligned to the path tangent.
#[derive(Debug, Clone)]
pub struct PathGlyph {
    /// The glyph data (x/y give the anchor point on the path).
    pub glyph: PositionedGlyph,
    /// Rotation angle in **radians** (the angle of the path tangent at the
    /// glyph's placement point). Apply as a rotation about `(glyph.x, glyph.y)`.
    pub angle: f64,
}

// ---------------------------------------------------------------------------
// TextPathLayout
// ---------------------------------------------------------------------------

/// The result of laying out text along a path.
#[derive(Debug, Clone)]
pub struct TextPathLayout {
    /// Glyphs positioned along the path.
    pub glyphs: Vec<PathGlyph>,
    /// Bounding box (axis-aligned, conservative -- does not account for
    /// rotation of individual glyphs).
    pub bounds: Rect,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lay out text along a `BezPath`.
///
/// Each glyph's midpoint (half its advance) is placed at the corresponding
/// arc-length distance along the path, and the glyph is rotated to match the
/// tangent at that point.
///
/// # Arguments
/// * `engine`         -- Text shaping engine.
/// * `text`           -- The string to lay out.
/// * `path`           -- The path to lay along.
/// * `font_family`    -- CSS-style family name.
/// * `font_size`      -- Font size in user units.
/// * `font_weight`    -- CSS numeric weight.
/// * `font_style`     -- Normal / Italic / Oblique.
/// * `letter_spacing` -- Extra space between glyphs.
/// * `word_spacing`   -- Extra space added before whitespace glyphs.
/// * `start_offset`   -- Where to begin placing text along the path.
/// * `spacing`        -- `Auto` or `Exact` spacing mode.
#[allow(clippy::too_many_arguments)]
pub fn layout_text_on_path(
    engine: &mut TextEngine,
    text: &str,
    path: &BezPath,
    font_family: &str,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    letter_spacing: f64,
    word_spacing: f64,
    start_offset: StartOffset,
    spacing: TextPathSpacing,
) -> TextPathLayout {
    if text.is_empty() || path.is_empty() {
        return TextPathLayout {
            glyphs: Vec::new(),
            bounds: Rect::ZERO,
        };
    }

    let accuracy = 0.5; // arc-length accuracy
    let total_path_len = path.arc_length(accuracy);
    if total_path_len <= 0.0 {
        return TextPathLayout {
            glyphs: Vec::new(),
            bounds: Rect::ZERO,
        };
    }

    let metrics = get_font_metrics(engine, font_family, font_size, font_weight, font_style);

    // Resolve startOffset to an absolute distance.
    let offset = match start_offset {
        StartOffset::Length(l) => l,
        StartOffset::Percentage(pct) => pct / 100.0 * total_path_len,
    };

    // Shape the text.
    let shaped = engine.shape_text(text, font_family, font_size, font_weight, font_style);
    let chars: Vec<char> = text.chars().collect();
    let shaped_glyphs = shaped.glyphs();

    // Compute per-glyph advances with spacing adjustments.
    let mut advances: Vec<f64> = Vec::with_capacity(shaped_glyphs.len());
    for (i, sg) in shaped_glyphs.iter().enumerate() {
        let ch = if i < chars.len() { chars[i] } else { ' ' };
        let mut adv = sg.w;
        // Word spacing.
        if i > 0 && ch.is_whitespace() {
            adv += word_spacing;
        }
        // Letter spacing (not after the last glyph).
        if i < shaped_glyphs.len() - 1 {
            adv += letter_spacing;
        }
        advances.push(adv);
    }

    let total_text_width: f64 = advances.iter().sum();

    // In Auto spacing mode, if the text is shorter than the path we add
    // uniform extra spacing. (We do *not* shrink below exact advances.)
    let extra_per_gap = if spacing == TextPathSpacing::Auto
        && shaped_glyphs.len() > 1
        && total_text_width < total_path_len - offset
    {
        let available = total_path_len - offset - total_text_width;
        let gaps = (shaped_glyphs.len() - 1) as f64;
        (available / gaps).max(0.0)
    } else {
        0.0
    };

    // Place glyphs.
    let mut cursor = offset; // arc-length cursor
    let mut result: Vec<PathGlyph> = Vec::with_capacity(shaped_glyphs.len());

    for (i, sg) in shaped_glyphs.iter().enumerate() {
        let ch = if i < chars.len() {
            chars[i]
        } else {
            '\u{FFFD}'
        };
        let adv = advances[i];

        // Place glyph midpoint at cursor + half-advance.
        let mid_distance = cursor + adv / 2.0;

        // Skip glyphs that fall outside the path.
        if mid_distance < 0.0 || mid_distance > total_path_len {
            cursor += adv + extra_per_gap;
            continue;
        }

        // Compute position and tangent at mid_distance.
        let param = match path.param_at_arclen(mid_distance, accuracy) {
            Some(t) => t,
            None => {
                cursor += adv + extra_per_gap;
                continue;
            }
        };

        let pos = match path.eval(param) {
            Some(p) => p,
            None => {
                cursor += adv + extra_per_gap;
                continue;
            }
        };

        let angle = match path.tangent_at(param) {
            Some((dx, dy)) => dy.atan2(dx),
            None => 0.0,
        };

        // The glyph x is placed so that the midpoint is at `pos`.
        // For the renderer the convention is: (x, y) is the glyph origin
        // (left edge, baseline), and the glyph is rotated by `angle` about
        // that point. The offset from mid to origin is -adv/2 in the
        // tangent direction, plus the baseline offset perpendicular.
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Tangent-parallel shift (glyph origin is half-advance before mid).
        let half_adv = adv / 2.0;
        // Perpendicular shift for baseline: move glyph *up* by ascent
        // (perpendicular = rotate tangent 90-deg CCW in screen coords, but
        // SVG y-axis points down so "up" is -perpendicular).
        let baseline_shift = metrics.ascent;

        let gx = pos.x - half_adv * cos_a + baseline_shift * sin_a;
        let gy = pos.y - half_adv * sin_a - baseline_shift * cos_a;

        result.push(PathGlyph {
            glyph: PositionedGlyph {
                x: gx,
                y: gy,
                advance: sg.w,
                character: ch,
                glyph_id: sg.glyph_id,
                font_id: sg.font_id,
                font_size: sg.font_size,
                font_weight: sg.font_weight,
            },
            angle,
        });

        cursor += adv + extra_per_gap;
    }

    // Compute bounding box (conservative: ignores rotation).
    let bounds = if result.is_empty() {
        Rect::ZERO
    } else {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for pg in &result {
            let g = &pg.glyph;
            // Account for the rotated bounding box of the glyph as a rough
            // approximation: use the glyph rectangle corners rotated by angle.
            let w = g.advance;
            let h = metrics.ascent + metrics.descent;
            let cos_a = pg.angle.cos().abs();
            let sin_a = pg.angle.sin().abs();
            let rotated_w = w * cos_a + h * sin_a;
            let rotated_h = w * sin_a + h * cos_a;

            let cx = g.x + w / 2.0 * pg.angle.cos();
            let cy = g.y + w / 2.0 * pg.angle.sin();

            if cx - rotated_w / 2.0 < min_x {
                min_x = cx - rotated_w / 2.0;
            }
            if cx + rotated_w / 2.0 > max_x {
                max_x = cx + rotated_w / 2.0;
            }
            if cy - rotated_h / 2.0 < min_y {
                min_y = cy - rotated_h / 2.0;
            }
            if cy + rotated_h / 2.0 > max_y {
                max_y = cy + rotated_h / 2.0;
            }
        }

        Rect::new(min_x, min_y, max_x, max_y)
    };

    TextPathLayout {
        glyphs: result,
        bounds,
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

    /// Create a simple horizontal line path from (0,0) to (500,0).
    fn horizontal_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(500.0, 0.0));
        p
    }

    /// Create a 90-degree quarter-circle arc (approximated as a cubic).
    fn quarter_circle_path() -> BezPath {
        // Approximate a 90-deg arc from (100,0) going counter-clockwise
        // around origin with radius 100.
        BezPath::from_svg_path("M 100 0 C 100 55.228 55.228 100 0 100").unwrap()
    }

    #[test]
    fn test_empty_text() {
        let mut engine = make_engine();
        let layout = layout_text_on_path(
            &mut engine,
            "",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn test_empty_path() {
        let mut engine = make_engine();
        let layout = layout_text_on_path(
            &mut engine,
            "Hello",
            &BezPath::new(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn test_glyphs_along_horizontal_path() {
        let mut engine = make_engine();
        let layout = layout_text_on_path(
            &mut engine,
            "Hello",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        assert!(!layout.glyphs.is_empty(), "Should produce glyphs");

        // On a horizontal path the rotation should be approximately 0.
        for pg in &layout.glyphs {
            assert!(
                pg.angle.abs() < 0.01,
                "Angle on horizontal path should be ~0, got {}",
                pg.angle
            );
        }

        // Glyphs should be ordered left to right.
        for window in layout.glyphs.windows(2) {
            assert!(
                window[1].glyph.x > window[0].glyph.x,
                "Glyphs should advance left-to-right: {} vs {}",
                window[0].glyph.x,
                window[1].glyph.x
            );
        }
    }

    #[test]
    fn test_start_offset_length() {
        let mut engine = make_engine();
        let no_offset = layout_text_on_path(
            &mut engine,
            "AB",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::Length(0.0),
            TextPathSpacing::Exact,
        );
        let with_offset = layout_text_on_path(
            &mut engine,
            "AB",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::Length(100.0),
            TextPathSpacing::Exact,
        );

        if !no_offset.glyphs.is_empty() && !with_offset.glyphs.is_empty() {
            let shift = with_offset.glyphs[0].glyph.x - no_offset.glyphs[0].glyph.x;
            assert!(
                (shift - 100.0).abs() < 1.0,
                "100px offset should shift glyphs ~100px, got {}",
                shift
            );
        }
    }

    #[test]
    fn test_start_offset_percentage() {
        let mut engine = make_engine();
        let path = horizontal_path(); // length = 500
        let layout = layout_text_on_path(
            &mut engine,
            "A",
            &path,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::Percentage(50.0),
            TextPathSpacing::Exact,
        );
        if !layout.glyphs.is_empty() {
            // 50% of 500 = 250 -- the glyph midpoint should be near x=250.
            let gx = layout.glyphs[0].glyph.x;
            assert!(
                gx > 200.0 && gx < 300.0,
                "50% offset on 500px path: glyph x {} should be near 250",
                gx
            );
        }
    }

    #[test]
    fn test_curved_path_rotation() {
        let mut engine = make_engine();
        let path = quarter_circle_path();
        let layout = layout_text_on_path(
            &mut engine,
            "ABC",
            &path,
            "sans-serif",
            12.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        assert!(!layout.glyphs.is_empty(), "Should produce glyphs");

        // On a quarter circle the angle should increase (become more positive
        // in SVG coords -- tangent rotates from rightward to downward).
        if layout.glyphs.len() >= 2 {
            let first_angle = layout.glyphs[0].angle;
            let last_angle = layout.glyphs.last().unwrap().angle;
            assert!(
                last_angle > first_angle,
                "On a quarter circle angles should increase: {} vs {}",
                first_angle,
                last_angle
            );
        }
    }

    #[test]
    fn test_spacing_auto_distributes() {
        let mut engine = make_engine();
        let path = horizontal_path(); // 500px
        let exact = layout_text_on_path(
            &mut engine,
            "AB",
            &path,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        let auto = layout_text_on_path(
            &mut engine,
            "AB",
            &path,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Auto,
        );

        if exact.glyphs.len() == 2 && auto.glyphs.len() == 2 {
            let gap_exact = exact.glyphs[1].glyph.x - exact.glyphs[0].glyph.x;
            let gap_auto = auto.glyphs[1].glyph.x - auto.glyphs[0].glyph.x;
            assert!(
                gap_auto > gap_exact,
                "Auto spacing should spread glyphs wider: {} vs {}",
                gap_auto,
                gap_exact
            );
        }
    }

    #[test]
    fn test_glyph_beyond_path_excluded() {
        let mut engine = make_engine();
        // Very short path: only 10 px.
        let mut short_path = BezPath::new();
        short_path.move_to(Point::new(0.0, 0.0));
        short_path.line_to(Point::new(10.0, 0.0));

        let layout = layout_text_on_path(
            &mut engine,
            "Hello World this is a very long string",
            &short_path,
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );

        // Most glyphs should be dropped because they exceed the path length.
        let full_layout = layout_text_on_path(
            &mut engine,
            "Hello World this is a very long string",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );

        assert!(
            layout.glyphs.len() < full_layout.glyphs.len(),
            "Short path should clip glyphs: {} vs {}",
            layout.glyphs.len(),
            full_layout.glyphs.len()
        );
    }

    #[test]
    fn test_bounds_non_empty() {
        let mut engine = make_engine();
        let layout = layout_text_on_path(
            &mut engine,
            "Hello",
            &horizontal_path(),
            "sans-serif",
            16.0,
            400,
            FontStyleKind::Normal,
            0.0,
            0.0,
            StartOffset::default(),
            TextPathSpacing::Exact,
        );
        assert!(!layout.bounds.is_empty(), "Bounds should not be empty");
    }
}
