//! Font variant support.
//!
//! Provides types for CSS font variant properties that map to OpenType features:
//!
//! - `font-variant-caps`: small-caps, all-small-caps, petite-caps
//! - `font-variant-ligatures`: common-ligatures, no-common-ligatures
//! - `font-variant-numeric`: lining-nums, oldstyle-nums, proportional-nums, tabular-nums
//!
//! These are stored as feature flags that get passed to the text shaper.

// ---------------------------------------------------------------------------
// FontVariantCaps
// ---------------------------------------------------------------------------

/// CSS `font-variant-caps` property values.
///
/// Maps to OpenType features:
/// - `small-caps` -> `smcp`
/// - `all-small-caps` -> `smcp` + `c2sc`
/// - `petite-caps` -> `pcap`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontVariantCaps {
    /// Normal capitalization (no OpenType feature).
    #[default]
    Normal,
    /// Lowercase letters are rendered as small capitals.
    /// OpenType feature: `smcp`.
    SmallCaps,
    /// Both upper and lowercase are rendered as small capitals.
    /// OpenType features: `smcp` + `c2sc`.
    AllSmallCaps,
    /// Lowercase letters are rendered as petite capitals (smaller than small-caps).
    /// OpenType feature: `pcap`.
    PetiteCaps,
}

impl FontVariantCaps {
    /// Parse a CSS `font-variant-caps` string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "small-caps" => FontVariantCaps::SmallCaps,
            "all-small-caps" => FontVariantCaps::AllSmallCaps,
            "petite-caps" => FontVariantCaps::PetiteCaps,
            _ => FontVariantCaps::Normal,
        }
    }

    /// Return the OpenType feature tags enabled by this variant.
    pub fn opentype_features(&self) -> Vec<OpenTypeFeature> {
        match self {
            FontVariantCaps::Normal => Vec::new(),
            FontVariantCaps::SmallCaps => vec![OpenTypeFeature::new(*b"smcp", 1)],
            FontVariantCaps::AllSmallCaps => vec![
                OpenTypeFeature::new(*b"smcp", 1),
                OpenTypeFeature::new(*b"c2sc", 1),
            ],
            FontVariantCaps::PetiteCaps => vec![OpenTypeFeature::new(*b"pcap", 1)],
        }
    }
}

// ---------------------------------------------------------------------------
// FontVariantLigatures
// ---------------------------------------------------------------------------

/// CSS `font-variant-ligatures` property values.
///
/// Maps to OpenType features:
/// - `common-ligatures` -> `liga` + `clig` (enabled)
/// - `no-common-ligatures` -> `liga` + `clig` (disabled)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontVariantLigatures {
    /// Normal ligature behavior (browser default, usually common ligatures enabled).
    #[default]
    Normal,
    /// Explicitly enable common ligatures.
    /// OpenType features: `liga` = 1, `clig` = 1.
    CommonLigatures,
    /// Explicitly disable common ligatures.
    /// OpenType features: `liga` = 0, `clig` = 0.
    NoCommonLigatures,
}

