//! SVG attribute value types: lengths, colors, paint, enums.

use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// SvgLength
// ---------------------------------------------------------------------------

/// A length value with an optional unit, as used throughout SVG attributes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgLength {
    pub value: f64,
    pub unit: LengthUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    None,
    Px,
    Pt,
    Mm,
    Cm,
    In,
    Em,
    Ex,
    Percent,
}

impl Default for SvgLength {
    fn default() -> Self {
        Self {
            value: 0.0,
            unit: LengthUnit::None,
        }
    }
}

impl SvgLength {
    pub fn new(value: f64, unit: LengthUnit) -> Self {
        Self { value, unit }
    }

    pub fn px(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Px,
        }
    }

    pub fn zero() -> Self {
        Self::default()
    }

    /// Resolve to user-space pixels (96 dpi baseline).
    ///
    /// For `em`, `ex`, and `%` units, this returns the raw value since no
    /// context is available. Use [`Self::resolve_px`] for context-aware resolution.
    pub fn to_px(&self) -> f64 {
        match self.unit {
            LengthUnit::None | LengthUnit::Px => self.value,
            LengthUnit::Pt => self.value * 96.0 / 72.0,
            LengthUnit::Mm => self.value * 96.0 / 25.4,
            LengthUnit::Cm => self.value * 96.0 / 2.54,
            LengthUnit::In => self.value * 96.0,
            // Em/Ex/Percent need context – return value as-is for now
            LengthUnit::Em | LengthUnit::Ex | LengthUnit::Percent => self.value,
        }
    }

    /// Resolve to user-space pixels with context.
    ///
    /// - `font_size`: the computed font-size in px (for `em`/`ex` units)
    /// - `reference_length`: the reference dimension in px (for `%` units,
    ///   e.g. viewport width for horizontal lengths, height for vertical)
    pub fn resolve_px(&self, font_size: f64, reference_length: f64) -> f64 {
        match self.unit {
            LengthUnit::None | LengthUnit::Px => self.value,
            LengthUnit::Pt => self.value * 96.0 / 72.0,
            LengthUnit::Mm => self.value * 96.0 / 25.4,
            LengthUnit::Cm => self.value * 96.0 / 2.54,
            LengthUnit::In => self.value * 96.0,
            LengthUnit::Em => self.value * font_size,
            LengthUnit::Ex => self.value * font_size * 0.5, // approximate: ex ≈ 0.5em
            LengthUnit::Percent => self.value * reference_length / 100.0,
        }
    }
}

impl fmt::Display for SvgLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            LengthUnit::None => write!(f, "{}", self.value),
            LengthUnit::Px => write!(f, "{}px", self.value),
            LengthUnit::Pt => write!(f, "{}pt", self.value),
            LengthUnit::Mm => write!(f, "{}mm", self.value),
            LengthUnit::Cm => write!(f, "{}cm", self.value),
            LengthUnit::In => write!(f, "{}in", self.value),
            LengthUnit::Em => write!(f, "{}em", self.value),
            LengthUnit::Ex => write!(f, "{}ex", self.value),
            LengthUnit::Percent => write!(f, "{}%", self.value),
        }
    }
}

impl FromStr for SvgLength {
    type Err = SvgLengthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(SvgLengthParseError);
        }

        // Try to find where the numeric part ends
        let (num_str, unit_str) = split_number_unit(s);
        let value: f64 = num_str.parse().map_err(|_| SvgLengthParseError)?;

        let unit = match unit_str {
            "" => LengthUnit::None,
            "px" => LengthUnit::Px,
            "pt" => LengthUnit::Pt,
            "mm" => LengthUnit::Mm,
            "cm" => LengthUnit::Cm,
            "in" => LengthUnit::In,
            "em" => LengthUnit::Em,
            "ex" => LengthUnit::Ex,
            "%" => LengthUnit::Percent,
            _ => return Err(SvgLengthParseError),
        };

        Ok(SvgLength { value, unit })
    }
}

