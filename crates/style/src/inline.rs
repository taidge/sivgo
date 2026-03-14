//! Inline `style` attribute parser.
//!
//! Parses CSS declarations from inline `style="..."` attribute values
//! into a property name -> value map.

use std::collections::HashMap;

/// Parse an inline `style` attribute value into property name/value pairs.
///
/// Input:  `"fill: red; stroke: blue; opacity: 0.5"`
/// Output: `{ "fill" => "red", "stroke" => "blue", "opacity" => "0.5" }`
///
/// Empty or whitespace-only declarations are silently skipped.
/// Malformed declarations (no colon) are also skipped.
pub fn parse_inline_style(style: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for decl in style.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }

        // Split on the first colon only (values may contain colons, e.g. url(...))
        if let Some(colon_pos) = decl.find(':') {
            let property = decl[..colon_pos].trim().to_ascii_lowercase();
            let value = decl[colon_pos + 1..].trim().to_string();

            if !property.is_empty() && !value.is_empty() {
                map.insert(property, value);
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let result = parse_inline_style("fill: red; stroke: blue; opacity: 0.5");
        assert_eq!(result.get("fill"), Some(&"red".to_string()));
        assert_eq!(result.get("stroke"), Some(&"blue".to_string()));
        assert_eq!(result.get("opacity"), Some(&"0.5".to_string()));
    }

    #[test]
    fn parse_trailing_semicolon() {
        let result = parse_inline_style("fill: red;");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("fill"), Some(&"red".to_string()));
    }

    #[test]
    fn parse_empty() {
        let result = parse_inline_style("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_whitespace_only() {
        let result = parse_inline_style("   ;  ;  ");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_no_colon_skipped() {
        let result = parse_inline_style("fill red; stroke: blue");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("stroke"), Some(&"blue".to_string()));
    }

    #[test]
    fn parse_url_value_with_colon() {
        // url values don't contain colons typically, but test that only first colon splits
        let result = parse_inline_style("fill: url(#gradient)");
        assert_eq!(result.get("fill"), Some(&"url(#gradient)".to_string()));
    }

    #[test]
    fn parse_extra_whitespace() {
        let result = parse_inline_style("  fill :  #ff0000  ;  stroke-width :  2px  ");
        assert_eq!(result.get("fill"), Some(&"#ff0000".to_string()));
        assert_eq!(result.get("stroke-width"), Some(&"2px".to_string()));
    }

    #[test]
    fn parse_case_insensitive_property() {
        let result = parse_inline_style("Fill: red; STROKE: blue");
        assert_eq!(result.get("fill"), Some(&"red".to_string()));
        assert_eq!(result.get("stroke"), Some(&"blue".to_string()));
    }

    #[test]
    fn parse_overwrites_duplicates() {
        let result = parse_inline_style("fill: red; fill: blue");
        assert_eq!(result.get("fill"), Some(&"blue".to_string()));
    }
}
