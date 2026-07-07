//! Document color profile settings.
//!
//! Manages the color profile configuration for a document, including
//! the working color space, display profile, and proofing profile.

use crate::color::{ColorF64, ColorSpace};

/// The color profile configuration for a document.
#[derive(Debug, Clone)]
pub struct DocumentColorProfile {
    /// The working color space for the document.
    pub working_space: ColorSpace,
    /// The display (monitor) color profile name, if set.
    pub display_profile: Option<String>,
    /// The proofing (soft-proof target) profile name, if set.
    pub proof_profile: Option<String>,
    /// Whether soft proofing is enabled.
    pub soft_proof_enabled: bool,
    /// The rendering intent for display.
    pub display_intent: RenderingIntent,
    /// The rendering intent for proofing.
    pub proof_intent: RenderingIntent,
    /// Whether to simulate paper color in soft proofing.
    pub simulate_paper: bool,
    /// Whether to gamut-warn out-of-gamut colors.
    pub gamut_warning: bool,
    /// Color used for gamut warning overlay.
    pub gamut_warning_color: ColorF64,
}

/// ICC rendering intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingIntent {
    /// Perceptual - compresses the entire source gamut to fit the
    /// destination gamut. Good for photographs.
    Perceptual,
    /// Relative colorimetric - maps white point, clips out-of-gamut.
    /// Good for logos and spot colors.
    RelativeColorimetric,
    /// Saturation - maps saturated colors to saturated colors.
    /// Good for business graphics.
    Saturation,
    /// Absolute colorimetric - no white point mapping, clips out-of-gamut.
    /// Good for proofing.
    AbsoluteColorimetric,
}

impl Default for DocumentColorProfile {
    fn default() -> Self {
        Self {
            working_space: ColorSpace::Srgb,
            display_profile: None,
            proof_profile: None,
            soft_proof_enabled: false,
            display_intent: RenderingIntent::RelativeColorimetric,
            proof_intent: RenderingIntent::RelativeColorimetric,
            simulate_paper: false,
            gamut_warning: false,
            gamut_warning_color: ColorF64::new(0.0, 1.0, 0.0, 1.0, ColorSpace::Srgb),
        }
    }
}

impl DocumentColorProfile {
    /// Create a new document color profile with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working color space.
    pub fn set_working_space(&mut self, space: ColorSpace) {
        self.working_space = space;
    }

    /// Set the display profile.
    pub fn set_display_profile(&mut self, name: Option<String>) {
        self.display_profile = name;
    }

    /// Set the proofing profile.
    pub fn set_proof_profile(&mut self, name: Option<String>) {
        self.proof_profile = name;
    }

    /// Toggle soft proofing.
    pub fn toggle_soft_proof(&mut self) {
        self.soft_proof_enabled = !self.soft_proof_enabled;
    }

    /// Convert a color from the working space to the display space.
    ///
    /// If no display profile is set, converts to sRGB.
    pub fn to_display(&self, color: &ColorF64) -> ColorF64 {
        // Without full ICC transform support, we convert to sRGB
        // as a reasonable default for display.
        color.to_srgb()
    }

    /// Convert a color to the proofing space.
    ///
    /// If soft proofing is disabled or no proof profile is set,
    /// returns the color unchanged.
    pub fn to_proof(&self, color: &ColorF64) -> ColorF64 {
        if !self.soft_proof_enabled || self.proof_profile.is_none() {
            return *color;
        }
        // Simplified: convert to CMYK and back as a basic proof simulation
        color.to_space(ColorSpace::Cmyk).to_srgb()
    }

    /// Check if a color is within the display gamut.
    ///
    /// Simplified: checks if sRGB components are in [0, 1].
    pub fn is_in_gamut(&self, color: &ColorF64) -> bool {
        let srgb = color.to_srgb();
        srgb.components[0] >= 0.0
            && srgb.components[0] <= 1.0
            && srgb.components[1] >= 0.0
            && srgb.components[1] <= 1.0
            && srgb.components[2] >= 0.0
            && srgb.components[2] <= 1.0
    }
}

impl RenderingIntent {
    /// Get the human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            RenderingIntent::Perceptual => "Perceptual",
            RenderingIntent::RelativeColorimetric => "Relative Colorimetric",
            RenderingIntent::Saturation => "Saturation",
            RenderingIntent::AbsoluteColorimetric => "Absolute Colorimetric",
        }
    }

    /// Get the ICC profile intent code (0-3).
    pub fn icc_code(&self) -> u8 {
        match self {
            RenderingIntent::Perceptual => 0,
            RenderingIntent::RelativeColorimetric => 1,
            RenderingIntent::Saturation => 2,
            RenderingIntent::AbsoluteColorimetric => 3,
        }
    }

    /// Create from ICC profile intent code.
    pub fn from_icc_code(code: u8) -> Self {
        match code {
            0 => RenderingIntent::Perceptual,
            1 => RenderingIntent::RelativeColorimetric,
            2 => RenderingIntent::Saturation,
            3 => RenderingIntent::AbsoluteColorimetric,
            _ => RenderingIntent::Perceptual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile() {
        let profile = DocumentColorProfile::new();
        assert_eq!(profile.working_space, ColorSpace::Srgb);
        assert!(!profile.soft_proof_enabled);
        assert!(profile.display_profile.is_none());
        assert!(profile.proof_profile.is_none());
    }

    #[test]
    fn set_working_space() {
        let mut profile = DocumentColorProfile::new();
        profile.set_working_space(ColorSpace::LinearRgb);
        assert_eq!(profile.working_space, ColorSpace::LinearRgb);
    }

    #[test]
    fn toggle_soft_proof() {
        let mut profile = DocumentColorProfile::new();
        assert!(!profile.soft_proof_enabled);
        profile.toggle_soft_proof();
        assert!(profile.soft_proof_enabled);
        profile.toggle_soft_proof();
        assert!(!profile.soft_proof_enabled);
    }

    #[test]
    fn rendering_intent_roundtrip() {
        for code in 0..4 {
            let intent = RenderingIntent::from_icc_code(code);
            assert_eq!(intent.icc_code(), code);
        }
    }

    #[test]
    fn rendering_intent_display_names() {
        assert_eq!(RenderingIntent::Perceptual.display_name(), "Perceptual");
        assert_eq!(
            RenderingIntent::AbsoluteColorimetric.display_name(),
            "Absolute Colorimetric"
        );
    }

    #[test]
    fn in_gamut_check() {
        let profile = DocumentColorProfile::new();
        let srgb = ColorF64::new(0.5, 0.5, 0.5, 1.0, ColorSpace::Srgb);
        assert!(profile.is_in_gamut(&srgb));
    }

    #[test]
    fn to_display_identity() {
        let profile = DocumentColorProfile::new();
        let color = ColorF64::new(0.5, 0.3, 0.8, 1.0, ColorSpace::Srgb);
        let display = profile.to_display(&color);
        assert!((display.components[0] - 0.5).abs() < 1e-6);
    }
}