/// Split a string like "12.5px" into ("12.5", "px").
fn split_number_unit(s: &str) -> (&str, &str) {
    // Find the first character that is not part of a number
    let end = s
        .find(|c: char| {
            !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e' && c != 'E'
        })
        .unwrap_or(s.len());

    // Handle the case where 'e'/'E' is the exponent in scientific notation vs. unit start
    // Simple heuristic: if the char after 'e'/'E' is a digit or +/-, it's scientific notation
    let end = {
        let bytes = s.as_bytes();
        if end > 0 && end < s.len() {
            let prev = bytes[end - 1];
            if prev == b'e' || prev == b'E' {
                // Check if this is scientific notation
                let next = bytes.get(end);
                if next.is_some_and(|c| c.is_ascii_digit() || *c == b'+' || *c == b'-') {
                    // It's scientific notation, find the real unit start
                    let rest = &s[end..];
                    let extra = rest
                        .find(|c: char| !c.is_ascii_digit() && c != '+' && c != '-')
                        .unwrap_or(rest.len());
                    end + extra
                } else {
                    // 'e'/'E' is start of unit like "em"/"ex", back up
                    end - 1
                }
            } else {
                end
            }
        } else {
            end
        }
    };

    (&s[..end], &s[end..])
}

#[derive(Debug, Clone)]
pub struct SvgLengthParseError;

impl fmt::Display for SvgLengthParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid SVG length")
    }
}

