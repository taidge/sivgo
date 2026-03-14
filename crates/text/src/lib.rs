//! `sivgo-text` -- Text shaping, layout, and text-to-path conversion for Sivgo.
//!
//! This crate wraps [`cosmic-text`] to provide:
//!
//! - **Text shaping** (`engine` module): load system fonts, shape strings into
//!   positioned glyphs.
//! - **SVG text layout** (`layout` module): apply SVG `<text>` positioning rules
//!   (`text-anchor`, `x`/`y` attributes).
//! - **Multi-line layout** (`multiline` module): word-wrap and explicit newline
//!   handling with per-line `text-anchor` alignment.
//! - **Text-on-path layout** (`text_path` module): position glyphs along an
//!   arbitrary `BezPath` with tangent-aligned rotation (`<textPath>`).
//! - **Font metrics** (`metrics` module): query ascent, descent, x-height, etc.
//! - **Text-to-path** (`to_path` module): convert shaped text into `BezPath`
//!   glyph outlines via `swash`.
//! - **Vertical text** (`vertical` module): vertical writing modes
//!   (`writing-mode: vertical-rl`, `vertical-lr`) with `text-orientation` support.
//! - **Font variants** (`font_variant` module): CSS `font-variant-caps`,
//!   `font-variant-ligatures`, and `font-variant-numeric` mapped to OpenType features.
//! - **BiDi text** (`bidi` module): simplified Unicode Bidirectional Algorithm
//!   for mixed LTR/RTL text.

pub mod bidi;
pub mod engine;
pub mod flow_text;
pub mod font_variant;
pub mod layout;
pub mod metrics;
pub mod multiline;
pub mod svg_font;
pub mod text_path;
pub mod to_path;
pub mod vertical;

// Convenience re-exports at the crate root.
pub use bidi::{
    analyze_bidi, classify_bidi_char, detect_paragraph_direction, reorder_by_bidi, BidiCharType,
    BidiInfo, BidiRun, Direction, UnicodeBidi,
};
pub use engine::{FontStyleKind, GlyphPosition, ShapedText, TextEngine};
pub use flow_text::{flow_text, FlowFragment, FlowLayout, FlowParagraph, FlowRegion};
pub use font_variant::{
    FontVariantCaps, FontVariantLigatures, FontVariantNumeric, FontVariantSettings, OpenTypeFeature,
};
pub use layout::{
    layout_svg_text, layout_svg_text_positioned, measure_text_width, PositionedGlyph, TextAnchor,
    TextLayout,
};
pub use metrics::{get_font_metrics, FontMetrics};
pub use multiline::{layout_multiline, MultiLineLayout, PositionedLine};
pub use svg_font::{KernPair, SvgFont, SvgFontGlyph, SvgFontRegistry, SvgGlyph};
pub use text_path::{layout_text_on_path, PathGlyph, StartOffset, TextPathLayout, TextPathSpacing};
pub use to_path::{text_layout_to_path, text_to_path, text_to_path_positioned};
pub use vertical::{
    layout_vertical_text, vertical_layout_to_text_layout, TextOrientation, VerticalGlyph,
    VerticalTextLayout, WritingMode,
};
