//! CSS stylesheet parser and selector matching.
//!
//! Parses `<style>` element content into CSS rules and matches selectors
//! against SVG elements. Supports simple selectors (element, class, id,
//! universal) and combinator selectors (descendant, child, adjacent
//! sibling, general sibling).

use std::collections::HashMap;

use sivgo_dom::{NodeId, SvgDocument};

/// A parsed CSS rule: selector + declarations.
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: CssSelector,
    pub declarations: HashMap<String, String>,
}

/// A CSS selector, supporting simple, compound, and combinator selectors.
#[derive(Debug, Clone)]
pub enum CssSelector {
    /// `*` — matches any element.
    Universal,
    /// `rect`, `circle`, etc. — matches by element tag name.
    Element(String),
    /// `.foo` — matches by class name.
    Class(String),
    /// `#foo` — matches by id.
    Id(String),
    /// Compound selector: `rect.foo#bar` — must match ALL parts.
    Compound(Vec<CssSelector>),
    /// Descendant combinator: `A B` — B is a descendant of A.
    Descendant(Box<CssSelector>, Box<CssSelector>),
    /// Child combinator: `A > B` — B is a direct child of A.
    Child(Box<CssSelector>, Box<CssSelector>),
    /// Adjacent sibling: `A + B` — B immediately follows A.
    AdjacentSibling(Box<CssSelector>, Box<CssSelector>),
    /// General sibling: `A ~ B` — B follows A (not necessarily immediately).
    GeneralSibling(Box<CssSelector>, Box<CssSelector>),
    /// `:first-child` pseudo-class.
    FirstChild,
    /// `:last-child` pseudo-class.
    LastChild,
}

