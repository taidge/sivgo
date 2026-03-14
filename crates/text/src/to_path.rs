//! Text-to-path conversion.
//!
//! Converts shaped text into `BezPath` outlines by extracting glyph contours
//! from the underlying font via `swash`.

use crate::engine::{FontStyleKind, TextEngine};
use crate::layout::{layout_svg_text, layout_svg_text_positioned, TextAnchor, TextLayout};
use sivgo_math::{BezPath, Point};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a text string to a `BezPath` containing the glyph outlines.
///
/// Each character is resolved to its font glyph, scaled to the requested size,
/// and translated to the correct position according to SVG text layout rules.
///
/// Returns `None` if the text is empty or no glyphs could be outlined.
#[allow(clippy::too_many_arguments)]
pub fn text_to_path(
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
) -> Option<BezPath> {
    if text.is_empty() {
        return None;
    }

    let layout = layout_svg_text(
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

    text_layout_to_path(engine, &layout)
}

/// Convert a text string to a `BezPath` using per-character SVG positioning arrays.
///
/// This is the positioned variant of [`text_to_path`] that supports SVG
/// `<text x="10 20 30">` and `<tspan dx="5 10">` style per-character positioning.
///
/// See [`layout_svg_text_positioned`] for details on the positioning parameters.
///
/// Returns `None` if the text is empty or no glyphs could be outlined.
#[allow(clippy::too_many_arguments)]
pub fn text_to_path_positioned(
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
) -> Option<BezPath> {
    if text.is_empty() {
        return None;
    }

    let layout = layout_svg_text_positioned(
        engine,
        text,
        base_x,
        base_y,
        font_family,
        font_size,
        font_weight,
        font_style,
        anchor,
        letter_spacing,
        word_spacing,
        x_list,
        y_list,
        dx_list,
        dy_list,
    );

    text_layout_to_path(engine, &layout)
}

/// Convert an already-computed [`TextLayout`] to a `BezPath`.
///
/// This is useful when you have already laid out text and want to convert
/// the result to outlines without re-shaping.
pub fn text_layout_to_path(engine: &mut TextEngine, layout: &TextLayout) -> Option<BezPath> {
    if layout.glyphs.is_empty() {
        return None;
    }

    let mut combined = BezPath::new();
    let mut any_outline = false;

    for pg in &layout.glyphs {
        if let Some(glyph_path) = outline_glyph(
            engine,
            pg.font_id,
            pg.glyph_id,
            pg.font_size,
            pg.font_weight,
            pg.x,
            pg.y,
        ) {
            combined.extend(&glyph_path);
            any_outline = true;
        }
    }

    if any_outline {
        Some(combined)
    } else {
        // Fallback: return the bounding box as a rectangular path.
        let b = &layout.bounds;
        if b.is_empty() {
            return None;
        }
        let mut path = BezPath::new();
        path.move_to(Point::new(b.x0, b.y0));
        path.line_to(Point::new(b.x1, b.y0));
        path.line_to(Point::new(b.x1, b.y1));
        path.line_to(Point::new(b.x0, b.y1));
        path.close();
        Some(path)
    }
}

// ---------------------------------------------------------------------------
// Internal: glyph outline extraction via swash
// ---------------------------------------------------------------------------

/// Extract the outline of a single glyph, scaled and positioned.
fn outline_glyph(
    engine: &mut TextEngine,
    font_id: cosmic_text::fontdb::ID,
    glyph_id: u16,
    font_size: f32,
    font_weight: cosmic_text::Weight,
    offset_x: f64,
    offset_y: f64,
) -> Option<BezPath> {
    let fs = engine.font_system_mut();
    let font = fs.get_font(font_id, font_weight)?;

    let swash_font = font.as_swash();

    // Build a scaler at the requested size.
    let mut context = swash::scale::ScaleContext::new();
    let mut scaler = context
        .builder(swash_font)
        .hint(false)
        .size(font_size)
        .build();

    let outline = scaler.scale_outline(glyph_id)?;

    // Convert swash outline to BezPath.
    let mut path = BezPath::new();

    use swash::zeno::PathData;
    for cmd in outline.path().commands() {
        use swash::zeno::Command;
        match cmd {
            Command::MoveTo(p) => {
                path.move_to(Point::new(p.x as f64 + offset_x, p.y as f64 + offset_y));
            }
            Command::LineTo(p) => {
                path.line_to(Point::new(p.x as f64 + offset_x, p.y as f64 + offset_y));
            }
            Command::QuadTo(cp, end) => {
                path.quad_to(
                    Point::new(cp.x as f64 + offset_x, cp.y as f64 + offset_y),
                    Point::new(end.x as f64 + offset_x, end.y as f64 + offset_y),
                );
            }
            Command::CurveTo(cp1, cp2, end) => {
                path.curve_to(
                    Point::new(cp1.x as f64 + offset_x, cp1.y as f64 + offset_y),
                    Point::new(cp2.x as f64 + offset_x, cp2.y as f64 + offset_y),
                    Point::new(end.x as f64 + offset_x, end.y as f64 + offset_y),
                );
            }
            Command::Close => {
                path.close();
            }
        }
    }

    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_path_empty() {
        let mut engine = TextEngine::new();
        let result = text_to_path(
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
        );
        assert!(result.is_none(), "Empty text should return None");
    }

    #[test]
    fn test_text_to_path_produces_path() {
        let mut engine = TextEngine::new();
        let result = text_to_path(
            &mut engine,
            "A",
            0.0,
            20.0,
            "sans-serif",
            24.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        assert!(result.is_some(), "Non-empty text should produce a path");
        let path = result.unwrap();
        assert!(!path.is_empty(), "Path should not be empty");
    }

    #[test]
    fn test_text_to_path_hello() {
        let mut engine = TextEngine::new();
        let result = text_to_path(
            &mut engine,
            "Hello",
            10.0,
            50.0,
            "sans-serif",
            20.0,
            400,
            FontStyleKind::Normal,
            TextAnchor::Start,
            0.0,
            0.0,
        );
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(
            path.element_count() > 4,
            "Hello should produce more than a simple rectangle"
        );
    }
}
