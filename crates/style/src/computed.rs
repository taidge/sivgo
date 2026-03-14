//! ComputedStyle: all resolved style values for a single SVG element.

use sivgo_dom::{
    BlendMode, Color, Display, FillRule, FontStyle, FontWeight, LineCap, LineJoin, Paint,
    TextAnchor, Visibility,
};
use sivgo_math::Transform2D;

/// All resolved style values for a single element.
///
/// Every field holds its final computed value, with inheritance and cascade
/// already resolved. Fields use SVG-spec default values.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    // -- Color property (inherited) --
    /// The CSS `color` property. Used to resolve `currentColor` keyword.
    /// Default: `Color::BLACK`.
    pub color: Color,

    // -- Painting (inherited) --
    /// The fill paint. Default: `Paint::Color(Color::BLACK)`.
    pub fill: Paint,
    /// Fill opacity. Default: `1.0`.
    pub fill_opacity: f64,
    /// Fill rule. Default: `NonZero`.
    pub fill_rule: FillRule,

    /// The stroke paint. Default: `Paint::None`.
    pub stroke: Paint,
    /// Stroke opacity. Default: `1.0`.
    pub stroke_opacity: f64,
    /// Stroke width in user-space pixels. Default: `1.0`.
    pub stroke_width: f64,
    /// Stroke line cap. Default: `Butt`.
    pub stroke_linecap: LineCap,
    /// Stroke line join. Default: `Miter`.
    pub stroke_linejoin: LineJoin,
    /// Stroke miter limit. Default: `4.0`.
    pub stroke_miterlimit: f64,
    /// Stroke dash array. Default: `None` (solid line).
    pub stroke_dasharray: Option<Vec<f64>>,
    /// Stroke dash offset. Default: `0.0`.
    pub stroke_dashoffset: f64,

    // -- Visibility --
    /// Element opacity. Default: `1.0`. **NOT inherited.**
    pub opacity: f64,
    /// Display mode. Default: `Inline`. **NOT inherited.**
    pub display: Display,
    /// Visibility. Default: `Visible`. Inherited.
    pub visibility: Visibility,

    // -- Transform --
    /// The element transform. Default: identity. **NOT inherited.**
    pub transform: Transform2D,

    // -- Clipping & masking (NOT inherited) --
    /// `clip-path` URL reference.
    pub clip_path: Option<String>,
    /// Clip rule. Default: `NonZero`. Inherited.
    pub clip_rule: FillRule,
    /// `mask` URL reference.
    pub mask: Option<String>,
    /// `filter` URL reference.
    pub filter: Option<String>,

    // -- Markers (NOT inherited) --
    pub marker_start: Option<String>,
    pub marker_mid: Option<String>,
    pub marker_end: Option<String>,

    // -- Text (inherited) --
    /// Font family. Default: `"sans-serif"`.
    pub font_family: String,
    /// Font size in px. Default: `16.0`.
    pub font_size: f64,
    /// Font weight. Default: `Normal`.
    pub font_weight: FontWeight,
    /// Font style. Default: `Normal`.
    pub font_style: FontStyle,
    /// Text anchor. Default: `Start`.
    pub text_anchor: TextAnchor,
    /// Letter spacing. Default: `0.0`.
    pub letter_spacing: f64,
    /// Word spacing. Default: `0.0`.
    pub word_spacing: f64,

    // -- Blend mode (NOT inherited) --
    pub mix_blend_mode: BlendMode,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            // Color property (inherited)
            color: Color::BLACK,

            // Painting (inherited)
            fill: Paint::Color(Color::BLACK),
            fill_opacity: 1.0,
            fill_rule: FillRule::NonZero,
            stroke: Paint::None,
            stroke_opacity: 1.0,
            stroke_width: 1.0,
            stroke_linecap: LineCap::Butt,
            stroke_linejoin: LineJoin::Miter,
            stroke_miterlimit: 4.0,
            stroke_dasharray: None,
            stroke_dashoffset: 0.0,

            // Visibility
            opacity: 1.0,
            display: Display::Inline,
            visibility: Visibility::Visible,

            // Transform
            transform: Transform2D::IDENTITY,

            // Clipping & masking
            clip_path: None,
            clip_rule: FillRule::NonZero,
            mask: None,
            filter: None,

            // Markers
            marker_start: None,
            marker_mid: None,
            marker_end: None,

            // Text
            font_family: "sans-serif".to_string(),
            font_size: 16.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            text_anchor: TextAnchor::Start,
            letter_spacing: 0.0,
            word_spacing: 0.0,

            // Blend
            mix_blend_mode: BlendMode::Normal,
        }
    }
}