/// Specificity as (id_count, class_count, element_count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl CssSelector {
    /// Compute the specificity of this selector.
    pub fn specificity(&self) -> Specificity {
        match self {
            CssSelector::Universal => Specificity(0, 0, 0),
            CssSelector::Element(_) => Specificity(0, 0, 1),
            CssSelector::Class(_) => Specificity(0, 1, 0),
            CssSelector::Id(_) => Specificity(1, 0, 0),
            CssSelector::Compound(parts) => {
                let mut a = 0u32;
                let mut b = 0u32;
                let mut c = 0u32;
                for part in parts {
                    let Specificity(pa, pb, pc) = part.specificity();
                    a += pa;
                    b += pb;
                    c += pc;
                }
                Specificity(a, b, c)
            }
            CssSelector::FirstChild | CssSelector::LastChild => Specificity(0, 1, 0),
            CssSelector::Descendant(parent, child)
            | CssSelector::Child(parent, child)
            | CssSelector::AdjacentSibling(parent, child)
            | CssSelector::GeneralSibling(parent, child) => {
                let Specificity(a1, b1, c1) = parent.specificity();
                let Specificity(a2, b2, c2) = child.specificity();
                Specificity(a1 + a2, b1 + b2, c1 + c2)
            }
        }
    }

    /// Check if this selector matches the given element (simple selectors only).
    ///
    /// For combinator selectors, use [`matches_in_context`] instead.
    pub fn matches(&self, tag_name: &str, id: Option<&str>, classes: &[&str]) -> bool {
        match self {
            CssSelector::Universal => true,
            CssSelector::Element(name) => tag_name == name,
            CssSelector::Class(cls) => classes.contains(&cls.as_str()),
            CssSelector::Id(sel_id) => id == Some(sel_id.as_str()),
            CssSelector::Compound(parts) => parts.iter().all(|p| p.matches(tag_name, id, classes)),
            // Pseudo-classes and combinator selectors need document context.
            CssSelector::FirstChild
            | CssSelector::LastChild
            | CssSelector::Descendant(..)
            | CssSelector::Child(..)
            | CssSelector::AdjacentSibling(..)
            | CssSelector::GeneralSibling(..) => false,
        }
    }

    /// Check if this selector matches a node in the document tree.
    ///
    /// This handles both simple and combinator selectors by walking the
    /// tree to check ancestor/sibling relationships.
    pub fn matches_in_context(&self, doc: &SvgDocument, node_id: NodeId) -> bool {
        let Some(node) = doc.get_node(node_id) else {
            return false;
        };
        let tag_name = node.element.tag_name();
        let id = node.xml_id.as_deref();
        let class_attr = node
            .attributes
            .get("class")
            .map(|s| s.as_str())
            .unwrap_or("");
        let classes: Vec<&str> = class_attr
            .split_ascii_whitespace()
            .filter(|s| !s.is_empty())
            .collect();

        self.matches_node(doc, node_id, tag_name, id, &classes)
    }

    /// Internal: match against a node given its pre-extracted properties.
    fn matches_node(
        &self,
        doc: &SvgDocument,
        node_id: NodeId,
        tag_name: &str,
        id: Option<&str>,
        classes: &[&str],
    ) -> bool {
        match self {
            CssSelector::Universal => true,
            CssSelector::Element(name) => tag_name == name,
            CssSelector::Class(cls) => classes.contains(&cls.as_str()),
            CssSelector::Id(sel_id) => id == Some(sel_id.as_str()),
            CssSelector::Compound(parts) => parts
                .iter()
                .all(|p| p.matches_node(doc, node_id, tag_name, id, classes)),
            CssSelector::FirstChild => {
                if let Some(parent_id) = doc.parent(node_id) {
                    let siblings = doc.children(parent_id);
                    !siblings.is_empty() && siblings[0] == node_id
                } else {
                    true // Root is trivially "first child"
                }
            }
            CssSelector::LastChild => {
                if let Some(parent_id) = doc.parent(node_id) {
                    let siblings = doc.children(parent_id);
                    !siblings.is_empty() && *siblings.last().unwrap() == node_id
                } else {
                    true
                }
            }
            CssSelector::Descendant(ancestor_sel, self_sel) => {
                // Self must match the current node.
                if !self_sel.matches_node(doc, node_id, tag_name, id, classes) {
                    return false;
                }
                // Some ancestor must match ancestor_sel.
                let mut current = doc.parent(node_id);
                while let Some(anc_id) = current {
                    if ancestor_sel.matches_in_context(doc, anc_id) {
                        return true;
                    }
                    current = doc.parent(anc_id);
                }
                false
            }
            CssSelector::Child(parent_sel, self_sel) => {
                // Self must match the current node.
                if !self_sel.matches_node(doc, node_id, tag_name, id, classes) {
                    return false;
                }
                // Direct parent must match parent_sel.
                if let Some(parent_id) = doc.parent(node_id) {
                    parent_sel.matches_in_context(doc, parent_id)
                } else {
                    false
                }
            }
            CssSelector::AdjacentSibling(prev_sel, self_sel) => {
                // Self must match the current node.
                if !self_sel.matches_node(doc, node_id, tag_name, id, classes) {
                    return false;
                }
                // The immediately preceding sibling must match prev_sel.
                if let Some(prev_id) = prev_sibling(doc, node_id) {
                    prev_sel.matches_in_context(doc, prev_id)
                } else {
                    false
                }
            }
            CssSelector::GeneralSibling(prev_sel, self_sel) => {
                // Self must match the current node.
                if !self_sel.matches_node(doc, node_id, tag_name, id, classes) {
                    return false;
                }
                // Any preceding sibling must match prev_sel.
                if let Some(parent_id) = doc.parent(node_id) {
                    let siblings = doc.children(parent_id);
                    for &sib_id in siblings {
                        if sib_id == node_id {
                            break; // Only look at preceding siblings.
                        }
                        if prev_sel.matches_in_context(doc, sib_id) {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    /// Returns true if this selector requires document context (combinators or pseudo-classes).
    pub fn has_combinators(&self) -> bool {
        match self {
            CssSelector::Descendant(..)
            | CssSelector::Child(..)
            | CssSelector::AdjacentSibling(..)
            | CssSelector::GeneralSibling(..)
            | CssSelector::FirstChild
            | CssSelector::LastChild => true,
            CssSelector::Compound(parts) => parts.iter().any(|p| p.has_combinators()),
            _ => false,
        }
    }
}

/// Get the immediately preceding sibling of a node.
fn prev_sibling(doc: &SvgDocument, node_id: NodeId) -> Option<NodeId> {
    let parent_id = doc.parent(node_id)?;
    let siblings = doc.children(parent_id);
    let mut prev = None;
    for &sib_id in siblings {
        if sib_id == node_id {
            return prev;
        }
        prev = Some(sib_id);
    }
    None
}

/// A parsed stylesheet (collection of rules).
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<CssRule>,
}

/// Parse CSS text (from a `<style>` element) into a [`Stylesheet`].
///
/// Supports simple rule syntax: `selector { property: value; ... }`
/// Supports comma-separated selectors: `rect, circle { fill: red }`
/// Ignores `@` rules (media queries, imports, etc.) and comments.
pub fn parse_stylesheet(css: &str) -> Stylesheet {
    let css = strip_comments(css);
    let mut rules = Vec::new();

    let mut chars = css.chars().peekable();
    let mut buf = String::new();

    while chars.peek().is_some() {
        buf.clear();

        // Read until '{' or end
        while let Some(&ch) = chars.peek() {
            if ch == '{' {
                chars.next();
                break;
            }
            buf.push(ch);
            chars.next();
        }

        let selector_text = buf.trim().to_string();
        if selector_text.is_empty() || selector_text.starts_with('@') {
            // Skip @-rules: consume until matching '}'
            let mut depth = 1i32;
            for ch in chars.by_ref() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth <= 0 {
                        break;
                    }
                }
            }
            continue;
        }

        // Read declarations until '}'
        buf.clear();
        while let Some(&ch) = chars.peek() {
            if ch == '}' {
                chars.next();
                break;
            }
            buf.push(ch);
            chars.next();
        }

        let declarations = parse_declarations(&buf);
        if declarations.is_empty() {
            continue;
        }

        // Parse comma-separated selectors
        for sel_text in selector_text.split(',') {
            let sel_text = sel_text.trim();
            if sel_text.is_empty() {
                continue;
            }
            if let Some(selector) = parse_selector(sel_text) {
                rules.push(CssRule {
                    selector,
                    declarations: declarations.clone(),
                });
            }
        }
    }

    Stylesheet { rules }
}

/// Parse a selector string, including combinator selectors.
///
/// Simple/compound: `*`, `rect`, `.class`, `#id`, `rect.class`, `rect#id.class`
/// Combinators: `g rect` (descendant), `g > rect` (child),
///              `rect + circle` (adjacent sibling), `rect ~ circle` (general sibling)
fn parse_selector(s: &str) -> Option<CssSelector> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Tokenize into (simple/compound selector, combinator) pairs.
    // We split the input into segments by detecting combinator boundaries.
    let segments = tokenize_selector(s);
    if segments.is_empty() {
        return None;
    }

    // Build selector tree left-to-right.
    let mut iter = segments.into_iter();
    let (first_text, _) = iter.next()?;
    let mut result = parse_simple_selector(&first_text)?;

    for (seg_text, combinator) in iter {
        let right = parse_simple_selector(&seg_text)?;
        result = match combinator {
            Combinator::Descendant => CssSelector::Descendant(Box::new(result), Box::new(right)),
            Combinator::Child => CssSelector::Child(Box::new(result), Box::new(right)),
            Combinator::AdjacentSibling => {
                CssSelector::AdjacentSibling(Box::new(result), Box::new(right))
            }
            Combinator::GeneralSibling => {
                CssSelector::GeneralSibling(Box::new(result), Box::new(right))
            }
        };
    }

    Some(result)
}

/// Combinator between selector segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Combinator {
    Descendant,      // whitespace
    Child,           // >
    AdjacentSibling, // +
    GeneralSibling,  // ~
}

