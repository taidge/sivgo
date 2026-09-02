//! Turn `<style>` element rules into per-node presentation overrides.
//!
//! The renderer reads `PresentationAttrs` directly rather than a computed
//! style, and the parser already folds an element's inline `style` attribute
//! into those attributes. What is left over is the document's stylesheet:
//! rules in `<style>` elements, matched by tag, id, class or combinator.
//!
//! This module resolves those rules into a per-node overlay the renderer can
//! consult, which keeps the CSS precedence order intact:
//!
//! 1. presentation attributes (lowest)
//! 2. `<style>` element rules, by specificity then source order
//! 3. the element's own inline `style` attribute (highest)
//!
//! Because the parser has already merged (1) and (3) together, the raw `style`
//! attribute is re-read here to find which properties are inline and must not
//! be overridden.

use std::collections::HashMap;

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument, SvgElement};

use crate::cascade::apply_declaration_to_presentation;
use crate::inline::parse_inline_style;
use crate::stylesheet::{parse_stylesheet, Specificity, Stylesheet};

/// Presentation values contributed by the document's stylesheet, per node.
///
/// Only nodes that some rule matches appear here, and each entry holds only
/// the properties those rules set.
pub type StyleOverrides = HashMap<NodeId, PresentationAttrs>;

/// Resolve every `<style>` element in `doc` into per-node overrides.
///
/// Returns an empty map when the document has no stylesheet, which is the
/// common case and costs one tree walk.
pub fn stylesheet_overrides(doc: &SvgDocument) -> StyleOverrides {
    let stylesheet = collect_stylesheet(doc);
    if stylesheet.rules.is_empty() {
        return StyleOverrides::new();
    }

    let mut overrides = StyleOverrides::new();
    let Some(root) = doc.root() else {
        return overrides;
    };

    for node_id in doc.traverse_pre_order(root) {
        if let Some(attrs) = node_overrides(doc, node_id, &stylesheet) {
            overrides.insert(node_id, attrs);
        }
    }
    overrides
}

/// Gather the CSS text of every `<style>` element and parse it as one sheet.
fn collect_stylesheet(doc: &SvgDocument) -> Stylesheet {
    let Some(root) = doc.root() else {
        return Stylesheet::default();
    };

    let mut css = String::new();
    for node_id in doc.traverse_pre_order(root) {
        if let Some(node) = doc.get_node(node_id) {
            if let SvgElement::StyleElement(attrs) = &node.element {
                // A `type` other than CSS belongs to some other language.
                let is_css = attrs
                    .style_type
                    .as_deref()
                    .map(|t| t.trim().eq_ignore_ascii_case("text/css"))
                    .unwrap_or(true);
                if is_css {
                    css.push_str(&attrs.content);
                    css.push('\n');
                }
            }
        }
    }

    if css.trim().is_empty() {
        Stylesheet::default()
    } else {
        parse_stylesheet(&css)
    }
}