impl std::error::Error for SvgLengthParseError {}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// An RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    // Named colors
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 128, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const CYAN: Color = Color::rgb(0, 255, 255);
    pub const MAGENTA: Color = Color::rgb(255, 0, 255);
    pub const ORANGE: Color = Color::rgb(255, 165, 0);
    pub const PURPLE: Color = Color::rgb(128, 0, 128);
    pub const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

    /// Parse from hex: `#rrggbb`, `#rgb`, `#rrggbbaa`, `#rgba`.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#')?;
        // The arms below dispatch on the byte length and then slice by byte
        // offsets, which would panic on a multi-byte character boundary.
        // Hex digits are ASCII by definition, so reject anything else early.
        if !s.is_ascii() {
            return None;
        }
        match s.len() {
            3 => {
                let r = u8::from_str_radix(&s[0..1], 16).ok()?;
                let g = u8::from_str_radix(&s[1..2], 16).ok()?;
                let b = u8::from_str_radix(&s[2..3], 16).ok()?;
                Some(Color::rgb(r * 17, g * 17, b * 17))
            }
            4 => {
                let r = u8::from_str_radix(&s[0..1], 16).ok()?;
                let g = u8::from_str_radix(&s[1..2], 16).ok()?;
                let b = u8::from_str_radix(&s[2..3], 16).ok()?;
                let a = u8::from_str_radix(&s[3..4], 16).ok()?;
                Some(Color::new(r * 17, g * 17, b * 17, a * 17))
            }
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some(Color::new(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Parse a CSS color string (hex or named).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.starts_with('#') {
            return Self::from_hex(s);
        }
        if s.starts_with("rgb") {
            return Self::parse_rgb_func(s);
        }
        Self::from_named(s)
    }

    /// Parse `rgb(r, g, b)` or `rgba(r, g, b, a)` functional notation.
    fn parse_rgb_func(s: &str) -> Option<Self> {
        let inner = s
            .strip_prefix("rgba(")
            .or_else(|| s.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = inner.split([',', '/']).map(|p| p.trim()).collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parse_color_component(parts[0])?;
        let g = parse_color_component(parts[1])?;
        let b = parse_color_component(parts[2])?;
        let a = if parts.len() >= 4 {
            parse_alpha_component(parts[3])?
        } else {
            255
        };
        Some(Color::new(r, g, b, a))
    }

    /// Look up a named CSS color.
    pub fn from_named(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "black" => Some(Self::BLACK),
            "white" => Some(Self::WHITE),
            "red" => Some(Self::RED),
            "green" => Some(Self::GREEN),
            "blue" => Some(Self::BLUE),
            "yellow" => Some(Self::YELLOW),
            "cyan" | "aqua" => Some(Self::CYAN),
            "magenta" | "fuchsia" => Some(Self::MAGENTA),
            "orange" => Some(Self::ORANGE),
            "purple" => Some(Self::PURPLE),
            "transparent" => Some(Self::TRANSPARENT),
            "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
            "silver" => Some(Color::rgb(192, 192, 192)),
            "maroon" => Some(Color::rgb(128, 0, 0)),
            "olive" => Some(Color::rgb(128, 128, 0)),
            "lime" => Some(Color::rgb(0, 255, 0)),
            "teal" => Some(Color::rgb(0, 128, 128)),
            "navy" => Some(Color::rgb(0, 0, 128)),
            "none" => Some(Self::TRANSPARENT),
            _ => None,
        }
    }

    /// Serialize to `#rrggbb` (or `#rrggbbaa` when alpha != 255).
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

fn parse_color_component(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let v: f64 = pct.trim().parse().ok()?;
        Some((v * 255.0 / 100.0).round().clamp(0.0, 255.0) as u8)
    } else {
        s.parse::<u8>().ok()
    }
}

fn parse_alpha_component(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let v: f64 = pct.trim().parse().ok()?;
        Some((v * 255.0 / 100.0).round().clamp(0.0, 255.0) as u8)
    } else {
        // Could be 0.0–1.0 or 0–255
        let v: f64 = s.parse().ok()?;
        if v <= 1.0 && !s.contains('.') {
            // Ambiguous, treat integer <=1 as 0 or 1 out of 255
            Some(v.round().clamp(0.0, 255.0) as u8)
        } else if v <= 1.0 {
            Some((v * 255.0).round().clamp(0.0, 255.0) as u8)
        } else {
            Some(v.round().clamp(0.0, 255.0) as u8)
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ---------------------------------------------------------------------------
// Paint
// ---------------------------------------------------------------------------

/// An SVG paint value (fill or stroke).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Paint {
    /// `none`
    #[default]
    None,
    /// A solid color.
    Color(Color),
    /// `currentColor` keyword — resolved to the inherited `color` property.
    CurrentColor,
    /// `url(#id)` referencing a paint server (gradient, pattern, …).
    Url(String),
}

impl Paint {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("none") {
            return Some(Paint::None);
        }
        if s.eq_ignore_ascii_case("currentcolor") || s.eq_ignore_ascii_case("currentColor") {
            return Some(Paint::CurrentColor);
        }
        if let Some(rest) = s.strip_prefix("url(") {
            let url = rest
                .strip_suffix(')')?
                .trim()
                .trim_matches(|c| c == '\'' || c == '"');
            return Some(Paint::Url(url.to_string()));
        }
        Color::parse(s).map(Paint::Color)
    }

    pub fn to_svg(&self) -> String {
        match self {
            Paint::None => "none".to_string(),
            Paint::Color(c) => c.to_hex(),
            Paint::CurrentColor => "currentColor".to_string(),
            Paint::Url(u) => format!("url({})", u),
        }
    }
}

// ---------------------------------------------------------------------------
// ViewBox
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    pub min_x: f64,
    pub min_y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewBox {
    pub fn new(min_x: f64, min_y: f64, width: f64, height: f64) -> Self {
        Self {
            min_x,
            min_y,
            width,
            height,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<f64> = s
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse().ok())
            .collect();
        if parts.len() != 4 {
            return None;
        }
        Some(Self::new(parts[0], parts[1], parts[2], parts[3]))
    }

    pub fn to_svg(&self) -> String {
        format!(
            "{} {} {} {}",
            self.min_x, self.min_y, self.width, self.height
        )
    }
}

// ---------------------------------------------------------------------------
// Style enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

impl FillRule {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "nonzero" => Some(FillRule::NonZero),
            "evenodd" => Some(FillRule::EvenOdd),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            FillRule::NonZero => "nonzero",
            FillRule::EvenOdd => "evenodd",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

impl LineCap {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "butt" => Some(LineCap::Butt),
            "round" => Some(LineCap::Round),
            "square" => Some(LineCap::Square),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

impl LineJoin {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "miter" => Some(LineJoin::Miter),
            "round" => Some(LineJoin::Round),
            "bevel" => Some(LineJoin::Bevel),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Display {
    #[default]
    Inline,
    Block,
    None,
}

impl Display {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "inline" => Some(Display::Inline),
            "block" => Some(Display::Block),
            "none" => Some(Display::None),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            Display::Inline => "inline",
            Display::Block => "block",
            Display::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

impl Visibility {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "visible" => Some(Visibility::Visible),
            "hidden" => Some(Visibility::Hidden),
            "collapse" => Some(Visibility::Collapse),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            Visibility::Visible => "visible",
            Visibility::Hidden => "hidden",
            Visibility::Collapse => "collapse",
        }
    }
}

// ---------------------------------------------------------------------------
// GradientUnits / PatternUnits / MaskUnits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GradientUnits {
    #[default]
    ObjectBoundingBox,
    UserSpaceOnUse,
}

impl GradientUnits {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "objectBoundingBox" => Some(GradientUnits::ObjectBoundingBox),
            "userSpaceOnUse" => Some(GradientUnits::UserSpaceOnUse),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            GradientUnits::ObjectBoundingBox => "objectBoundingBox",
            GradientUnits::UserSpaceOnUse => "userSpaceOnUse",
        }
    }
}

