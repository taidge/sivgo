//! Color palette system for Sivgo.
//!
//! Provides [`ColorPalette`] for storing named collections of colors,
//! a parser for GIMP `.gpl` palette files, and a set of built-in palettes
//! (SVG Named Colors and Material Design primary colors).

use crate::color::ColorF64;

/// A named collection of colors.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /// Human-readable palette name.
    pub name: String,
    /// Named colors in this palette: `(label, color)`.
    pub colors: Vec<(String, ColorF64)>,
}

impl ColorPalette {
    /// Create an empty palette with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: Vec::new(),
        }
    }

    /// Add a named color to the palette.
    pub fn push(&mut self, label: impl Into<String>, color: ColorF64) {
        self.colors.push((label.into(), color));
    }

    /// Look up a color by its label (case-insensitive).
    pub fn find(&self, label: &str) -> Option<&ColorF64> {
        let lower = label.to_lowercase();
        self.colors
            .iter()
            .find(|(l, _)| l.to_lowercase() == lower)
            .map(|(_, c)| c)
    }

    /// Number of colors in the palette.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Whether the palette is empty.
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GIMP .gpl palette parser
// ---------------------------------------------------------------------------

/// Parse a GIMP `.gpl` palette file.
///
/// The format is:
/// ```text
/// GIMP Palette
/// Name: My Palette
/// Columns: 16
/// #
/// 255 128   0  Orange
///   0   0   0  Black
/// ```
///
/// Lines starting with `#` are comments.  Each color line has three
/// whitespace-separated integers (R, G, B in 0-255) optionally followed
/// by a name.
///
/// Returns `None` if the data does not begin with `GIMP Palette`.
pub fn parse_gpl(data: &str) -> Option<ColorPalette> {
    let mut lines = data.lines();

    // First line must be "GIMP Palette"
    let header = lines.next()?.trim();
    if header != "GIMP Palette" {
        return None;
    }

    let mut name = String::from("Untitled");
    let mut colors: Vec<(String, ColorF64)> = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for metadata lines.
        if let Some(rest) = trimmed.strip_prefix("Name:") {
            name = rest.trim().to_string();
            continue;
        }
        if trimmed.starts_with("Columns:") {
            continue;
        }

        // Parse a color line: R G B [name]
        let mut parts = trimmed.split_whitespace();
        let r: u8 = parts.next()?.parse().ok()?;
        let g: u8 = parts.next()?.parse().ok()?;
        let b: u8 = parts.next()?.parse().ok()?;

        // Everything after the three numbers is the color name.
        let label: String = parts.collect::<Vec<&str>>().join(" ");
        let label = if label.is_empty() {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            label
        };

        colors.push((label, ColorF64::srgb_u8(r, g, b, 255)));
    }

    Some(ColorPalette { name, colors })
}

// ---------------------------------------------------------------------------
// Built-in palettes
// ---------------------------------------------------------------------------

/// Return a set of built-in palettes.
///
/// Currently includes:
/// - **SVG Named Colors** -- the 147 named colors from the SVG/CSS specification.
/// - **Material Design** -- the primary color swatches from Material Design.
pub fn builtin_palettes() -> Vec<ColorPalette> {
    vec![svg_named_colors(), material_design()]
}