/// Tokenize a selector string into segments.
/// Returns `Vec<(segment_text, combinator_before_this_segment)>`.
/// The first segment's combinator is `Descendant` (unused placeholder).
fn tokenize_selector(s: &str) -> Vec<(String, Combinator)> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut combinator = Combinator::Descendant; // placeholder for first
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'>' || ch == b'+' || ch == b'~' {
            // Explicit combinator
            let seg = current.trim().to_string();
            if !seg.is_empty() {
                segments.push((seg, combinator));
            }
            current.clear();
            combinator = match ch {
                b'>' => Combinator::Child,
                b'+' => Combinator::AdjacentSibling,
                b'~' => Combinator::GeneralSibling,
                _ => unreachable!(),
            };
            i += 1;
        } else if ch.is_ascii_whitespace() {
            // Whitespace: might be descendant combinator or just padding around > + ~
            // Skip all whitespace, then check if the next non-ws char is a combinator.
            let seg_so_far = current.trim().to_string();
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                // End of string — push what we have.
                if !seg_so_far.is_empty() {
                    current = seg_so_far;
                }
                break;
            }
            let next = bytes[i];
            if next == b'>' || next == b'+' || next == b'~' {
                // The whitespace was just padding; the real combinator is next.
                // Don't push yet — let the next iteration handle it.
                if !seg_so_far.is_empty() {
                    current = seg_so_far;
                }
                continue;
            }
            // Otherwise, this whitespace IS the descendant combinator.
            if !seg_so_far.is_empty() {
                segments.push((seg_so_far, combinator));
                current.clear();
                combinator = Combinator::Descendant;
            }
        } else {
            current.push(ch as char);
            i += 1;
        }
    }

    let seg = current.trim().to_string();
    if !seg.is_empty() {
        segments.push((seg, combinator));
    }

    segments
}

