//! SVG font rendering support (SVG 1.1 `<font>` elements).
//!
//! SVG fonts are an older, rarely-used feature where glyph outlines are
//! defined inline in the SVG using `<glyph>` elements. This module
//! provides the data model and a basic lookup mechanism.
//!
//! Note: SVG fonts are deprecated in SVG 2 in favor of WOFF/WOFF2.

use sivgo_math::BezPath;
use std::collections::HashMap;

/// An SVG font definition parsed from `<font>` / `<font-face>` / `<glyph>`.
#[derive(Debug, Clone)]
pub struct SvgFont {
    /// The font family name from `<font-face>`.
    pub family: String,
    /// Units-per-em (default 1000).
    pub units_per_em: f64,
    /// Ascent value from `<font-face>`.
    pub ascent: f64,
    /// Descent value from `<font-face>` (typically negative).
    pub descent: f64,
    /// Default horizontal advance (from `<font>` horiz-adv-x).
    pub default_horiz_adv_x: f64,
    /// Character-to-glyph mapping.
    glyphs: HashMap<char, SvgGlyph>,
    /// The missing glyph (fallback).
    missing_glyph: Option<SvgGlyph>,
    /// Kerning pairs.
    kern_pairs: Vec<KernPair>,
}

/// A single glyph in an SVG font.
#[derive(Debug, Clone)]
pub struct SvgGlyph {
    /// The Unicode character this glyph represents.
    pub unicode: Option<char>,
    /// Glyph name (e.g., "A", "space").
    pub glyph_name: Option<String>,
    /// The glyph outline as a path.
    pub path: BezPath,
    /// Horizontal advance width (overrides font default if present).
    pub horiz_adv_x: Option<f64>,
}

/// A horizontal kerning pair.
#[derive(Debug, Clone)]
pub struct KernPair {
    /// First glyph character.
    pub g1: char,
    /// Second glyph character.
    pub g2: char,
    /// Kerning adjustment (usually negative to tighten).
    pub k: f64,
}

/// A positioned glyph from SVG font layout.
#[derive(Debug, Clone)]
pub struct SvgFontGlyph {
    /// The character.
    pub ch: char,
    /// X position.
    pub x: f64,
    /// Y position.
    pub y: f64,
    /// The glyph path (in font units, needs scaling).
    pub path: BezPath,
    /// Advance width used.
    pub advance: f64,
}

impl SvgFont {
    /// Create a new SVG font with the given family name and metrics.
    pub fn new(family: &str, units_per_em: f64, ascent: f64, descent: f64) -> Self {
        Self {
            family: family.to_string(),
            units_per_em,
            ascent,
            descent,
            default_horiz_adv_x: units_per_em / 2.0,
            glyphs: HashMap::new(),
            missing_glyph: None,
            kern_pairs: Vec::new(),
        }
    }

    /// Add a glyph to the font.
    pub fn add_glyph(&mut self, glyph: SvgGlyph) {
        if let Some(ch) = glyph.unicode {
            self.glyphs.insert(ch, glyph);
        }
    }

    /// Set the missing glyph (fallback).
    pub fn set_missing_glyph(&mut self, glyph: SvgGlyph) {
        self.missing_glyph = Some(glyph);
    }

    /// Add a kerning pair.
    pub fn add_kern(&mut self, g1: char, g2: char, k: f64) {
        self.kern_pairs.push(KernPair { g1, g2, k });
    }

    /// Look up the glyph for a character.
    pub fn glyph(&self, ch: char) -> Option<&SvgGlyph> {
        self.glyphs.get(&ch).or(self.missing_glyph.as_ref())
    }

    /// Get the advance width for a character.
    pub fn advance_width(&self, ch: char) -> f64 {
        self.glyphs
            .get(&ch)
            .and_then(|g| g.horiz_adv_x)
            .unwrap_or(self.default_horiz_adv_x)
    }

    /// Get the kerning value between two characters.
    pub fn kern(&self, g1: char, g2: char) -> f64 {
        self.kern_pairs
            .iter()
            .find(|k| k.g1 == g1 && k.g2 == g2)
            .map(|k| k.k)
            .unwrap_or(0.0)
    }

    /// Layout a string using this SVG font.
    ///
    /// Returns positioned glyphs in font units. The caller must scale
    /// from font units to the desired font size:
    ///   scale = font_size / units_per_em
    pub fn layout_text(&self, text: &str, font_size: f64) -> Vec<SvgFontGlyph> {
        let scale = font_size / self.units_per_em;
        let mut result = Vec::new();
        let mut x = 0.0;
        let chars: Vec<char> = text.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            let advance = self.advance_width(ch);
            let path = self.glyph(ch).map(|g| g.path.clone()).unwrap_or_default();

            result.push(SvgFontGlyph {
                ch,
                x: x * scale,
                y: 0.0,
                path,
                advance: advance * scale,
            });

            x += advance;

            // Apply kerning
            if i + 1 < chars.len() {
                x += self.kern(ch, chars[i + 1]);
            }
        }