// ---------------------------------------------------------------------------
// SpreadMethod
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpreadMethod {
    #[default]
    Pad,
    Reflect,
    Repeat,
}

impl SpreadMethod {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "pad" => Some(SpreadMethod::Pad),
            "reflect" => Some(SpreadMethod::Reflect),
            "repeat" => Some(SpreadMethod::Repeat),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            SpreadMethod::Pad => "pad",
            SpreadMethod::Reflect => "reflect",
            SpreadMethod::Repeat => "repeat",
        }
    }
}

// ---------------------------------------------------------------------------
// MarkerUnits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MarkerUnits {
    #[default]
    StrokeWidth,
    UserSpaceOnUse,
}

impl MarkerUnits {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "strokeWidth" => Some(MarkerUnits::StrokeWidth),
            "userSpaceOnUse" => Some(MarkerUnits::UserSpaceOnUse),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            MarkerUnits::StrokeWidth => "strokeWidth",
            MarkerUnits::UserSpaceOnUse => "userSpaceOnUse",
        }
    }
}

// ---------------------------------------------------------------------------
// TextAnchor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

impl TextAnchor {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "start" => Some(TextAnchor::Start),
            "middle" => Some(TextAnchor::Middle),
            "end" => Some(TextAnchor::End),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            TextAnchor::Start => "start",
            TextAnchor::Middle => "middle",
            TextAnchor::End => "end",
        }
    }
}

// ---------------------------------------------------------------------------
// FontWeight
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
    Number(f64),
}

impl FontWeight {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "normal" => Some(FontWeight::Normal),
            "bold" => Some(FontWeight::Bold),
            other => other.parse::<f64>().ok().map(FontWeight::Number),
        }
    }

    pub fn to_svg(&self) -> String {
        match self {
            FontWeight::Normal => "normal".to_string(),
            FontWeight::Bold => "bold".to_string(),
            FontWeight::Number(n) => n.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// FontStyle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

impl FontStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "normal" => Some(FontStyle::Normal),
            "italic" => Some(FontStyle::Italic),
            "oblique" => Some(FontStyle::Oblique),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        }
    }
}

// ---------------------------------------------------------------------------
// TextDecoration
// ---------------------------------------------------------------------------

/// SVG `text-decoration` value, modelled as a set of decoration lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextDecoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
}

impl TextDecoration {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s == "none" {
            return Some(Self::default());
        }
        let mut td = Self::default();
        let mut any = false;
        for token in s.split_whitespace() {
            match token {
                "underline" => {
                    td.underline = true;
                    any = true;
                }
                "overline" => {
                    td.overline = true;
                    any = true;
                }
                "line-through" => {
                    td.line_through = true;
                    any = true;
                }
                _ => {}
            }
        }
        if any {
            Some(td)
        } else {
            None
        }
    }

    pub fn to_svg(&self) -> String {
        let mut parts = Vec::new();
        if self.underline {
            parts.push("underline");
        }
        if self.overline {
            parts.push("overline");
        }
        if self.line_through {
            parts.push("line-through");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" ")
        }
    }

    pub fn is_none(&self) -> bool {
        !self.underline && !self.overline && !self.line_through
    }
}

// ---------------------------------------------------------------------------
// Orient (for markers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Orient {
    #[default]
    Auto,
    AutoStartReverse,
    Angle(f64),
}