/// Build the override set for one node, or `None` when no rule contributes.
fn node_overrides(
    doc: &SvgDocument,
    node_id: NodeId,
    stylesheet: &Stylesheet,
) -> Option<PresentationAttrs> {
    let node = doc.get_node(node_id)?;

    let tag_name = node.element.tag_name();
    let id = node.xml_id.as_deref();
    let class_attr = node
        .attributes
        .get("class")
        .map(String::as_str)
        .unwrap_or("");
    let classes: Vec<&str> = class_attr.split_ascii_whitespace().collect();

    let mut matching: Vec<(Specificity, usize, &HashMap<String, String>)> = Vec::new();
    for (idx, rule) in stylesheet.rules.iter().enumerate() {
        let is_match = if rule.selector.has_combinators() {
            rule.selector.matches_in_context(doc, node_id)
        } else {
            rule.selector.matches(tag_name, id, &classes)
        };
        if is_match {
            matching.push((rule.selector.specificity(), idx, &rule.declarations));
        }
    }
    if matching.is_empty() {
        return None;
    }

    // Weakest first, so the strongest rule is applied last and wins.
    matching.sort_by_key(|(spec, idx, _)| (*spec, *idx));

    // Anything the element sets inline outranks every rule.
    let inline = node
        .attributes
        .get("style")
        .map(|s| parse_inline_style(s))
        .unwrap_or_default();

    let mut attrs = PresentationAttrs::default();
    let mut any_applied = false;
    for (_, _, declarations) in matching {
        // Sorted so the result does not depend on hash iteration order.
        let mut properties: Vec<(&String, &String)> = declarations.iter().collect();
        properties.sort_by(|a, b| a.0.cmp(b.0));
        for (property, value) in properties {
            if inline.contains_key(property) {
                continue;
            }
            if apply_declaration_to_presentation(&mut attrs, property, value) {
                any_applied = true;
            }
        }
    }

    any_applied.then_some(attrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::parse_svg;

    fn overrides_for(svg: &str) -> (SvgDocument, StyleOverrides) {
        let doc = parse_svg(svg).expect("parse");
        let overrides = stylesheet_overrides(&doc);
        (doc, overrides)
    }

    /// Find the first node whose tag matches.
    fn find(doc: &SvgDocument, tag: &str) -> NodeId {
        let root = doc.root().expect("root");
        doc.traverse_pre_order(root)
            .into_iter()
            .find(|id| {
                doc.get_node(*id)
                    .is_some_and(|n| n.element.tag_name() == tag)
            })
            .expect("element not found")
    }

    fn fill_rgb(attrs: &PresentationAttrs) -> Option<(u8, u8, u8)> {
        match attrs.fill {
            Some(sivgo_dom::Paint::Color(c)) => Some((c.r, c.g, c.b)),
            _ => None,
        }
    }

    #[test]
    fn a_class_rule_produces_an_override() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.a { fill: #00ff00 }</style><rect class=\"a\"/></svg>",
        );
        let rect = find(&doc, "rect");
        let attrs = overrides.get(&rect).expect("rect should be matched");
        assert_eq!(fill_rgb(attrs), Some((0, 255, 0)));
    }

    #[test]
    fn an_inline_style_is_not_overridden() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.a { fill: #00ff00 }</style><rect class=\"a\" style=\"fill:#ff0000\"/></svg>",
        );
        let rect = find(&doc, "rect");
        assert!(
            overrides.get(&rect).is_none_or(|a| a.fill.is_none()),
            "an inline declaration must win over the stylesheet"
        );
    }

    #[test]
    fn a_more_specific_rule_wins() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>rect { fill: #ff0000 } #target { fill: #00ff00 }</style><rect id=\"target\"/></svg>",
        );
        let rect = find(&doc, "rect");
        let attrs = overrides.get(&rect).expect("rect should be matched");
        assert_eq!(fill_rgb(attrs), Some((0, 255, 0)), "the id rule should win");
    }

    #[test]
    fn later_rules_win_at_equal_specificity() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>rect { fill: #ff0000 } rect { fill: #00ff00 }</style><rect/></svg>",
        );
        let rect = find(&doc, "rect");
        let attrs = overrides.get(&rect).expect("rect should be matched");
        assert_eq!(fill_rgb(attrs), Some((0, 255, 0)));
    }

    #[test]
    fn a_document_without_a_stylesheet_produces_nothing() {
        let (_, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect fill=\"#ff0000\"/></svg>",
        );
        assert!(overrides.is_empty());
    }

    #[test]
    fn unmatched_elements_are_absent() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.a { fill: #00ff00 }</style><rect class=\"b\"/></svg>",
        );
        let rect = find(&doc, "rect");
        assert!(!overrides.contains_key(&rect));
    }

    #[test]
    fn descendant_selectors_are_matched() {
        let (doc, overrides) = overrides_for(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>g rect { fill: #00ff00 }</style><g><rect/></g></svg>",
        );
        let rect = find(&doc, "rect");
        let attrs = overrides.get(&rect).expect("rect should be matched");
        assert_eq!(fill_rgb(attrs), Some((0, 255, 0)));
    }
}