        result
    }

    /// Measure the total width of a string in user units at the given font size.
    pub fn measure_width(&self, text: &str, font_size: f64) -> f64 {
        let scale = font_size / self.units_per_em;
        let mut width = 0.0;
        let chars: Vec<char> = text.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            width += self.advance_width(ch);
            if i + 1 < chars.len() {
                width += self.kern(ch, chars[i + 1]);
            }
        }

        width * scale
    }

    /// Number of glyphs defined in this font.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

/// Registry of SVG fonts found in a document.
#[derive(Debug, Clone, Default)]
pub struct SvgFontRegistry {
    fonts: HashMap<String, SvgFont>,
}

impl SvgFontRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an SVG font.
    pub fn register(&mut self, font: SvgFont) {
        self.fonts.insert(font.family.clone(), font);
    }

    /// Look up a font by family name.
    pub fn get(&self, family: &str) -> Option<&SvgFont> {
        self.fonts.get(family)
    }

    /// List all registered font families.
    pub fn families(&self) -> Vec<&str> {
        self.fonts.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    fn make_test_font() -> SvgFont {
        let mut font = SvgFont::new("TestFont", 1000.0, 800.0, -200.0);
        font.default_horiz_adv_x = 500.0;

        // Add a simple "A" glyph
        let mut a_path = BezPath::new();
        a_path.move_to(Point::new(0.0, 0.0));
        a_path.line_to(Point::new(250.0, 800.0));
        a_path.line_to(Point::new(500.0, 0.0));
        a_path.close();

        font.add_glyph(SvgGlyph {
            unicode: Some('A'),
            glyph_name: Some("A".into()),
            path: a_path,
            horiz_adv_x: Some(600.0),
        });

        // Add a "B" glyph with different width
        font.add_glyph(SvgGlyph {
            unicode: Some('B'),
            glyph_name: Some("B".into()),
            path: BezPath::new(),
            horiz_adv_x: Some(550.0),
        });

        // Add kerning
        font.add_kern('A', 'B', -50.0);

        font
    }

    #[test]
    fn font_glyph_lookup() {
        let font = make_test_font();
        assert!(font.glyph('A').is_some());
        assert!(font.glyph('B').is_some());
        assert!(font.glyph('Z').is_none());
    }

    #[test]
    fn font_advance_width() {
        let font = make_test_font();
        assert!((font.advance_width('A') - 600.0).abs() < 1e-10);
        assert!((font.advance_width('B') - 550.0).abs() < 1e-10);
        // Unknown char uses default
        assert!((font.advance_width('Z') - 500.0).abs() < 1e-10);
    }

    #[test]
    fn font_kerning() {
        let font = make_test_font();
        assert!((font.kern('A', 'B') - (-50.0)).abs() < 1e-10);
        assert!((font.kern('B', 'A') - 0.0).abs() < 1e-10);
    }

    #[test]
    fn layout_text_positions() {
        let font = make_test_font();
        let glyphs = font.layout_text("AB", 10.0); // scale = 10/1000 = 0.01
        assert_eq!(glyphs.len(), 2);
        assert!((glyphs[0].x - 0.0).abs() < 1e-10);
        // Second glyph: x = (600 + (-50)) * 0.01 = 5.5
        assert!((glyphs[1].x - 5.5).abs() < 1e-6);
    }

    #[test]
    fn measure_width() {
        let font = make_test_font();
        // "AB" = 600 + (-50 kern) + 550 = 1100 font units
        // At 10px: 1100 * 0.01 = 11.0
        let width = font.measure_width("AB", 10.0);
        assert!((width - 11.0).abs() < 1e-6);
    }

    #[test]
    fn missing_glyph_fallback() {
        let mut font = make_test_font();
        let missing = SvgGlyph {
            unicode: None,
            glyph_name: Some("missing".into()),
            path: BezPath::new(),
            horiz_adv_x: Some(400.0),
        };
        font.set_missing_glyph(missing);
        // Unknown char now returns the missing glyph
        assert!(font.glyph('Z').is_some());
        assert_eq!(
            font.glyph('Z').unwrap().glyph_name.as_deref(),
            Some("missing")
        );
    }

    #[test]
    fn font_registry() {
        let mut registry = SvgFontRegistry::new();
        assert!(registry.is_empty());

        registry.register(make_test_font());
        assert_eq!(registry.len(), 1);
        assert!(registry.get("TestFont").is_some());
        assert!(registry.get("NonExistent").is_none());

        let families = registry.families();
        assert_eq!(families.len(), 1);
        assert!(families.contains(&"TestFont"));
    }

    #[test]
    fn glyph_count() {
        let font = make_test_font();
        assert_eq!(font.glyph_count(), 2);
    }
}
