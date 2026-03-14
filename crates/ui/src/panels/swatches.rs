//! Swatches panel data model (11.4.13).
//!
//! Colour palettes and recently-used colours for quick access.

/// An RGBA colour stored as four `u8` components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new opaque colour.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a new colour with alpha.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Return the colour as a CSS hex string (e.g. `"#ff0000"`).
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb(0, 0, 0)
    }
}

/// A single named colour in a swatch palette.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchColor {
    /// The colour value.
    pub color: Color,
    /// Optional human-readable name.
    pub name: Option<String>,
}

impl SwatchColor {
    /// Create a named swatch colour.
    pub fn named(color: Color, name: impl Into<String>) -> Self {
        Self {
            color,
            name: Some(name.into()),
        }
    }

    /// Create an unnamed swatch colour.
    pub fn unnamed(color: Color) -> Self {
        Self { color, name: None }
    }
}

/// A palette of swatch colours.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchPalette {
    /// Palette display name.
    pub name: String,
    /// Ordered list of colours in the palette.
    pub colors: Vec<SwatchColor>,
}

impl SwatchPalette {
    /// Create a new empty palette.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: Vec::new(),
        }
    }
}

/// State for the Swatches panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchesPanel {
    /// Available colour palettes.
    pub palettes: Vec<SwatchPalette>,
    /// Index of the active palette.
    pub active_palette_index: usize,
    /// Recently used colours (most recent first).
    pub recent_colors: Vec<Color>,
}

impl Default for SwatchesPanel {
    fn default() -> Self {
        // Provide a minimal default palette.
        let mut default_palette = SwatchPalette::new("Default");
        default_palette
            .colors
            .push(SwatchColor::named(Color::rgb(0, 0, 0), "Black"));
        default_palette
            .colors
            .push(SwatchColor::named(Color::rgb(255, 255, 255), "White"));
        default_palette
            .colors
            .push(SwatchColor::named(Color::rgb(255, 0, 0), "Red"));
        default_palette
            .colors
            .push(SwatchColor::named(Color::rgb(0, 255, 0), "Green"));
        default_palette
            .colors
            .push(SwatchColor::named(Color::rgb(0, 0, 255), "Blue"));
        Self {
            palettes: vec![default_palette],
            active_palette_index: 0,
            recent_colors: Vec::new(),
        }
    }
}

/// Maximum number of recent colours to keep.
const MAX_RECENT_COLORS: usize = 32;

impl SwatchesPanel {
    /// Select a palette by index.  Returns `true` if the index was valid.
    pub fn select_palette(&mut self, index: usize) -> bool {
        if index < self.palettes.len() {
            self.active_palette_index = index;
            true
        } else {
            false
        }
    }

    /// Add a colour to the recent-colours list.  If the colour already exists
    /// it is moved to the front.
    pub fn add_recent(&mut self, color: Color) {
        self.recent_colors.retain(|c| *c != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(MAX_RECENT_COLORS);
    }

    /// Return a reference to the currently active palette, if any.
    pub fn get_active_palette(&self) -> Option<&SwatchPalette> {
        self.palettes.get(self.active_palette_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_hex_opaque() {
        let c = Color::rgb(255, 128, 0);
        assert_eq!(c.to_hex(), "#ff8000");
    }

    #[test]
    fn color_hex_with_alpha() {
        let c = Color::rgba(255, 0, 0, 128);
        assert_eq!(c.to_hex(), "#ff000080");
    }

    #[test]
    fn default_palette() {
        let panel = SwatchesPanel::default();
        assert_eq!(panel.palettes.len(), 1);
        assert_eq!(panel.palettes[0].name, "Default");
        assert_eq!(panel.palettes[0].colors.len(), 5);
    }

    #[test]
    fn select_palette_valid() {
        let mut panel = SwatchesPanel::default();
        panel.palettes.push(SwatchPalette::new("Custom"));
        assert!(panel.select_palette(1));
        assert_eq!(panel.active_palette_index, 1);
    }

    #[test]
    fn select_palette_invalid() {
        let mut panel = SwatchesPanel::default();
        assert!(!panel.select_palette(99));
        assert_eq!(panel.active_palette_index, 0);
    }

    #[test]
    fn add_recent_deduplicates() {
        let mut panel = SwatchesPanel::default();
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        panel.add_recent(red);
        panel.add_recent(blue);
        panel.add_recent(red); // move red to front
        assert_eq!(panel.recent_colors.len(), 2);
        assert_eq!(panel.recent_colors[0], red);
        assert_eq!(panel.recent_colors[1], blue);
    }

    #[test]
    fn add_recent_caps_at_max() {
        let mut panel = SwatchesPanel::default();
        for i in 0..50u8 {
            panel.add_recent(Color::rgb(i, i, i));
        }
        assert_eq!(panel.recent_colors.len(), MAX_RECENT_COLORS);
    }

    #[test]
    fn get_active_palette() {
        let panel = SwatchesPanel::default();
        let active = panel.get_active_palette().unwrap();
        assert_eq!(active.name, "Default");
    }

    #[test]
    fn swatch_color_named() {
        let sc = SwatchColor::named(Color::rgb(0, 0, 0), "Black");
        assert_eq!(sc.name, Some("Black".to_string()));
    }

    #[test]
    fn swatch_color_unnamed() {
        let sc = SwatchColor::unnamed(Color::rgb(0, 0, 0));
        assert_eq!(sc.name, None);
    }
}
