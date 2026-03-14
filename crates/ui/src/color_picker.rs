//! Color picker data model (11.5.1-11.5.10).
//!
//! Provides the color picker mode, state, and color conversion utilities.

/// The active mode / tab of the color picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerMode {
    /// Color wheel with saturation/lightness triangle.
    Wheel,
    /// RGB sliders (0-255).
    RgbSliders,
    /// HSL sliders.
    HslSliders,
    /// CMYK sliders.
    CmykSliders,
    /// Direct hex input field.
    HexInput,
    /// OkLab perceptual sliders.
    OkLabSliders,
}

impl ColorPickerMode {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ColorPickerMode::Wheel => "Wheel",
            ColorPickerMode::RgbSliders => "RGB",
            ColorPickerMode::HslSliders => "HSL",
            ColorPickerMode::CmykSliders => "CMYK",
            ColorPickerMode::HexInput => "Hex",
            ColorPickerMode::OkLabSliders => "OkLab",
        }
    }

    /// All modes in display order.
    pub fn all() -> &'static [ColorPickerMode] {
        &[
            ColorPickerMode::Wheel,
            ColorPickerMode::RgbSliders,
            ColorPickerMode::HslSliders,
            ColorPickerMode::CmykSliders,
            ColorPickerMode::HexInput,
            ColorPickerMode::OkLabSliders,
        ]
    }
}

/// State of the color picker widget.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPickerState {
    /// Currently active picker mode.
    pub mode: ColorPickerMode,
    /// Current color as RGBA in `[0.0, 1.0]`.
    pub color: [f64; 4],
    /// Alpha channel value in `[0.0, 1.0]`.
    pub alpha: f64,
    /// Recently used colors (RGBA).
    pub recent_colors: Vec<[f64; 4]>,
    /// Current hex input string.
    pub hex_input: String,
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            mode: ColorPickerMode::Wheel,
            color: [0.0, 0.0, 0.0, 1.0],
            alpha: 1.0,
            recent_colors: Vec::new(),
            hex_input: "#000000".to_string(),
        }
    }
}

impl ColorPickerState {
    /// Set the color from RGB values in `[0.0, 1.0]`, updating the hex field.
    pub fn set_rgb(&mut self, r: f64, g: f64, b: f64) {
        self.color = [r, g, b, self.alpha];
        self.hex_input = color_to_hex(r, g, b);
    }

    /// Set the color from a hex string, updating the RGBA fields.
    /// Returns `true` if the hex string was valid.
    pub fn set_hex(&mut self, hex: &str) -> bool {
        if let Some((r, g, b)) = hex_to_color(hex) {
            self.color = [r, g, b, self.alpha];
            self.hex_input = hex.to_string();
            true
        } else {
            false
        }
    }

    /// Push the current color onto the recent colors list (keeping at most 16).
    pub fn push_recent(&mut self) {
        if self.recent_colors.len() >= 16 {
            self.recent_colors.remove(0);
        }
        self.recent_colors.push(self.color);
    }
}

/// Convert RGB values in `[0.0, 1.0]` to a CSS hex color string.
pub fn color_to_hex(r: f64, g: f64, b: f64) -> String {
    let r8 = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g8 = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b8 = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", r8, g8, b8)
}

/// Parse a CSS hex color string (e.g. `"#ff0000"` or `"ff0000"` or `"#f00"`)
/// into RGB values in `[0.0, 1.0]`.
pub fn hex_to_color(hex: &str) -> Option<(f64, f64, f64)> {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some((
                (r * 17) as f64 / 255.0,
                (g * 17) as f64 / 255.0,
                (b * 17) as f64 / 255.0,
            ))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_to_hex_basic() {
        assert_eq!(color_to_hex(1.0, 0.0, 0.0), "#ff0000");
        assert_eq!(color_to_hex(0.0, 1.0, 0.0), "#00ff00");
        assert_eq!(color_to_hex(0.0, 0.0, 1.0), "#0000ff");
        assert_eq!(color_to_hex(0.0, 0.0, 0.0), "#000000");
        assert_eq!(color_to_hex(1.0, 1.0, 1.0), "#ffffff");
    }

    #[test]
    fn hex_to_color_6_digit() {
        let (r, g, b) = hex_to_color("#ff0000").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);
    }

    #[test]
    fn hex_to_color_3_digit() {
        let (r, g, b) = hex_to_color("#f00").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);
    }

    #[test]
    fn hex_to_color_invalid() {
        assert!(hex_to_color("xyz").is_none());
        assert!(hex_to_color("#gggggg").is_none());
    }

    #[test]
    fn roundtrip_hex() {
        let hex = color_to_hex(0.5, 0.25, 0.75);
        let (r, g, b) = hex_to_color(&hex).unwrap();
        assert!((r - 0.5).abs() < 0.01);
        assert!((g - 0.25).abs() < 0.02);
        assert!((b - 0.75).abs() < 0.01);
    }
}
