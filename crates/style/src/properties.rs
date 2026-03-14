//! CSS/SVG property definitions and inheritance rules.

/// Whether a CSS/SVG presentation property is inherited by default.
///
/// Inherited properties propagate from parent to child unless explicitly
/// overridden. Non-inherited properties reset to their initial value on
/// each element unless explicitly set.
pub fn is_inherited(property: &str) -> bool {
    matches!(
        property,
        "color"
            | "fill"
            | "fill-rule"
            | "fill-opacity"
            | "stroke"
            | "stroke-width"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-opacity"
            | "clip-rule"
            | "visibility"
            | "font-family"
            | "font-size"
            | "font-weight"
            | "font-style"
            | "text-anchor"
            | "letter-spacing"
            | "word-spacing"
            | "direction"
            | "writing-mode"
            | "dominant-baseline"
    )
}

/// Non-inherited properties use their initial (default) value on each element
/// unless explicitly set. These include:
///   opacity, display, transform, clip-path, mask, filter,
///   marker-start, marker-mid, marker-end, stop-color, stop-opacity,
///   mix-blend-mode, etc.
pub fn is_non_inherited(property: &str) -> bool {
    !is_inherited(property)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_is_inherited() {
        assert!(is_inherited("fill"));
        assert!(is_inherited("stroke"));
        assert!(is_inherited("font-size"));
        assert!(is_inherited("visibility"));
    }

    #[test]
    fn opacity_is_not_inherited() {
        assert!(!is_inherited("opacity"));
        assert!(!is_inherited("display"));
        assert!(!is_inherited("transform"));
        assert!(!is_inherited("clip-path"));
        assert!(!is_inherited("mask"));
        assert!(!is_inherited("filter"));
        assert!(!is_inherited("mix-blend-mode"));
    }
}