/// The 147 SVG/CSS named colors.
fn svg_named_colors() -> ColorPalette {
    let mut p = ColorPalette::new("SVG Named Colors");
    // Sorted alphabetically - the full set from the CSS Color Level 4 spec.
    let entries: &[(&str, u8, u8, u8)] = &[
        ("aliceblue", 240, 248, 255),
        ("antiquewhite", 250, 235, 215),
        ("aqua", 0, 255, 255),
        ("aquamarine", 127, 255, 212),
        ("azure", 240, 255, 255),
        ("beige", 245, 245, 220),
        ("bisque", 255, 228, 196),
        ("black", 0, 0, 0),
        ("blanchedalmond", 255, 235, 205),
        ("blue", 0, 0, 255),
        ("blueviolet", 138, 43, 226),
        ("brown", 165, 42, 42),
        ("burlywood", 222, 184, 135),
        ("cadetblue", 95, 158, 160),
        ("chartreuse", 127, 255, 0),
        ("chocolate", 210, 105, 30),
        ("coral", 255, 127, 80),
        ("cornflowerblue", 100, 149, 237),
        ("cornsilk", 255, 248, 220),
        ("crimson", 220, 20, 60),
        ("cyan", 0, 255, 255),
        ("darkblue", 0, 0, 139),
        ("darkcyan", 0, 139, 139),
        ("darkgoldenrod", 184, 134, 11),
        ("darkgray", 169, 169, 169),
        ("darkgreen", 0, 100, 0),
        ("darkgrey", 169, 169, 169),
        ("darkkhaki", 189, 183, 107),
        ("darkmagenta", 139, 0, 139),
        ("darkolivegreen", 85, 107, 47),
        ("darkorange", 255, 140, 0),
        ("darkorchid", 153, 50, 204),
        ("darkred", 139, 0, 0),
        ("darksalmon", 233, 150, 122),
        ("darkseagreen", 143, 188, 143),
        ("darkslateblue", 72, 61, 139),
        ("darkslategray", 47, 79, 79),
        ("darkslategrey", 47, 79, 79),
        ("darkturquoise", 0, 206, 209),
        ("darkviolet", 148, 0, 211),
        ("deeppink", 255, 20, 147),
        ("deepskyblue", 0, 191, 255),
        ("dimgray", 105, 105, 105),
        ("dimgrey", 105, 105, 105),
        ("dodgerblue", 30, 144, 255),
        ("firebrick", 178, 34, 34),
        ("floralwhite", 255, 250, 240),
        ("forestgreen", 34, 139, 34),
        ("fuchsia", 255, 0, 255),
        ("gainsboro", 220, 220, 220),
        ("ghostwhite", 248, 248, 255),
        ("gold", 255, 215, 0),
        ("goldenrod", 218, 165, 32),
        ("gray", 128, 128, 128),
        ("green", 0, 128, 0),
        ("greenyellow", 173, 255, 47),
        ("grey", 128, 128, 128),
        ("honeydew", 240, 255, 240),
        ("hotpink", 255, 105, 180),
        ("indianred", 205, 92, 92),
        ("indigo", 75, 0, 130),
        ("ivory", 255, 255, 240),
        ("khaki", 240, 230, 140),
        ("lavender", 230, 230, 250),
        ("lavenderblush", 255, 240, 245),
        ("lawngreen", 124, 252, 0),
        ("lemonchiffon", 255, 250, 205),
        ("lightblue", 173, 216, 230),
        ("lightcoral", 240, 128, 128),
        ("lightcyan", 224, 255, 255),
        ("lightgoldenrodyellow", 250, 250, 210),
        ("lightgray", 211, 211, 211),
        ("lightgreen", 144, 238, 144),
        ("lightgrey", 211, 211, 211),
        ("lightpink", 255, 182, 193),
        ("lightsalmon", 255, 160, 122),
        ("lightseagreen", 32, 178, 170),
        ("lightskyblue", 135, 206, 250),
        ("lightslategray", 119, 136, 153),
        ("lightslategrey", 119, 136, 153),
        ("lightsteelblue", 176, 196, 222),
        ("lightyellow", 255, 255, 224),
        ("lime", 0, 255, 0),
        ("limegreen", 50, 205, 50),
        ("linen", 250, 240, 230),
        ("magenta", 255, 0, 255),
        ("maroon", 128, 0, 0),
        ("mediumaquamarine", 102, 205, 170),
        ("mediumblue", 0, 0, 205),
        ("mediumorchid", 186, 85, 211),
        ("mediumpurple", 147, 112, 219),
        ("mediumseagreen", 60, 179, 113),
        ("mediumslateblue", 123, 104, 238),
        ("mediumspringgreen", 0, 250, 154),
        ("mediumturquoise", 72, 209, 204),
        ("mediumvioletred", 199, 21, 133),
        ("midnightblue", 25, 25, 112),
        ("mintcream", 245, 255, 250),
        ("mistyrose", 255, 228, 225),
        ("moccasin", 255, 228, 181),
        ("navajowhite", 255, 222, 173),
        ("navy", 0, 0, 128),
        ("oldlace", 253, 245, 230),
        ("olive", 128, 128, 0),
        ("olivedrab", 107, 142, 35),
        ("orange", 255, 165, 0),
        ("orangered", 255, 69, 0),
        ("orchid", 218, 112, 214),
        ("palegoldenrod", 238, 232, 170),
        ("palegreen", 152, 251, 152),
        ("paleturquoise", 175, 238, 238),
        ("palevioletred", 219, 112, 147),
        ("papayawhip", 255, 239, 213),
        ("peachpuff", 255, 218, 185),
        ("peru", 205, 133, 63),
        ("pink", 255, 192, 203),
        ("plum", 221, 160, 221),
        ("powderblue", 176, 224, 230),
        ("purple", 128, 0, 128),
        ("rebeccapurple", 102, 51, 153),
        ("red", 255, 0, 0),
        ("rosybrown", 188, 143, 143),
        ("royalblue", 65, 105, 225),
        ("saddlebrown", 139, 69, 19),
        ("salmon", 250, 128, 114),
        ("sandybrown", 244, 164, 96),
        ("seagreen", 46, 139, 87),
        ("seashell", 255, 245, 238),
        ("sienna", 160, 82, 45),
        ("silver", 192, 192, 192),
        ("skyblue", 135, 206, 235),
        ("slateblue", 106, 90, 205),
        ("slategray", 112, 128, 144),
        ("slategrey", 112, 128, 144),
        ("snow", 255, 250, 250),
        ("springgreen", 0, 255, 127),
        ("steelblue", 70, 130, 180),
        ("tan", 210, 180, 140),
        ("teal", 0, 128, 128),
        ("thistle", 216, 191, 216),
        ("tomato", 255, 99, 71),
        ("turquoise", 64, 224, 208),
        ("violet", 238, 130, 238),
        ("wheat", 245, 222, 179),
        ("white", 255, 255, 255),
        ("whitesmoke", 245, 245, 245),
        ("yellow", 255, 255, 0),
        ("yellowgreen", 154, 205, 50),
    ];
    for &(name, r, g, b) in entries {
        p.push(name, ColorF64::srgb_u8(r, g, b, 255));
    }
    p
}