impl Orient {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "auto" => Some(Orient::Auto),
            "auto-start-reverse" => Some(Orient::AutoStartReverse),
            other => other.parse::<f64>().ok().map(Orient::Angle),
        }
    }

    pub fn to_svg(&self) -> String {
        match self {
            Orient::Auto => "auto".to_string(),
            Orient::AutoStartReverse => "auto-start-reverse".to_string(),
            Orient::Angle(a) => a.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// PreserveAspectRatio
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    None,
    XMinYMin,
    XMidYMin,
    XMaxYMin,
    XMinYMid,
    XMidYMid,
    XMaxYMid,
    XMinYMax,
    XMidYMax,
    XMaxYMax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MeetOrSlice {
    #[default]
    Meet,
    Slice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreserveAspectRatio {
    pub align: Align,
    pub meet_or_slice: MeetOrSlice,
}

impl Default for PreserveAspectRatio {
    fn default() -> Self {
        Self {
            align: Align::XMidYMid,
            meet_or_slice: MeetOrSlice::Meet,
        }
    }
}

impl PreserveAspectRatio {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let align = match parts[0] {
            "none" => Align::None,
            "xMinYMin" => Align::XMinYMin,
            "xMidYMin" => Align::XMidYMin,
            "xMaxYMin" => Align::XMaxYMin,
            "xMinYMid" => Align::XMinYMid,
            "xMidYMid" => Align::XMidYMid,
            "xMaxYMid" => Align::XMaxYMid,
            "xMinYMax" => Align::XMinYMax,
            "xMidYMax" => Align::XMidYMax,
            "xMaxYMax" => Align::XMaxYMax,
            _ => return None,
        };

        let meet_or_slice = if parts.len() >= 2 {
            match parts[1] {
                "meet" => MeetOrSlice::Meet,
                "slice" => MeetOrSlice::Slice,
                _ => MeetOrSlice::Meet,
            }
        } else {
            MeetOrSlice::Meet
        };

        Some(PreserveAspectRatio {
            align,
            meet_or_slice,
        })
    }

    pub fn to_svg(&self) -> String {
        let align = match self.align {
            Align::None => "none",
            Align::XMinYMin => "xMinYMin",
            Align::XMidYMin => "xMidYMin",
            Align::XMaxYMin => "xMaxYMin",
            Align::XMinYMid => "xMinYMid",
            Align::XMidYMid => "xMidYMid",
            Align::XMaxYMid => "xMaxYMid",
            Align::XMinYMax => "xMinYMax",
            Align::XMidYMax => "xMidYMax",
            Align::XMaxYMax => "xMaxYMax",
        };
        let mos = match self.meet_or_slice {
            MeetOrSlice::Meet => "meet",
            MeetOrSlice::Slice => "slice",
        };
        format!("{} {}", align, mos)
    }
}

// ---------------------------------------------------------------------------
// FilterInput
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FilterInput {
    #[default]
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    Reference(String),
}

impl FilterInput {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "SourceGraphic" => FilterInput::SourceGraphic,
            "SourceAlpha" => FilterInput::SourceAlpha,
            "BackgroundImage" => FilterInput::BackgroundImage,
            "BackgroundAlpha" => FilterInput::BackgroundAlpha,
            "FillPaint" => FilterInput::FillPaint,
            "StrokePaint" => FilterInput::StrokePaint,
            other => FilterInput::Reference(other.to_string()),
        }
    }

    pub fn to_svg(&self) -> String {
        match self {
            FilterInput::SourceGraphic => "SourceGraphic".to_string(),
            FilterInput::SourceAlpha => "SourceAlpha".to_string(),
            FilterInput::BackgroundImage => "BackgroundImage".to_string(),
            FilterInput::BackgroundAlpha => "BackgroundAlpha".to_string(),
            FilterInput::FillPaint => "FillPaint".to_string(),
            FilterInput::StrokePaint => "StrokePaint".to_string(),
            FilterInput::Reference(s) => s.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// BlendMode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "normal" => Some(BlendMode::Normal),
            "multiply" => Some(BlendMode::Multiply),
            "screen" => Some(BlendMode::Screen),
            "overlay" => Some(BlendMode::Overlay),
            "darken" => Some(BlendMode::Darken),
            "lighten" => Some(BlendMode::Lighten),
            "color-dodge" => Some(BlendMode::ColorDodge),
            "color-burn" => Some(BlendMode::ColorBurn),
            "hard-light" => Some(BlendMode::HardLight),
            "soft-light" => Some(BlendMode::SoftLight),
            "difference" => Some(BlendMode::Difference),
            "exclusion" => Some(BlendMode::Exclusion),
            "hue" => Some(BlendMode::Hue),
            "saturation" => Some(BlendMode::Saturation),
            "color" => Some(BlendMode::Color),
            "luminosity" => Some(BlendMode::Luminosity),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::ColorDodge => "color-dodge",
            BlendMode::ColorBurn => "color-burn",
            BlendMode::HardLight => "hard-light",
            BlendMode::SoftLight => "soft-light",
            BlendMode::Difference => "difference",
            BlendMode::Exclusion => "exclusion",
            BlendMode::Hue => "hue",
            BlendMode::Saturation => "saturation",
            BlendMode::Color => "color",
            BlendMode::Luminosity => "luminosity",
        }
    }
}