impl FontVariantLigatures {
    /// Parse a CSS `font-variant-ligatures` string.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "common-ligatures" => FontVariantLigatures::CommonLigatures,
            "no-common-ligatures" => FontVariantLigatures::NoCommonLigatures,
            _ => FontVariantLigatures::Normal,
        }
    }

    /// Return the OpenType feature tags set by this variant.
    pub fn opentype_features(&self) -> Vec<OpenTypeFeature> {
        match self {
            FontVariantLigatures::Normal => Vec::new(),
            FontVariantLigatures::CommonLigatures => vec![
                OpenTypeFeature::new(*b"liga", 1),
                OpenTypeFeature::new(*b"clig", 1),
            ],
            FontVariantLigatures::NoCommonLigatures => vec![
                OpenTypeFeature::new(*b"liga", 0),
                OpenTypeFeature::new(*b"clig", 0),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// FontVariantNumeric
// ---------------------------------------------------------------------------

/// CSS `font-variant-numeric` property values.
///
/// Multiple values can be combined (e.g., `lining-nums tabular-nums`).
///
/// Maps to OpenType features:
/// - `lining-nums` -> `lnum`
/// - `oldstyle-nums` -> `onum`
/// - `proportional-nums` -> `pnum`
/// - `tabular-nums` -> `tnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontVariantNumeric {
    /// Use lining (uppercase-height) figures. OpenType: `lnum`.
    pub lining_nums: bool,
    /// Use oldstyle (lowercase-height) figures. OpenType: `onum`.
    pub oldstyle_nums: bool,
    /// Use proportionally-spaced figures. OpenType: `pnum`.
    pub proportional_nums: bool,
    /// Use tabular (fixed-width) figures. OpenType: `tnum`.
    pub tabular_nums: bool,
}

impl FontVariantNumeric {
    /// Create a default (normal) numeric variant.
    pub fn normal() -> Self {
        Self::default()
    }

    /// Parse a CSS `font-variant-numeric` string.
    ///
    /// Multiple values are space-separated:
    /// `"lining-nums tabular-nums"` enables both features.
    pub fn parse(s: &str) -> Self {
        let mut result = Self::default();
        for token in s.split_whitespace() {
            match token.to_lowercase().as_str() {
                "lining-nums" => result.lining_nums = true,
                "oldstyle-nums" => result.oldstyle_nums = true,
                "proportional-nums" => result.proportional_nums = true,
                "tabular-nums" => result.tabular_nums = true,
                _ => {}
            }
        }
        result
    }

    /// Whether any numeric variant is set.
    pub fn is_normal(&self) -> bool {
        !self.lining_nums && !self.oldstyle_nums && !self.proportional_nums && !self.tabular_nums
    }

    /// Return the OpenType feature tags enabled by this variant.
    pub fn opentype_features(&self) -> Vec<OpenTypeFeature> {
        let mut features = Vec::new();
        if self.lining_nums {
            features.push(OpenTypeFeature::new(*b"lnum", 1));
        }
        if self.oldstyle_nums {
            features.push(OpenTypeFeature::new(*b"onum", 1));
        }
        if self.proportional_nums {
            features.push(OpenTypeFeature::new(*b"pnum", 1));
        }
        if self.tabular_nums {
            features.push(OpenTypeFeature::new(*b"tnum", 1));
        }
        features
    }
}

// ---------------------------------------------------------------------------
// OpenTypeFeature
// ---------------------------------------------------------------------------

/// An OpenType feature setting.
///
/// Represents a single OpenType feature tag with a value. The tag is a
/// 4-byte ASCII identifier (e.g., `smcp`, `liga`, `lnum`). The value
/// is typically 0 (disabled) or 1 (enabled), but some features accept
/// other values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenTypeFeature {
    /// The 4-byte feature tag (e.g., `b"smcp"`, `b"liga"`).
    pub tag: [u8; 4],
    /// Feature value (0 = disabled, 1 = enabled, other values feature-specific).
    pub value: u32,
}

impl OpenTypeFeature {
    /// Create a new OpenType feature setting.
    pub fn new(tag: [u8; 4], value: u32) -> Self {
        Self { tag, value }
    }

    /// Return the tag as a string (for debugging / display).
    pub fn tag_str(&self) -> &str {
        std::str::from_utf8(&self.tag).unwrap_or("????")
    }

    /// Whether this feature is enabled (value > 0).
    pub fn is_enabled(&self) -> bool {
        self.value > 0
    }
}

impl std::fmt::Display for OpenTypeFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\" = {}", self.tag_str(), self.value)
    }
}

// ---------------------------------------------------------------------------
// FontVariantSettings - aggregate
// ---------------------------------------------------------------------------

/// Aggregated font variant settings combining caps, ligatures, and numeric.
///
/// This is a convenience struct that collects all font-variant sub-properties
/// and can produce a unified list of OpenType features for the shaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontVariantSettings {
    /// `font-variant-caps` setting.
    pub caps: FontVariantCaps,
    /// `font-variant-ligatures` setting.
    pub ligatures: FontVariantLigatures,
    /// `font-variant-numeric` setting.
    pub numeric: FontVariantNumeric,
}