/// Material Design primary colors (500 shade for each hue).
fn material_design() -> ColorPalette {
    let mut p = ColorPalette::new("Material Design");
    let entries: &[(&str, u8, u8, u8)] = &[
        ("Red", 244, 67, 54),
        ("Pink", 233, 30, 99),
        ("Purple", 156, 39, 176),
        ("Deep Purple", 103, 58, 183),
        ("Indigo", 63, 81, 181),
        ("Blue", 33, 150, 243),
        ("Light Blue", 3, 169, 244),
        ("Cyan", 0, 188, 212),
        ("Teal", 0, 150, 136),
        ("Green", 76, 175, 80),
        ("Light Green", 139, 195, 74),
        ("Lime", 205, 220, 57),
        ("Yellow", 255, 235, 59),
        ("Amber", 255, 193, 7),
        ("Orange", 255, 152, 0),
        ("Deep Orange", 255, 87, 34),
        ("Brown", 121, 85, 72),
        ("Grey", 158, 158, 158),
        ("Blue Grey", 96, 125, 139),
    ];
    for &(name, r, g, b) in entries {
        p.push(name, ColorF64::srgb_u8(r, g, b, 255));
    }
    p
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gpl_basic() {
        let data = "\
GIMP Palette
Name: Test Palette
Columns: 8
#
255   0   0\tRed
  0 255   0\tGreen
  0   0 255\tBlue
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.name, "Test Palette");
        assert_eq!(pal.len(), 3);
        assert_eq!(pal.colors[0].0, "Red");
        assert_eq!(pal.colors[1].0, "Green");
        assert_eq!(pal.colors[2].0, "Blue");

        // Verify RGB values
        let red = &pal.colors[0].1;
        assert!((red.c0() - 1.0).abs() < 1e-10);
        assert!((red.c1() - 0.0).abs() < 1e-10);
        assert!((red.c2() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn parse_gpl_no_name() {
        let data = "\
GIMP Palette
#
128 128 128\tGray
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.name, "Untitled");
        assert_eq!(pal.len(), 1);
    }

    #[test]
    fn parse_gpl_unnamed_color() {
        let data = "\
GIMP Palette
Name: NoNames
#
255 128   0
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.colors[0].0, "#FF8000");
    }

    #[test]
    fn parse_gpl_invalid_header() {
        let data = "Not a palette\n255 0 0 Red";
        assert!(parse_gpl(data).is_none());
    }

    #[test]
    fn parse_gpl_empty() {
        let data = "\
GIMP Palette
Name: Empty
#
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.name, "Empty");
        assert!(pal.is_empty());
    }

    #[test]
    fn parse_gpl_comments_skipped() {
        let data = "\
GIMP Palette
Name: Comments
# This is a comment
255   0   0\tRed
# Another comment
  0 255   0\tGreen
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.len(), 2);
    }

    #[test]
    fn palette_find() {
        let mut pal = ColorPalette::new("Test");
        pal.push("Red", ColorF64::srgb(1.0, 0.0, 0.0, 1.0));
        pal.push("Green", ColorF64::srgb(0.0, 1.0, 0.0, 1.0));

        assert!(pal.find("Red").is_some());
        assert!(pal.find("red").is_some()); // case-insensitive
        assert!(pal.find("RED").is_some());
        assert!(pal.find("Blue").is_none());
    }

    #[test]
    fn palette_new_and_push() {
        let mut pal = ColorPalette::new("Custom");
        assert!(pal.is_empty());
        assert_eq!(pal.len(), 0);

        pal.push("Black", ColorF64::srgb(0.0, 0.0, 0.0, 1.0));
        assert!(!pal.is_empty());
        assert_eq!(pal.len(), 1);
    }

    #[test]
    fn builtin_palettes_exist() {
        let palettes = builtin_palettes();
        assert_eq!(palettes.len(), 2);

        let svg_pal = &palettes[0];
        assert_eq!(svg_pal.name, "SVG Named Colors");
        assert_eq!(svg_pal.len(), 148);

        // Spot-check a few colors
        let red = svg_pal.find("red").unwrap();
        assert!((red.c0() - 1.0).abs() < 1e-10);
        assert!((red.c1() - 0.0).abs() < 1e-10);
        assert!((red.c2() - 0.0).abs() < 1e-10);

        let dodger = svg_pal.find("dodgerblue").unwrap();
        assert!((dodger.c0() - 30.0 / 255.0).abs() < 1e-10);
        assert!((dodger.c1() - 144.0 / 255.0).abs() < 1e-10);
        assert!((dodger.c2() - 1.0).abs() < 1e-10);

        let material = &palettes[1];
        assert_eq!(material.name, "Material Design");
        assert_eq!(material.len(), 19);

        let mat_red = material.find("Red").unwrap();
        assert_eq!(mat_red.space, crate::color::ColorSpace::Srgb);
    }

    #[test]
    fn svg_palette_color_count() {
        let pal = svg_named_colors();
        // CSS Color Level 4 defines 148 named colors (the SVG 1.1 set plus
        // rebeccapurple).  aqua==cyan and fuchsia==magenta are duplicates
        // by value but distinct by name.
        assert_eq!(pal.len(), 148);
    }

    #[test]
    fn parse_gpl_multi_word_name() {
        let data = "\
GIMP Palette
Name: My Awesome Palette
#
100 200  50\tLight Olive Green
";
        let pal = parse_gpl(data).unwrap();
        assert_eq!(pal.name, "My Awesome Palette");
        assert_eq!(pal.colors[0].0, "Light Olive Green");
    }
}