/// Parse a simple or compound selector (no combinators — just element, class, id, *, pseudo-classes).
fn parse_simple_selector(s: &str) -> Option<CssSelector> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < bytes.len() {
        match bytes[i] {
            b'*' => {
                parts.push(CssSelector::Universal);
                i += 1;
            }
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len()
                    && bytes[i] != b'.'
                    && bytes[i] != b'#'
                    && bytes[i] != b':'
                    && !bytes[i].is_ascii_whitespace()
                {
                    i += 1;
                }
                if i > start {
                    parts.push(CssSelector::Class(s[start..i].to_string()));
                }
            }
            b'#' => {
                i += 1;
                let start = i;
                while i < bytes.len()
                    && bytes[i] != b'.'
                    && bytes[i] != b'#'
                    && bytes[i] != b':'
                    && !bytes[i].is_ascii_whitespace()
                {
                    i += 1;
                }
                if i > start {
                    parts.push(CssSelector::Id(s[start..i].to_string()));
                }
            }
            b':' => {
                // Pseudo-class
                i += 1;
                let start = i;
                while i < bytes.len()
                    && bytes[i] != b'.'
                    && bytes[i] != b'#'
                    && bytes[i] != b':'
                    && bytes[i] != b'('
                    && !bytes[i].is_ascii_whitespace()
                {
                    i += 1;
                }
                let pseudo_name = &s[start..i];
                match pseudo_name {
                    "first-child" => parts.push(CssSelector::FirstChild),
                    "last-child" => parts.push(CssSelector::LastChild),
                    _ => {
                        // Skip unsupported pseudo-classes (including parenthesized args)
                        if i < bytes.len() && bytes[i] == b'(' {
                            let mut depth = 1;
                            i += 1;
                            while i < bytes.len() && depth > 0 {
                                if bytes[i] == b'(' {
                                    depth += 1;
                                } else if bytes[i] == b')' {
                                    depth -= 1;
                                }
                                i += 1;
                            }
                        }
                        // Unsupported pseudo-class — ignore.
                    }
                }
            }
            _ => {
                let start = i;
                while i < bytes.len()
                    && bytes[i] != b'.'
                    && bytes[i] != b'#'
                    && bytes[i] != b':'
                    && !bytes[i].is_ascii_whitespace()
                {
                    i += 1;
                }
                if i > start {
                    parts.push(CssSelector::Element(s[start..i].to_string()));
                }
            }
        }
    }

    match parts.len() {
        0 => None,
        1 => Some(parts.into_iter().next().unwrap()),
        _ => Some(CssSelector::Compound(parts)),
    }
}

