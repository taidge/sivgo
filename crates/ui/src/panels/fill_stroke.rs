//! Fill & Stroke panel data model (11.4.3).
//!
//! Pure data types describing paint settings for fill and stroke.

/// The type of paint applied to a fill or stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintType {
    /// No paint (transparent).
    None,
    /// A single flat color.
    FlatColor,
    /// A linear gradient.
    LinearGradient,
    /// A radial gradient.
    RadialGradient,
    /// A tiled pattern.
    Pattern,
    /// A mesh gradient (SVG 2).
    MeshGradient,
    /// A swatch / palette reference.
    Swatch,
}

impl PaintType {
    /// Human-readable label for display.
    pub fn label(&self) -> &'static str {
        match self {
            PaintType::None => "None",
            PaintType::FlatColor => "Flat Color",
            PaintType::LinearGradient => "Linear Gradient",
            PaintType::RadialGradient => "Radial Gradient",
            PaintType::Pattern => "Pattern",
            PaintType::MeshGradient => "Mesh Gradient",
            PaintType::Swatch => "Swatch",
        }
    }

    /// All paint types in display order.
    pub fn all() -> &'static [PaintType] {
        &[
            PaintType::None,
            PaintType::FlatColor,
            PaintType::LinearGradient,
            PaintType::RadialGradient,
            PaintType::Pattern,
            PaintType::MeshGradient,
            PaintType::Swatch,
        ]
    }
}

/// Complete fill-and-stroke state for a selected element.
#[derive(Debug, Clone, PartialEq)]
pub struct FillStrokeState {
    /// Paint type applied to the fill.
    pub fill_type: PaintType,
    /// Paint type applied to the stroke.
    pub stroke_type: PaintType,
    /// Fill color as RGBA in `[0.0, 1.0]`.
    pub fill_color: [f64; 4],
    /// Stroke color as RGBA in `[0.0, 1.0]`.
    pub stroke_color: [f64; 4],
    /// Stroke width in user units.
    pub stroke_width: f64,
    /// Stroke line-cap style (`"butt"`, `"round"`, `"square"`).
    pub stroke_cap: String,
    /// Stroke line-join style (`"miter"`, `"round"`, `"bevel"`).
    pub stroke_join: String,
    /// Dash pattern as a list of dash/gap lengths.
    pub dash_pattern: Vec<f64>,
    /// Overall element opacity in `[0.0, 1.0]`.
    pub opacity: f64,
}

impl Default for FillStrokeState {
    fn default() -> Self {
        Self {
            fill_type: PaintType::FlatColor,
            stroke_type: PaintType::None,
            fill_color: [0.0, 0.0, 0.0, 1.0],
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            stroke_width: 1.0,
            stroke_cap: "butt".to_string(),
            stroke_join: "miter".to_string(),
            dash_pattern: Vec::new(),
            opacity: 1.0,
        }
    }
}

impl FillStrokeState {
    /// Return `true` if neither fill nor stroke is set.
    pub fn is_invisible(&self) -> bool {
        self.fill_type == PaintType::None && self.stroke_type == PaintType::None
    }

    /// Return the fill color as a CSS hex string (e.g. `"#ff0000"`).
    pub fn fill_hex(&self) -> String {
        let [r, g, b, _] = self.fill_color;
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    }

    /// Return the stroke color as a CSS hex string (e.g. `"#000000"`).
    pub fn stroke_hex(&self) -> String {
        let [r, g, b, _] = self.stroke_color;
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = FillStrokeState::default();
        assert_eq!(state.fill_type, PaintType::FlatColor);
        assert_eq!(state.stroke_type, PaintType::None);
        assert!((state.opacity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fill_hex_conversion() {
        let mut state = FillStrokeState::default();
        state.fill_color = [1.0, 0.0, 0.0, 1.0];
        assert_eq!(state.fill_hex(), "#ff0000");
    }

    #[test]
    fn is_invisible_when_no_paint() {
        let state = FillStrokeState {
            fill_type: PaintType::None,
            stroke_type: PaintType::None,
            ..Default::default()
        };
        assert!(state.is_invisible());
    }
}
