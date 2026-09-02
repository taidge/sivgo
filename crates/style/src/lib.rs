//! `sivgo-style` -- CSS cascade and computed style resolution for SVG elements.
//!
//! This crate resolves the final "computed" value for every CSS/SVG
//! presentation property on each element in an [`sivgo_dom::SvgDocument`],
//! considering:
//!
//! 1. Inherited values from the parent element
//! 2. Presentation attributes on the element (`fill="red"`)
//! 3. Inline `style` attribute (`style="fill: red"`) -- higher priority
//!
//! # Quick start
//!
//! ```no_run
//! use sivgo_dom::parse_svg;
//! use sivgo_style::resolve_styles;
//!
//! let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
//!   <g fill="red">
//!     <rect x="0" y="0" width="100" height="100"/>
//!   </g>
//! </svg>"#;
//!
//! let doc = parse_svg(svg).unwrap();
//! let styles = resolve_styles(&doc);
//! // The rect's computed fill is red (inherited from the group).
//! ```

pub mod cache;
pub mod cascade;
pub mod computed;
pub mod inline;
pub mod overrides;
pub mod properties;
pub mod stylesheet;

// Re-export the primary API at the crate root.
pub use cache::StyleCache;
pub use cascade::{
    apply_declaration_to_presentation, apply_inline_style, apply_presentation_attrs,
    resolve_single, resolve_styles,
};
pub use computed::ComputedStyle;
pub use inline::parse_inline_style;
pub use overrides::{stylesheet_overrides, StyleOverrides};
pub use properties::{is_inherited, is_non_inherited};
pub use stylesheet::{parse_stylesheet, CssRule, CssSelector, Specificity, Stylesheet};