// ---------------------------------------------------------------------------
// CompositeOperator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompositeOperator {
    #[default]
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic,
}

impl CompositeOperator {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "over" => Some(CompositeOperator::Over),
            "in" => Some(CompositeOperator::In),
            "out" => Some(CompositeOperator::Out),
            "atop" => Some(CompositeOperator::Atop),
            "xor" => Some(CompositeOperator::Xor),
            "arithmetic" => Some(CompositeOperator::Arithmetic),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            CompositeOperator::Over => "over",
            CompositeOperator::In => "in",
            CompositeOperator::Out => "out",
            CompositeOperator::Atop => "atop",
            CompositeOperator::Xor => "xor",
            CompositeOperator::Arithmetic => "arithmetic",
        }
    }
}

// ---------------------------------------------------------------------------
// ColorMatrixType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorMatrixType {
    #[default]
    Matrix,
    Saturate,
    HueRotate,
    LuminanceToAlpha,
}

impl ColorMatrixType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "matrix" => Some(ColorMatrixType::Matrix),
            "saturate" => Some(ColorMatrixType::Saturate),
            "hueRotate" => Some(ColorMatrixType::HueRotate),
            "luminanceToAlpha" => Some(ColorMatrixType::LuminanceToAlpha),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            ColorMatrixType::Matrix => "matrix",
            ColorMatrixType::Saturate => "saturate",
            ColorMatrixType::HueRotate => "hueRotate",
            ColorMatrixType::LuminanceToAlpha => "luminanceToAlpha",
        }
    }
}

// ---------------------------------------------------------------------------
// MorphologyOperator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MorphologyOperator {
    #[default]
    Erode,
    Dilate,
}

impl MorphologyOperator {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "erode" => Some(MorphologyOperator::Erode),
            "dilate" => Some(MorphologyOperator::Dilate),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            MorphologyOperator::Erode => "erode",
            MorphologyOperator::Dilate => "dilate",
        }
    }
}

// ---------------------------------------------------------------------------
// TurbulenceType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TurbulenceType {
    #[default]
    Turbulence,
    FractalNoise,
}

impl TurbulenceType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "turbulence" => Some(TurbulenceType::Turbulence),
            "fractalNoise" => Some(TurbulenceType::FractalNoise),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            TurbulenceType::Turbulence => "turbulence",
            TurbulenceType::FractalNoise => "fractalNoise",
        }
    }
}

// ---------------------------------------------------------------------------
// StitchTiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StitchTiles {
    #[default]
    NoStitch,
    Stitch,
}

impl StitchTiles {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "noStitch" => Some(StitchTiles::NoStitch),
            "stitch" => Some(StitchTiles::Stitch),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            StitchTiles::NoStitch => "noStitch",
            StitchTiles::Stitch => "stitch",
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelSelector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChannelSelector {
    #[default]
    R,
    G,
    B,
    A,
}

impl ChannelSelector {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "R" => Some(ChannelSelector::R),
            "G" => Some(ChannelSelector::G),
            "B" => Some(ChannelSelector::B),
            "A" => Some(ChannelSelector::A),
            _ => None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            ChannelSelector::R => "R",
            ChannelSelector::G => "G",
            ChannelSelector::B => "B",
            ChannelSelector::A => "A",
        }
    }
}