impl ComputedStyle {
    /// Create a new style with all properties set to their initial (default) values.
    pub fn initial() -> Self {
        Self::default()
    }

    /// Create a child style by inheriting inherited properties from `parent`
    /// and resetting non-inherited properties to their defaults.
    pub fn inherit_from(parent: &ComputedStyle) -> Self {
        let defaults = ComputedStyle::default();
        ComputedStyle {
            // Inherited color property
            color: parent.color,

            // Inherited painting properties
            fill: parent.fill.clone(),
            fill_opacity: parent.fill_opacity,
            fill_rule: parent.fill_rule,
            stroke: parent.stroke.clone(),
            stroke_opacity: parent.stroke_opacity,
            stroke_width: parent.stroke_width,
            stroke_linecap: parent.stroke_linecap,
            stroke_linejoin: parent.stroke_linejoin,
            stroke_miterlimit: parent.stroke_miterlimit,
            stroke_dasharray: parent.stroke_dasharray.clone(),
            stroke_dashoffset: parent.stroke_dashoffset,

            // NOT inherited -- reset to defaults
            opacity: defaults.opacity,
            // NOT inherited
            display: defaults.display,
            // Inherited
            visibility: parent.visibility,

            // NOT inherited
            transform: defaults.transform,

            // NOT inherited
            clip_path: None,
            // Inherited
            clip_rule: parent.clip_rule,
            // NOT inherited
            mask: None,
            filter: None,

            // NOT inherited
            marker_start: None,
            marker_mid: None,
            marker_end: None,

            // Inherited text properties
            font_family: parent.font_family.clone(),
            font_size: parent.font_size,
            font_weight: parent.font_weight,
            font_style: parent.font_style,
            text_anchor: parent.text_anchor,
            letter_spacing: parent.letter_spacing,
            word_spacing: parent.word_spacing,

            // NOT inherited
            mix_blend_mode: defaults.mix_blend_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_correct_values() {
        let s = ComputedStyle::default();
        assert_eq!(s.fill, Paint::Color(Color::BLACK));
        assert_eq!(s.fill_opacity, 1.0);
        assert_eq!(s.fill_rule, FillRule::NonZero);
        assert_eq!(s.stroke, Paint::None);
        assert_eq!(s.stroke_width, 1.0);
        assert_eq!(s.opacity, 1.0);
        assert_eq!(s.display, Display::Inline);
        assert_eq!(s.visibility, Visibility::Visible);
        assert_eq!(s.font_family, "sans-serif");
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.stroke_miterlimit, 4.0);
        assert!(s.stroke_dasharray.is_none());
        assert!(s.clip_path.is_none());
        assert!(s.marker_start.is_none());
    }

    #[test]
    fn inherit_copies_inherited_props() {
        let mut parent = ComputedStyle::default();
        parent.fill = Paint::Color(Color::RED);
        parent.stroke = Paint::Color(Color::BLUE);
        parent.font_size = 24.0;
        parent.visibility = Visibility::Hidden;
        parent.clip_rule = FillRule::EvenOdd;
        parent.stroke_dasharray = Some(vec![5.0, 3.0]);

        let child = ComputedStyle::inherit_from(&parent);

        // Inherited properties come from parent
        assert_eq!(child.fill, Paint::Color(Color::RED));
        assert_eq!(child.stroke, Paint::Color(Color::BLUE));
        assert_eq!(child.font_size, 24.0);
        assert_eq!(child.visibility, Visibility::Hidden);
        assert_eq!(child.clip_rule, FillRule::EvenOdd);
        assert_eq!(child.stroke_dasharray, Some(vec![5.0, 3.0]));
    }

    #[test]
    fn inherit_resets_non_inherited_props() {
        let mut parent = ComputedStyle::default();
        parent.opacity = 0.5;
        parent.display = Display::None;
        parent.mix_blend_mode = BlendMode::Multiply;
        parent.clip_path = Some("url(#clip)".to_string());
        parent.filter = Some("url(#blur)".to_string());
        parent.marker_start = Some("url(#arrow)".to_string());

        let child = ComputedStyle::inherit_from(&parent);

        // Non-inherited properties reset to defaults
        assert_eq!(child.opacity, 1.0);
        assert_eq!(child.display, Display::Inline);
        assert_eq!(child.mix_blend_mode, BlendMode::Normal);
        assert!(child.clip_path.is_none());
        assert!(child.filter.is_none());
        assert!(child.marker_start.is_none());
        assert_eq!(child.transform, Transform2D::IDENTITY);
    }
}