/// Parse CSS declarations from a block body (between `{` and `}`).
fn parse_declarations(block: &str) -> HashMap<String, String> {
    crate::inline::parse_inline_style(block)
}

/// Strip CSS comments (`/* ... */`) from the input.
fn strip_comments(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            // Start of comment — skip until */
            chars.next(); // consume '*'
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next(); // consume '/'
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            continue;
        }
        result.push(ch);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_element_selector() {
        let sel = parse_selector("rect").unwrap();
        assert!(sel.matches("rect", None, &[]));
        assert!(!sel.matches("circle", None, &[]));
    }

    #[test]
    fn parse_class_selector() {
        let sel = parse_selector(".highlight").unwrap();
        assert!(sel.matches("rect", None, &["highlight"]));
        assert!(!sel.matches("rect", None, &["other"]));
    }

    #[test]
    fn parse_id_selector() {
        let sel = parse_selector("#my-id").unwrap();
        assert!(sel.matches("rect", Some("my-id"), &[]));
        assert!(!sel.matches("rect", Some("other"), &[]));
        assert!(!sel.matches("rect", None, &[]));
    }

    #[test]
    fn parse_universal_selector() {
        let sel = parse_selector("*").unwrap();
        assert!(sel.matches("rect", None, &[]));
        assert!(sel.matches("circle", Some("x"), &["a"]));
    }

    #[test]
    fn parse_compound_selector() {
        let sel = parse_selector("rect.highlight").unwrap();
        assert!(sel.matches("rect", None, &["highlight"]));
        assert!(!sel.matches("circle", None, &["highlight"]));
        assert!(!sel.matches("rect", None, &["other"]));
    }

    #[test]
    fn parse_compound_element_id_class() {
        let sel = parse_selector("rect#foo.bar").unwrap();
        assert!(sel.matches("rect", Some("foo"), &["bar"]));
        assert!(!sel.matches("rect", Some("foo"), &[]));
        assert!(!sel.matches("circle", Some("foo"), &["bar"]));
    }

    #[test]
    fn specificity_ordering() {
        assert!(
            CssSelector::Id("x".into()).specificity()
                > CssSelector::Class("x".into()).specificity()
        );
        assert!(
            CssSelector::Class("x".into()).specificity()
                > CssSelector::Element("x".into()).specificity()
        );
        assert!(
            CssSelector::Element("x".into()).specificity() > CssSelector::Universal.specificity()
        );
    }

    #[test]
    fn parse_stylesheet_basic() {
        let css = r#"
            rect { fill: red; stroke: blue }
            .highlight { opacity: 0.5 }
            #special { fill: green }
        "#;

        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 3);

        assert!(sheet.rules[0].selector.matches("rect", None, &[]));
        assert_eq!(
            sheet.rules[0].declarations.get("fill"),
            Some(&"red".to_string())
        );

        assert!(sheet.rules[1].selector.matches("any", None, &["highlight"]));
        assert_eq!(
            sheet.rules[1].declarations.get("opacity"),
            Some(&"0.5".to_string())
        );

        assert!(sheet.rules[2].selector.matches("any", Some("special"), &[]));
    }

    #[test]
    fn parse_stylesheet_comma_selectors() {
        let css = "rect, circle { fill: red }";
        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);
        assert!(sheet.rules[0].selector.matches("rect", None, &[]));
        assert!(sheet.rules[1].selector.matches("circle", None, &[]));
    }

    #[test]
    fn parse_stylesheet_with_comments() {
        let css = r#"
            /* This is a comment */
            rect { fill: red }
            /* Another comment */
            circle { stroke: blue }
        "#;

        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);
    }

    #[test]
    fn parse_stylesheet_skips_at_rules() {
        let css = r#"
            @media screen { .foo { fill: red } }
            rect { fill: blue }
        "#;

        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        assert!(sheet.rules[0].selector.matches("rect", None, &[]));
    }

    #[test]
    fn strip_comments_works() {
        assert_eq!(strip_comments("a /* b */ c"), "a  c");
        assert_eq!(strip_comments("/* start */ text /* end */"), " text ");
        assert_eq!(strip_comments("no comments"), "no comments");
    }

    #[test]
    fn empty_stylesheet() {
        let sheet = parse_stylesheet("");
        assert!(sheet.rules.is_empty());
    }

    #[test]
    fn compound_specificity() {
        let sel = parse_selector("rect.foo#bar").unwrap();
        // id(1) + class(1) + element(1) = (1, 1, 1)
        assert_eq!(sel.specificity(), Specificity(1, 1, 1));
    }

    // --- Combinator selector parsing ---

    #[test]
    fn parse_descendant_selector() {
        let sel = parse_selector("g rect").unwrap();
        assert!(matches!(sel, CssSelector::Descendant(..)));
        assert_eq!(sel.specificity(), Specificity(0, 0, 2));
    }

    #[test]
    fn parse_child_selector() {
        let sel = parse_selector("g > rect").unwrap();
        assert!(matches!(sel, CssSelector::Child(..)));
        assert_eq!(sel.specificity(), Specificity(0, 0, 2));
    }

    #[test]
    fn parse_adjacent_sibling_selector() {
        let sel = parse_selector("rect + circle").unwrap();
        assert!(matches!(sel, CssSelector::AdjacentSibling(..)));
        assert_eq!(sel.specificity(), Specificity(0, 0, 2));
    }

    #[test]
    fn parse_general_sibling_selector() {
        let sel = parse_selector("rect ~ circle").unwrap();
        assert!(matches!(sel, CssSelector::GeneralSibling(..)));
        assert_eq!(sel.specificity(), Specificity(0, 0, 2));
    }

    #[test]
    fn parse_child_selector_no_spaces() {
        let sel = parse_selector("g>rect").unwrap();
        assert!(matches!(sel, CssSelector::Child(..)));
    }

    #[test]
    fn parse_complex_combinator_selector() {
        // "svg > g rect.highlight" = Child(Element("svg"), Descendant(Element("g"), Compound([Element("rect"), Class("highlight")])))
        let sel = parse_selector("svg > g rect.highlight").unwrap();
        // specificity: svg(0,0,1) + g(0,0,1) + rect(0,0,1) + .highlight(0,1,0) = (0,1,3)
        assert_eq!(sel.specificity(), Specificity(0, 1, 3));
    }

    // --- Combinator selector matching with document context ---

    #[test]
    fn descendant_selector_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g>
                <rect x="0" y="0" width="10" height="10"/>
            </g>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let sel = parse_selector("g rect").unwrap();
        assert!(sel.matches_in_context(&doc, rect_id));
        // rect is not a descendant match for "g rect" when checking g itself
        assert!(!sel.matches_in_context(&doc, g_id));
    }

    #[test]
    fn child_selector_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g>
                <g>
                    <rect x="0" y="0" width="10" height="10"/>
                </g>
            </g>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let g1 = doc.children(root)[0];
        let g2 = doc.children(g1)[0];
        let rect_id = doc.children(g2)[0];

        // "g > rect" should match rect (direct child of g2)
        let sel_child = parse_selector("g > rect").unwrap();
        assert!(sel_child.matches_in_context(&doc, rect_id));

        // "svg > rect" should NOT match (rect's parent is g, not svg)
        let sel_not = parse_selector("svg > rect").unwrap();
        assert!(!sel_not.matches_in_context(&doc, rect_id));
    }

    #[test]
    fn adjacent_sibling_selector_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
            <ellipse cx="10" cy="10" rx="5" ry="3"/>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let _rect_id = children[0];
        let circle_id = children[1];
        let ellipse_id = children[2];

        // "rect + circle" should match circle (immediately after rect)
        let sel = parse_selector("rect + circle").unwrap();
        assert!(sel.matches_in_context(&doc, circle_id));

        // "rect + ellipse" should NOT match (ellipse is not immediately after rect)
        let sel2 = parse_selector("rect + ellipse").unwrap();
        assert!(!sel2.matches_in_context(&doc, ellipse_id));
    }

    #[test]
    fn general_sibling_selector_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
            <ellipse cx="10" cy="10" rx="5" ry="3"/>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let ellipse_id = children[2];

        // "rect ~ ellipse" should match (ellipse follows rect, even with circle between)
        let sel = parse_selector("rect ~ ellipse").unwrap();
        assert!(sel.matches_in_context(&doc, ellipse_id));
    }

    #[test]
    fn deep_descendant_selector() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g>
                <g>
                    <rect x="0" y="0" width="10" height="10"/>
                </g>
            </g>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let g1 = doc.children(root)[0];
        let g2 = doc.children(g1)[0];
        let rect_id = doc.children(g2)[0];

        // "svg rect" should match (rect is a descendant of svg, through two g's)
        let sel = parse_selector("svg rect").unwrap();
        assert!(sel.matches_in_context(&doc, rect_id));
    }

    #[test]
    fn has_combinators_flag() {
        assert!(!parse_selector("rect").unwrap().has_combinators());
        assert!(!parse_selector(".foo").unwrap().has_combinators());
        assert!(!parse_selector("rect.foo#bar").unwrap().has_combinators());
        assert!(parse_selector("g rect").unwrap().has_combinators());
        assert!(parse_selector("g > rect").unwrap().has_combinators());
        assert!(parse_selector("a + b").unwrap().has_combinators());
        assert!(parse_selector("a ~ b").unwrap().has_combinators());
        assert!(parse_selector(":first-child").unwrap().has_combinators());
        assert!(parse_selector("rect:last-child").unwrap().has_combinators());
    }

    // --- Pseudo-class parsing ---

    #[test]
    fn parse_first_child_selector() {
        let sel = parse_selector(":first-child").unwrap();
        assert!(matches!(sel, CssSelector::FirstChild));
        assert_eq!(sel.specificity(), Specificity(0, 1, 0));
    }

    #[test]
    fn parse_last_child_selector() {
        let sel = parse_selector(":last-child").unwrap();
        assert!(matches!(sel, CssSelector::LastChild));
    }

    #[test]
    fn parse_element_with_pseudo_class() {
        let sel = parse_selector("rect:first-child").unwrap();
        assert!(matches!(sel, CssSelector::Compound(..)));
        // element(0,0,1) + :first-child(0,1,0) = (0,1,1)
        assert_eq!(sel.specificity(), Specificity(0, 1, 1));
    }

    // --- Pseudo-class matching ---

    #[test]
    fn first_child_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let rect_id = children[0];
        let circle_id = children[1];

        let sel = parse_selector(":first-child").unwrap();
        assert!(sel.matches_in_context(&doc, rect_id));
        assert!(!sel.matches_in_context(&doc, circle_id));
    }

    #[test]
    fn last_child_matches() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let rect_id = children[0];
        let circle_id = children[1];

        let sel = parse_selector(":last-child").unwrap();
        assert!(!sel.matches_in_context(&doc, rect_id));
        assert!(sel.matches_in_context(&doc, circle_id));
    }

    #[test]
    fn element_first_child_compound() {
        use sivgo_dom::parse_svg;

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
        </svg>"#;
        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let rect_id = children[0];
        let circle_id = children[1];

        // "rect:first-child" matches rect (first child and a rect)
        let sel = parse_selector("rect:first-child").unwrap();
        assert!(sel.matches_in_context(&doc, rect_id));

        // "circle:first-child" doesn't match circle (circle is second child)
        let sel2 = parse_selector("circle:first-child").unwrap();
        assert!(!sel2.matches_in_context(&doc, circle_id));

        // "circle:last-child" matches circle
        let sel3 = parse_selector("circle:last-child").unwrap();
        assert!(sel3.matches_in_context(&doc, circle_id));
    }
}