// ---------------------------------------------------------------------------
// EdgeMode (for feConvolveMatrix)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EdgeMode {
    #[default]
    Duplicate,
    Wrap,
    None,
}

impl EdgeMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "duplicate" => Some(EdgeMode::Duplicate),
            "wrap" => Some(EdgeMode::Wrap),
            "none" => Some(EdgeMode::None),
            _ => Option::None,
        }
    }

    pub fn to_svg(&self) -> &'static str {
        match self {
            EdgeMode::Duplicate => "duplicate",
            EdgeMode::Wrap => "wrap",
            EdgeMode::None => "none",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_length_bare_number() {
        let l: SvgLength = "42".parse().unwrap();
        assert_eq!(l.value, 42.0);
        assert_eq!(l.unit, LengthUnit::None);
    }

    #[test]
    fn parse_length_px() {
        let l: SvgLength = "10px".parse().unwrap();
        assert_eq!(l.value, 10.0);
        assert_eq!(l.unit, LengthUnit::Px);
    }

    #[test]
    fn parse_length_percent() {
        let l: SvgLength = "50%".parse().unwrap();
        assert_eq!(l.value, 50.0);
        assert_eq!(l.unit, LengthUnit::Percent);
    }

    #[test]
    fn parse_length_cm() {
        let l: SvgLength = "2.5cm".parse().unwrap();
        assert!((l.value - 2.5).abs() < 1e-9);
        assert_eq!(l.unit, LengthUnit::Cm);
    }

    #[test]
    fn parse_length_negative() {
        let l: SvgLength = "-3.5mm".parse().unwrap();
        assert!((l.value - (-3.5)).abs() < 1e-9);
        assert_eq!(l.unit, LengthUnit::Mm);
    }

    #[test]
    fn parse_length_em() {
        let l: SvgLength = "1.2em".parse().unwrap();
        assert!((l.value - 1.2).abs() < 1e-9);
        assert_eq!(l.unit, LengthUnit::Em);
    }

    #[test]
    fn length_display_roundtrip() {
        let l = SvgLength::new(12.5, LengthUnit::Px);
        let s = l.to_string();
        let l2: SvgLength = s.parse().unwrap();
        assert_eq!(l, l2);
    }

    #[test]
    fn color_hex_6() {
        let c = Color::from_hex("#ff8000").unwrap();
        assert_eq!(c, Color::rgb(255, 128, 0));
    }

    #[test]
    fn color_hex_3() {
        let c = Color::from_hex("#f80").unwrap();
        assert_eq!(c, Color::rgb(255, 136, 0));
    }

    #[test]
    fn color_named() {
        assert_eq!(Color::from_named("red"), Some(Color::RED));
        assert_eq!(Color::from_named("Blue"), Some(Color::BLUE));
        assert_eq!(Color::from_named("transparent"), Some(Color::TRANSPARENT));
    }

    #[test]
    fn color_to_hex() {
        assert_eq!(Color::RED.to_hex(), "#ff0000");
        assert_eq!(Color::TRANSPARENT.to_hex(), "#00000000");
    }

    #[test]
    fn paint_parse_none() {
        assert_eq!(Paint::parse("none"), Some(Paint::None));
    }

    #[test]
    fn paint_parse_color() {
        assert_eq!(Paint::parse("#ff0000"), Some(Paint::Color(Color::RED)));
    }

    #[test]
    fn paint_parse_url() {
        assert_eq!(
            Paint::parse("url(#grad1)"),
            Some(Paint::Url("#grad1".to_string()))
        );
    }

    #[test]
    fn viewbox_parse() {
        let vb = ViewBox::parse("0 0 100 200").unwrap();
        assert_eq!(vb.min_x, 0.0);
        assert_eq!(vb.width, 100.0);
        assert_eq!(vb.height, 200.0);
    }

    #[test]
    fn viewbox_roundtrip() {
        let vb = ViewBox::new(10.0, 20.0, 300.0, 400.0);
        let vb2 = ViewBox::parse(&vb.to_svg()).unwrap();
        assert_eq!(vb, vb2);
    }
}