impl FontVariantSettings {
    /// Create default (normal) settings.
    pub fn normal() -> Self {
        Self::default()
    }

    /// Collect all OpenType features from all sub-properties.
    pub fn opentype_features(&self) -> Vec<OpenTypeFeature> {
        let mut features = Vec::new();
        features.extend(self.caps.opentype_features());
        features.extend(self.ligatures.opentype_features());
        features.extend(self.numeric.opentype_features());
        features
    }

    /// Whether all sub-properties are at their default (normal) values.
    pub fn is_normal(&self) -> bool {
        self.caps == FontVariantCaps::Normal
            && self.ligatures == FontVariantLigatures::Normal
            && self.numeric.is_normal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // FontVariantCaps
    // -----------------------------------------------------------------------

    #[test]
    fn test_caps_parse() {
        assert_eq!(FontVariantCaps::parse("normal"), FontVariantCaps::Normal);
        assert_eq!(
            FontVariantCaps::parse("small-caps"),
            FontVariantCaps::SmallCaps
        );
        assert_eq!(
            FontVariantCaps::parse("all-small-caps"),
            FontVariantCaps::AllSmallCaps
        );
        assert_eq!(
            FontVariantCaps::parse("petite-caps"),
            FontVariantCaps::PetiteCaps
        );
        assert_eq!(FontVariantCaps::parse("unknown"), FontVariantCaps::Normal);
    }

    #[test]
    fn test_caps_opentype_features_normal() {
        let features = FontVariantCaps::Normal.opentype_features();
        assert!(features.is_empty());
    }

    #[test]
    fn test_caps_opentype_features_small_caps() {
        let features = FontVariantCaps::SmallCaps.opentype_features();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].tag, *b"smcp");
        assert_eq!(features[0].value, 1);
    }

    #[test]
    fn test_caps_opentype_features_all_small_caps() {
        let features = FontVariantCaps::AllSmallCaps.opentype_features();
        assert_eq!(features.len(), 2);
        let tags: Vec<&str> = features.iter().map(|f| f.tag_str()).collect();
        assert!(tags.contains(&"smcp"));
        assert!(tags.contains(&"c2sc"));
    }

    #[test]
    fn test_caps_opentype_features_petite_caps() {
        let features = FontVariantCaps::PetiteCaps.opentype_features();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].tag, *b"pcap");
        assert_eq!(features[0].value, 1);
    }

    // -----------------------------------------------------------------------
    // FontVariantLigatures
    // -----------------------------------------------------------------------

    #[test]
    fn test_ligatures_parse() {
        assert_eq!(
            FontVariantLigatures::parse("normal"),
            FontVariantLigatures::Normal
        );
        assert_eq!(
            FontVariantLigatures::parse("common-ligatures"),
            FontVariantLigatures::CommonLigatures
        );
        assert_eq!(
            FontVariantLigatures::parse("no-common-ligatures"),
            FontVariantLigatures::NoCommonLigatures
        );
    }

    #[test]
    fn test_ligatures_opentype_features_normal() {
        let features = FontVariantLigatures::Normal.opentype_features();
        assert!(features.is_empty());
    }

    #[test]
    fn test_ligatures_opentype_features_common() {
        let features = FontVariantLigatures::CommonLigatures.opentype_features();
        assert_eq!(features.len(), 2);
        assert!(features.iter().all(|f| f.is_enabled()));
        let tags: Vec<&str> = features.iter().map(|f| f.tag_str()).collect();
        assert!(tags.contains(&"liga"));
        assert!(tags.contains(&"clig"));
    }

    #[test]
    fn test_ligatures_opentype_features_no_common() {
        let features = FontVariantLigatures::NoCommonLigatures.opentype_features();
        assert_eq!(features.len(), 2);
        assert!(features.iter().all(|f| !f.is_enabled()));
        assert!(features.iter().all(|f| f.value == 0));
    }

    // -----------------------------------------------------------------------
    // FontVariantNumeric
    // -----------------------------------------------------------------------

    #[test]
    fn test_numeric_parse_single() {
        let n = FontVariantNumeric::parse("lining-nums");
        assert!(n.lining_nums);
        assert!(!n.oldstyle_nums);
        assert!(!n.proportional_nums);
        assert!(!n.tabular_nums);
    }

    #[test]
    fn test_numeric_parse_multiple() {
        let n = FontVariantNumeric::parse("lining-nums tabular-nums");
        assert!(n.lining_nums);
        assert!(!n.oldstyle_nums);
        assert!(!n.proportional_nums);
        assert!(n.tabular_nums);
    }

    #[test]
    fn test_numeric_parse_all() {
        let n =
            FontVariantNumeric::parse("lining-nums oldstyle-nums proportional-nums tabular-nums");
        assert!(n.lining_nums);
        assert!(n.oldstyle_nums);
        assert!(n.proportional_nums);
        assert!(n.tabular_nums);
    }

    #[test]
    fn test_numeric_parse_normal() {
        let n = FontVariantNumeric::parse("normal");
        assert!(n.is_normal());
    }

    #[test]
    fn test_numeric_opentype_features() {
        let n = FontVariantNumeric::parse("lining-nums tabular-nums");
        let features = n.opentype_features();
        assert_eq!(features.len(), 2);
        let tags: Vec<&str> = features.iter().map(|f| f.tag_str()).collect();
        assert!(tags.contains(&"lnum"));
        assert!(tags.contains(&"tnum"));
    }

    #[test]
    fn test_numeric_is_normal() {
        assert!(FontVariantNumeric::default().is_normal());
        assert!(!FontVariantNumeric::parse("lining-nums").is_normal());
    }

    // -----------------------------------------------------------------------
    // OpenTypeFeature
    // -----------------------------------------------------------------------

    #[test]
    fn test_opentype_feature_display() {
        let f = OpenTypeFeature::new(*b"smcp", 1);
        assert_eq!(format!("{}", f), "\"smcp\" = 1");
    }

    #[test]
    fn test_opentype_feature_tag_str() {
        let f = OpenTypeFeature::new(*b"liga", 0);
        assert_eq!(f.tag_str(), "liga");
    }

    #[test]
    fn test_opentype_feature_is_enabled() {
        assert!(OpenTypeFeature::new(*b"smcp", 1).is_enabled());
        assert!(!OpenTypeFeature::new(*b"liga", 0).is_enabled());
        assert!(OpenTypeFeature::new(*b"aalt", 2).is_enabled());
    }

    // -----------------------------------------------------------------------
    // FontVariantSettings (aggregate)
    // -----------------------------------------------------------------------

    #[test]
    fn test_settings_normal() {
        let s = FontVariantSettings::normal();
        assert!(s.is_normal());
        assert!(s.opentype_features().is_empty());
    }

    #[test]
    fn test_settings_combined() {
        let s = FontVariantSettings {
            caps: FontVariantCaps::SmallCaps,
            ligatures: FontVariantLigatures::NoCommonLigatures,
            numeric: FontVariantNumeric::parse("tabular-nums"),
        };
        assert!(!s.is_normal());
        let features = s.opentype_features();
        // smcp (1) + liga (0) + clig (0) + tnum (1) = 4 features
        assert_eq!(features.len(), 4);
        let tags: Vec<&str> = features.iter().map(|f| f.tag_str()).collect();
        assert!(tags.contains(&"smcp"));
        assert!(tags.contains(&"liga"));
        assert!(tags.contains(&"clig"));
        assert!(tags.contains(&"tnum"));
    }

    #[test]
    fn test_settings_all_small_caps_plus_numeric() {
        let s = FontVariantSettings {
            caps: FontVariantCaps::AllSmallCaps,
            ligatures: FontVariantLigatures::Normal,
            numeric: FontVariantNumeric::parse("oldstyle-nums proportional-nums"),
        };
        let features = s.opentype_features();
        // smcp + c2sc + onum + pnum = 4 features
        assert_eq!(features.len(), 4);
    }
}
