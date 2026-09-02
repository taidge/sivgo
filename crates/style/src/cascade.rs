//! CSS cascade resolution: walk the SVG tree and resolve computed styles.
//!
//! The cascade considers (in increasing priority):
//! 1. Inherited values from parent
//! 2. Stylesheet rules (from `<style>` elements, sorted by specificity)
//! 3. Presentation attributes on the element
//! 4. Inline `style` attribute on the element

use std::collections::HashMap;

use sivgo_dom::{
    BlendMode, Color, Display, FillRule, FontStyle, FontWeight, LineCap, LineJoin, NodeId, Paint,
    PresentationAttrs, SvgDocument, SvgElement, SvgLength, TextAnchor, TextDecoration, Visibility,
};
use sivgo_math::parse_transform;

use crate::computed::ComputedStyle;
use crate::inline::parse_inline_style;
use crate::stylesheet::{parse_stylesheet, Stylesheet};

/// Resolve computed styles for all nodes in the document.
///
/// Returns a `HashMap<NodeId, ComputedStyle>` mapping every node to its
/// fully resolved computed style, with inheritance and cascade applied.
pub fn resolve_styles(doc: &SvgDocument) -> HashMap<NodeId, ComputedStyle> {
    // Collect and parse all <style> elements in the document.
    let stylesheet = collect_stylesheets(doc);

    let mut styles = HashMap::new();
    if let Some(root) = doc.root() {
        resolve_recursive(
            doc,
            root,
            &ComputedStyle::default(),
            &stylesheet,
            &mut styles,
        );
    }
    styles
}

/// Collect CSS text from all `<style>` elements in the document and parse
/// them into a single stylesheet.
fn collect_stylesheets(doc: &SvgDocument) -> Stylesheet {
    let mut css_text = String::new();
    if let Some(root) = doc.root() {
        collect_style_elements(doc, root, &mut css_text);
    }
    if css_text.is_empty() {
        Stylesheet::default()
    } else {
        parse_stylesheet(&css_text)
    }
}

/// Recursively collect CSS text from `<style>` elements.
fn collect_style_elements(doc: &SvgDocument, node_id: NodeId, css_text: &mut String) {
    let Some(node) = doc.get_node(node_id) else {
        return;
    };
    if let SvgElement::StyleElement(ref attrs) = node.element {
        if !attrs.content.is_empty() {
            css_text.push_str(&attrs.content);
            css_text.push('\n');
        }
    }
    let children: Vec<NodeId> = doc.children(node_id).to_vec();
    for child_id in children {
        collect_style_elements(doc, child_id, css_text);
    }
}

/// Recursively resolve styles from `node_id` downward.
fn resolve_recursive(
    doc: &SvgDocument,
    node_id: NodeId,
    parent_style: &ComputedStyle,
    stylesheet: &Stylesheet,
    styles: &mut HashMap<NodeId, ComputedStyle>,
) {
    let node = match doc.get_node(node_id) {
        Some(n) => n,
        None => return,
    };

    // 1. Start with inherited values from parent
    let mut computed = ComputedStyle::inherit_from(parent_style);

    // 2. Apply matching stylesheet rules (sorted by specificity, lowest first)
    if !stylesheet.rules.is_empty() {
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

        // Collect matching rules with their specificity and original order
        let mut matching: Vec<(
            crate::stylesheet::Specificity,
            usize,
            &HashMap<String, String>,
        )> = Vec::new();
        for (idx, rule) in stylesheet.rules.iter().enumerate() {
            let is_match = if rule.selector.has_combinators() {
                // Combinator selectors need document context.
                rule.selector.matches_in_context(doc, node_id)
            } else {
                rule.selector.matches(tag_name, id, &classes)
            };
            if is_match {
                matching.push((rule.selector.specificity(), idx, &rule.declarations));
            }
        }
        // Sort by (specificity, source order) — stable sort preserves source order for equal specificity
        matching.sort_by_key(|(spec, idx, _)| (*spec, *idx));

        for (_, _, declarations) in &matching {
            apply_inline_style_with_parent(&mut computed, declarations, Some(parent_style));
        }
    }

    // 3. Apply presentation attributes (higher priority than stylesheet rules)
    apply_presentation_attrs(&mut computed, &node.presentation);

    // 4. Apply inline style attribute (highest priority)
    if let Some(style_str) = node.attributes.get("style") {
        let inline = parse_inline_style(style_str);
        apply_inline_style_with_parent(&mut computed, &inline, Some(parent_style));
    }

    // 5. Resolve currentColor: replace Paint::CurrentColor with the computed color value
    resolve_current_color(&mut computed);

    styles.insert(node_id, computed.clone());

    // Recurse into children
    let children: Vec<NodeId> = doc.children(node_id).to_vec();
    for child_id in children {
        resolve_recursive(doc, child_id, &computed, stylesheet, styles);
    }
}

/// Apply typed presentation attributes onto a computed style.
///
/// Each `Some(...)` field in `PresentationAttrs` overrides the corresponding
/// computed value (which may have been inherited from the parent).
pub fn apply_presentation_attrs(computed: &mut ComputedStyle, pres: &PresentationAttrs) {
    if let Some(color) = pres.color {
        computed.color = color;
    }
    if let Some(ref fill) = pres.fill {
        computed.fill = fill.clone();
    }
    if let Some(fill_opacity) = pres.fill_opacity {
        computed.fill_opacity = fill_opacity;
    }
    if let Some(fill_rule) = pres.fill_rule {
        computed.fill_rule = fill_rule;
    }
    if let Some(ref stroke) = pres.stroke {
        computed.stroke = stroke.clone();
    }
    if let Some(stroke_opacity) = pres.stroke_opacity {
        computed.stroke_opacity = stroke_opacity;
    }
    if let Some(ref stroke_width) = pres.stroke_width {
        // Use context-aware resolution: em/ex reference the computed font-size,
        // % references the computed font-size as fallback (spec: viewport diagonal / sqrt(2)).
        computed.stroke_width = stroke_width.resolve_px(computed.font_size, computed.font_size);
    }
    if let Some(stroke_linecap) = pres.stroke_linecap {
        computed.stroke_linecap = stroke_linecap;
    }
    if let Some(stroke_linejoin) = pres.stroke_linejoin {
        computed.stroke_linejoin = stroke_linejoin;
    }
    if let Some(stroke_miterlimit) = pres.stroke_miterlimit {
        computed.stroke_miterlimit = stroke_miterlimit;
    }
    if let Some(ref stroke_dasharray) = pres.stroke_dasharray {
        computed.stroke_dasharray = Some(stroke_dasharray.clone());
    }
    if let Some(stroke_dashoffset) = pres.stroke_dashoffset {
        computed.stroke_dashoffset = stroke_dashoffset;
    }
    if let Some(opacity) = pres.opacity {
        computed.opacity = opacity;
    }
    if let Some(display) = pres.display {
        computed.display = display;
    }
    if let Some(visibility) = pres.visibility {
        computed.visibility = visibility;
    }
    if let Some(transform) = pres.transform {
        computed.transform = transform;
    }
    if let Some(clip_rule) = pres.clip_rule {
        computed.clip_rule = clip_rule;
    }
    // Clipping, masking, filter references
    if let Some(ref clip_path) = pres.clip_path {
        computed.clip_path = Some(clip_path.clone());
    }
    if let Some(ref mask) = pres.mask {
        computed.mask = Some(mask.clone());
    }
    if let Some(ref filter) = pres.filter {
        computed.filter = Some(filter.clone());
    }
    // Marker references
    if let Some(ref marker_start) = pres.marker_start {
        computed.marker_start = Some(marker_start.clone());
    }
    if let Some(ref marker_mid) = pres.marker_mid {
        computed.marker_mid = Some(marker_mid.clone());
    }
    if let Some(ref marker_end) = pres.marker_end {
        computed.marker_end = Some(marker_end.clone());
    }
    // Blend mode
    if let Some(mix_blend_mode) = pres.mix_blend_mode {
        computed.mix_blend_mode = mix_blend_mode;
    }
    // Font / text properties — resolve font-size FIRST so other lengths can use it
    if let Some(ref font_size) = pres.font_size {
        computed.font_size = font_size.resolve_px(computed.font_size, computed.font_size);
    }
    if let Some(ref font_family) = pres.font_family {
        computed.font_family = font_family.clone();
    }
    if let Some(font_weight) = pres.font_weight {
        computed.font_weight = font_weight;
    }
    if let Some(font_style) = pres.font_style {
        computed.font_style = font_style;
    }
    if let Some(text_anchor) = pres.text_anchor {
        computed.text_anchor = text_anchor;
    }
    if let Some(ref letter_spacing) = pres.letter_spacing {
        computed.letter_spacing = letter_spacing.resolve_px(computed.font_size, computed.font_size);
    }
    if let Some(ref word_spacing) = pres.word_spacing {
        computed.word_spacing = word_spacing.resolve_px(computed.font_size, computed.font_size);
    }
}

/// Apply inline CSS style declarations to a computed style.
///
/// This handles parsing of string property values into typed values.
/// Inline styles have higher specificity than presentation attributes.
/// `parent_style` is needed for `inherit` keyword resolution.
pub fn apply_inline_style(computed: &mut ComputedStyle, inline: &HashMap<String, String>) {
    apply_inline_style_with_parent(computed, inline, None);
}

/// Apply inline CSS with optional parent reference for `inherit`/`initial`/`unset` handling.
pub fn apply_inline_style_with_parent(
    computed: &mut ComputedStyle,
    inline: &HashMap<String, String>,
    parent_style: Option<&ComputedStyle>,
) {
    // Process font-size FIRST so that em/ex units in other length properties
    // (stroke-width, letter-spacing, etc.) can reference the correct font-size.
    if let Some(value) = inline.get("font-size") {
        apply_keyword_or_property(computed, "font-size", value, parent_style);
    }

    for (property, value) in inline {
        if property == "font-size" {
            continue; // Already handled above
        }
        apply_keyword_or_property(computed, property, value, parent_style);
    }
}

/// Handle CSS-wide keywords (inherit/initial/unset) or delegate to property application.
fn apply_keyword_or_property(
    computed: &mut ComputedStyle,
    property: &str,
    value: &str,
    parent_style: Option<&ComputedStyle>,
) {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("inherit") {
        if let Some(parent) = parent_style {
            apply_inherit(computed, property, parent);
        }
        return;
    }
    if trimmed.eq_ignore_ascii_case("initial") {
        apply_initial(computed, property);
        return;
    }
    if trimmed.eq_ignore_ascii_case("unset") {
        if crate::properties::is_inherited(property) {
            if let Some(parent) = parent_style {
                apply_inherit(computed, property, parent);
            }
        } else {
            apply_initial(computed, property);
        }
        return;
    }
    apply_single_property(computed, property, value, parent_style);
}

/// Apply a single CSS property name/value pair to a computed style.
///
/// `parent_style` is needed for `font-size`, whose relative units reference the
/// parent's font size rather than the value being computed.
fn apply_single_property(
    computed: &mut ComputedStyle,
    property: &str,
    value: &str,
    parent_style: Option<&ComputedStyle>,
) {
    let value = value.trim();

    match property {
        "fill" => {
            if let Some(paint) = Paint::parse(value) {
                computed.fill = paint;
            }
        }
        "fill-opacity" => {
            if let Ok(v) = value.parse::<f64>() {
                computed.fill_opacity = v.clamp(0.0, 1.0);
            }
        }
        "fill-rule" => {
            if let Some(rule) = FillRule::parse(value) {
                computed.fill_rule = rule;
            }
        }
        "stroke" => {
            if let Some(paint) = Paint::parse(value) {
                computed.stroke = paint;
            }
        }
        "stroke-opacity" => {
            if let Ok(v) = value.parse::<f64>() {
                computed.stroke_opacity = v.clamp(0.0, 1.0);
            }
        }
        "stroke-width" => {
            if let Ok(len) = value.parse::<SvgLength>() {
                computed.stroke_width = len.resolve_px(computed.font_size, computed.font_size);
            }
        }
        "stroke-linecap" => {
            if let Some(cap) = LineCap::parse(value) {
                computed.stroke_linecap = cap;
            }
        }
        "stroke-linejoin" => {
            if let Some(join) = LineJoin::parse(value) {
                computed.stroke_linejoin = join;
            }
        }
        "stroke-miterlimit" => {
            if let Ok(v) = value.parse::<f64>() {
                if v >= 1.0 {
                    computed.stroke_miterlimit = v;
                }
            }
        }
        "stroke-dasharray" => {
            if value.eq_ignore_ascii_case("none") {
                computed.stroke_dasharray = None;
            } else {
                let dashes: Vec<f64> = value
                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.trim().parse::<f64>().ok())
                    .collect();
                if !dashes.is_empty() {
                    computed.stroke_dasharray = Some(dashes);
                }
            }
        }
        "stroke-dashoffset" => {
            if let Ok(len) = value.parse::<SvgLength>() {
                computed.stroke_dashoffset = len.resolve_px(computed.font_size, computed.font_size);
            }
        }
        "opacity" => {
            if let Ok(v) = value.parse::<f64>() {
                computed.opacity = v.clamp(0.0, 1.0);
            }
        }
        "display" => {
            if let Some(d) = Display::parse(value) {
                computed.display = d;
            }
        }
        "visibility" => {
            if let Some(v) = Visibility::parse(value) {
                computed.visibility = v;
            }
        }
        "transform" => {
            if let Ok(t) = parse_transform(value) {
                computed.transform = t;
            }
        }
        "clip-path" => {
            computed.clip_path = parse_url_reference(value);
        }
        "clip-rule" => {
            if let Some(rule) = FillRule::parse(value) {
                computed.clip_rule = rule;
            }
        }
        "mask" => {
            computed.mask = parse_url_reference(value);
        }
        "filter" => {
            computed.filter = parse_url_reference(value);
        }
        "marker-start" => {
            computed.marker_start = parse_url_reference(value);
        }
        "marker-mid" => {
            computed.marker_mid = parse_url_reference(value);
        }
        "marker-end" => {
            computed.marker_end = parse_url_reference(value);
        }
        "font-family" => {
            let v = value.trim_matches(|c| c == '\'' || c == '"').to_string();
            if !v.is_empty() {
                computed.font_family = v;
            }
        }
        "font-size" => {
            if let Ok(len) = value.parse::<SvgLength>() {
                // `em` and `%` on font-size reference the *parent's* font-size,
                // not the value being computed. Reading it from `computed`
                // would compound whenever font-size has already been set on
                // this element from a lower-priority source, such as the
                // presentation attribute folded in from `style`.
                let reference = parent_style
                    .map(|parent| parent.font_size)
                    .unwrap_or(computed.font_size);
                computed.font_size = len.resolve_px(reference, reference);
            }
        }
        "font-weight" => {
            if let Some(w) = FontWeight::parse(value) {
                computed.font_weight = w;
            }
        }
        "font-style" => {
            if let Some(s) = FontStyle::parse(value) {
                computed.font_style = s;
            }
        }
        "text-anchor" => {
            if let Some(a) = TextAnchor::parse(value) {
                computed.text_anchor = a;
            }
        }
        "letter-spacing" => {
            if let Ok(len) = value.parse::<SvgLength>() {
                computed.letter_spacing = len.resolve_px(computed.font_size, computed.font_size);
            }
        }
        "word-spacing" => {
            if let Ok(len) = value.parse::<SvgLength>() {
                computed.word_spacing = len.resolve_px(computed.font_size, computed.font_size);
            }
        }
        "mix-blend-mode" => {
            if let Some(mode) = BlendMode::parse(value) {
                computed.mix_blend_mode = mode;
            }
        }
        "color" => {
            if let Some(c) = Color::parse(value) {
                computed.color = c;
            }
        }
        _ => {
            // Unknown property -- silently ignored
        }
    }
}

/// Apply the `inherit` keyword: copy the property value from the parent style.
fn apply_inherit(computed: &mut ComputedStyle, property: &str, parent: &ComputedStyle) {
    match property {
        "color" => computed.color = parent.color,
        "fill" => computed.fill = parent.fill.clone(),
        "fill-opacity" => computed.fill_opacity = parent.fill_opacity,
        "fill-rule" => computed.fill_rule = parent.fill_rule,
        "stroke" => computed.stroke = parent.stroke.clone(),
        "stroke-opacity" => computed.stroke_opacity = parent.stroke_opacity,
        "stroke-width" => computed.stroke_width = parent.stroke_width,
        "stroke-linecap" => computed.stroke_linecap = parent.stroke_linecap,
        "stroke-linejoin" => computed.stroke_linejoin = parent.stroke_linejoin,
        "stroke-miterlimit" => computed.stroke_miterlimit = parent.stroke_miterlimit,
        "stroke-dasharray" => computed.stroke_dasharray = parent.stroke_dasharray.clone(),
        "stroke-dashoffset" => computed.stroke_dashoffset = parent.stroke_dashoffset,
        "opacity" => computed.opacity = parent.opacity,
        "display" => computed.display = parent.display,
        "visibility" => computed.visibility = parent.visibility,
        "transform" => computed.transform = parent.transform,
        "clip-path" => computed.clip_path = parent.clip_path.clone(),
        "clip-rule" => computed.clip_rule = parent.clip_rule,
        "mask" => computed.mask = parent.mask.clone(),
        "filter" => computed.filter = parent.filter.clone(),
        "marker-start" => computed.marker_start = parent.marker_start.clone(),
        "marker-mid" => computed.marker_mid = parent.marker_mid.clone(),
        "marker-end" => computed.marker_end = parent.marker_end.clone(),
        "font-family" => computed.font_family = parent.font_family.clone(),
        "font-size" => computed.font_size = parent.font_size,
        "font-weight" => computed.font_weight = parent.font_weight,
        "font-style" => computed.font_style = parent.font_style,
        "text-anchor" => computed.text_anchor = parent.text_anchor,
        "letter-spacing" => computed.letter_spacing = parent.letter_spacing,
        "word-spacing" => computed.word_spacing = parent.word_spacing,
        "mix-blend-mode" => computed.mix_blend_mode = parent.mix_blend_mode,
        _ => {}
    }
}

/// Apply the `initial` keyword: reset the property to its initial (default) value.
fn apply_initial(computed: &mut ComputedStyle, property: &str) {
    let defaults = ComputedStyle::default();
    match property {
        "color" => computed.color = defaults.color,
        "fill" => computed.fill = defaults.fill,
        "fill-opacity" => computed.fill_opacity = defaults.fill_opacity,
        "fill-rule" => computed.fill_rule = defaults.fill_rule,
        "stroke" => computed.stroke = defaults.stroke,
        "stroke-opacity" => computed.stroke_opacity = defaults.stroke_opacity,
        "stroke-width" => computed.stroke_width = defaults.stroke_width,
        "stroke-linecap" => computed.stroke_linecap = defaults.stroke_linecap,
        "stroke-linejoin" => computed.stroke_linejoin = defaults.stroke_linejoin,
        "stroke-miterlimit" => computed.stroke_miterlimit = defaults.stroke_miterlimit,
        "stroke-dasharray" => computed.stroke_dasharray = defaults.stroke_dasharray,
        "stroke-dashoffset" => computed.stroke_dashoffset = defaults.stroke_dashoffset,
        "opacity" => computed.opacity = defaults.opacity,
        "display" => computed.display = defaults.display,
        "visibility" => computed.visibility = defaults.visibility,
        "transform" => computed.transform = defaults.transform,
        "clip-path" => computed.clip_path = defaults.clip_path,
        "clip-rule" => computed.clip_rule = defaults.clip_rule,
        "mask" => computed.mask = defaults.mask,
        "filter" => computed.filter = defaults.filter,
        "marker-start" => computed.marker_start = defaults.marker_start,
        "marker-mid" => computed.marker_mid = defaults.marker_mid,
        "marker-end" => computed.marker_end = defaults.marker_end,
        "font-family" => computed.font_family = defaults.font_family,
        "font-size" => computed.font_size = defaults.font_size,
        "font-weight" => computed.font_weight = defaults.font_weight,
        "font-style" => computed.font_style = defaults.font_style,
        "text-anchor" => computed.text_anchor = defaults.text_anchor,
        "letter-spacing" => computed.letter_spacing = defaults.letter_spacing,
        "word-spacing" => computed.word_spacing = defaults.word_spacing,
        "mix-blend-mode" => computed.mix_blend_mode = defaults.mix_blend_mode,
        _ => {}
    }
}

/// Resolve `Paint::CurrentColor` to `Paint::Color(computed.color)` for fill and stroke.
fn resolve_current_color(computed: &mut ComputedStyle) {
    if computed.fill == Paint::CurrentColor {
        computed.fill = Paint::Color(computed.color);
    }
    if computed.stroke == Paint::CurrentColor {
        computed.stroke = Paint::Color(computed.color);
    }
}

/// Parse a `url(#id)` or `none` reference value. Returns `Some(url_string)` or `None`.
fn parse_url_reference(value: &str) -> Option<String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    if let Some(rest) = value.strip_prefix("url(") {
        let url = rest
            .strip_suffix(')')?
            .trim()
            .trim_matches(|c| c == '\'' || c == '"');
        Some(url.to_string())
    } else {
        // Not a valid reference, return as-is if non-empty
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}

/// Resolve the computed style for a single node, given its parent's computed
/// style. This does **not** recurse into children — it is used by the style
/// cache to lazily recompute individual nodes.
pub fn resolve_single(
    doc: &SvgDocument,
    node_id: NodeId,
    parent_style: &ComputedStyle,
) -> ComputedStyle {
    let node = match doc.get_node(node_id) {
        Some(n) => n,
        None => return ComputedStyle::inherit_from(parent_style),
    };

    let mut computed = ComputedStyle::inherit_from(parent_style);

    // Apply matching stylesheet rules.
    let stylesheet = collect_stylesheets(doc);
    if !stylesheet.rules.is_empty() {
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

        let mut matching: Vec<(
            crate::stylesheet::Specificity,
            usize,
            &HashMap<String, String>,
        )> = Vec::new();
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
        matching.sort_by_key(|(spec, idx, _)| (*spec, *idx));
        for (_, _, declarations) in &matching {
            apply_inline_style_with_parent(&mut computed, declarations, Some(parent_style));
        }
    }

    // Apply presentation attributes.
    apply_presentation_attrs(&mut computed, &node.presentation);

    // Apply inline style.
    if let Some(style_str) = node.attributes.get("style") {
        let inline = parse_inline_style(style_str);
        apply_inline_style_with_parent(&mut computed, &inline, Some(parent_style));
    }

    // Resolve currentColor.
    resolve_current_color(&mut computed);

    computed
}

/// Write one CSS declaration onto a [`PresentationAttrs`].
///
/// Returns `true` when the property was recognized and stored. This is the
/// bridge used to fold `<style>` element rules into the attributes the
/// renderer reads; the fuller [`ComputedStyle`] path is not used there because
/// the renderer works from presentation attributes plus its own inheritance.
///
/// Values that need context the caller does not have — `inherit`, `initial`
/// and `unset` — are skipped, leaving the underlying attribute in place.
pub fn apply_declaration_to_presentation(
    attrs: &mut PresentationAttrs,
    property: &str,
    value: &str,
) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("inherit")
        || value.eq_ignore_ascii_case("initial")
        || value.eq_ignore_ascii_case("unset")
    {
        return false;
    }

    match property {
        "color" => set(&mut attrs.color, Color::parse(value)),
        "fill" => set(&mut attrs.fill, Paint::parse(value)),
        "stroke" => set(&mut attrs.stroke, Paint::parse(value)),
        "stroke-width" => set(&mut attrs.stroke_width, value.parse().ok()),
        "opacity" => set(&mut attrs.opacity, value.parse().ok()),
        "fill-opacity" => set(&mut attrs.fill_opacity, value.parse().ok()),
        "stroke-opacity" => set(&mut attrs.stroke_opacity, value.parse().ok()),
        "fill-rule" => set(&mut attrs.fill_rule, FillRule::parse(value)),
        "clip-rule" => set(&mut attrs.clip_rule, FillRule::parse(value)),
        "stroke-linecap" => set(&mut attrs.stroke_linecap, LineCap::parse(value)),
        "stroke-linejoin" => set(&mut attrs.stroke_linejoin, LineJoin::parse(value)),
        "stroke-miterlimit" => set(&mut attrs.stroke_miterlimit, value.parse().ok()),
        "stroke-dashoffset" => set(&mut attrs.stroke_dashoffset, value.parse().ok()),
        "stroke-dasharray" => {
            if value.eq_ignore_ascii_case("none") {
                return false;
            }
            let values: Vec<f64> = value
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            set(
                &mut attrs.stroke_dasharray,
                (!values.is_empty()).then_some(values),
            )
        }
        "display" => set(&mut attrs.display, Display::parse(value)),
        "visibility" => set(&mut attrs.visibility, Visibility::parse(value)),
        "mix-blend-mode" => set(&mut attrs.mix_blend_mode, BlendMode::parse(value)),
        "isolation" => set(&mut attrs.isolation, Some(value == "isolate")),
        "clip-path" => set(&mut attrs.clip_path, url_value(value)),
        "mask" => set(&mut attrs.mask, url_value(value)),
        "filter" => set(&mut attrs.filter, url_value(value)),
        "marker-start" => set(&mut attrs.marker_start, url_value(value)),
        "marker-mid" => set(&mut attrs.marker_mid, url_value(value)),
        "marker-end" => set(&mut attrs.marker_end, url_value(value)),
        "font-family" => set(
            &mut attrs.font_family,
            Some(value.trim_matches(['\'', '"']).to_string()),
        ),
        "font-size" => set(&mut attrs.font_size, value.parse().ok()),
        "font-weight" => set(&mut attrs.font_weight, FontWeight::parse(value)),
        "font-style" => set(&mut attrs.font_style, FontStyle::parse(value)),
        "text-anchor" => set(&mut attrs.text_anchor, TextAnchor::parse(value)),
        "letter-spacing" => set(&mut attrs.letter_spacing, value.parse().ok()),
        "word-spacing" => set(&mut attrs.word_spacing, value.parse().ok()),
        "text-decoration" => set(&mut attrs.text_decoration, TextDecoration::parse(value)),
        "vector-effect" => set(&mut attrs.vector_effect, Some(value.to_string())),
        _ => false,
    }
}

/// Store `parsed` into `slot`, reporting whether anything was stored.
fn set<T>(slot: &mut Option<T>, parsed: Option<T>) -> bool {
    match parsed {
        Some(v) => {
            *slot = Some(v);
            true
        }
        None => false,
    }
}

/// Extract the id from a `url(#id)` reference, treating `none` as unset.
fn url_value(value: &str) -> Option<String> {
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::{Color, GroupAttrs, PresentationAttrs, RectAttrs, SvgElement, SvgSvgAttrs};

    /// Helper: build a simple document for testing.
    fn make_test_doc() -> (SvgDocument, NodeId, NodeId, NodeId) {
        let mut doc = SvgDocument::new();

        // Root <svg>
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        // <g> with fill=red
        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g_id);
        if let Some(g_node) = doc.get_node_mut(g_id) {
            g_node.presentation.fill = Some(Paint::Color(Color::RED));
            g_node.presentation.opacity = Some(0.5);
        }

        // <rect> child of g (inherits fill, but not opacity)
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g_id, rect_id);

        (doc, svg_id, g_id, rect_id)
    }

    #[test]
    fn fill_inherits_from_parent() {
        let (doc, _svg_id, _g_id, rect_id) = make_test_doc();
        let styles = resolve_styles(&doc);

        let rect_style = styles.get(&rect_id).expect("no style for rect");
        // Fill should be inherited from the <g> parent
        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
    }

    #[test]
    fn opacity_does_not_inherit() {
        let (doc, _svg_id, _g_id, rect_id) = make_test_doc();
        let styles = resolve_styles(&doc);

        let rect_style = styles.get(&rect_id).expect("no style for rect");
        // Opacity is NOT inherited, should be default 1.0 (parent set 0.5)
        assert_eq!(rect_style.opacity, 1.0);
    }

    #[test]
    fn presentation_attrs_override_inherited() {
        let (mut doc, _svg_id, g_id, rect_id) = make_test_doc();

        // Set the rect's own fill to blue (overrides inherited red)
        if let Some(rect_node) = doc.get_node_mut(rect_id) {
            rect_node.presentation.fill = Some(Paint::Color(Color::BLUE));
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).expect("no style for rect");

        // Rect's own fill overrides the inherited red
        assert_eq!(rect_style.fill, Paint::Color(Color::BLUE));

        // Parent group still has red
        let g_style = styles.get(&g_id).expect("no style for g");
        assert_eq!(g_style.fill, Paint::Color(Color::RED));
    }

    #[test]
    fn inline_style_overrides_presentation_attrs() {
        let (mut doc, _svg_id, _g_id, rect_id) = make_test_doc();

        // Set presentation fill=blue AND inline style fill=green
        if let Some(rect_node) = doc.get_node_mut(rect_id) {
            rect_node.presentation.fill = Some(Paint::Color(Color::BLUE));
            rect_node
                .attributes
                .insert("style".to_string(), "fill: green".to_string());
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).expect("no style for rect");

        // Inline style wins over presentation attribute
        assert_eq!(rect_style.fill, Paint::Color(Color::GREEN));
    }

    #[test]
    fn full_tree_resolution() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
            <g fill="red" opacity="0.5" stroke-width="3">
                <rect x="0" y="0" width="100" height="100" stroke="blue"/>
                <circle cx="50" cy="50" r="25" fill="green"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root_id = doc.root().unwrap();
        let children = doc.children(root_id);
        assert_eq!(children.len(), 1);

        let g_id = children[0];
        let g_children = doc.children(g_id);
        assert_eq!(g_children.len(), 2);

        let rect_id = g_children[0];
        let circle_id = g_children[1];

        // Group style
        let g_style = styles.get(&g_id).unwrap();
        assert_eq!(g_style.fill, Paint::Color(Color::RED));
        assert_eq!(g_style.opacity, 0.5);
        assert_eq!(g_style.stroke_width, 3.0);

        // Rect inherits fill=red and stroke-width=3 from group, has own stroke=blue
        let rect_style = styles.get(&rect_id).unwrap();
        assert_eq!(rect_style.fill, Paint::Color(Color::RED)); // inherited
        assert_eq!(rect_style.stroke, Paint::Color(Color::BLUE)); // own
        assert_eq!(rect_style.stroke_width, 3.0); // inherited
        assert_eq!(rect_style.opacity, 1.0); // NOT inherited, default

        // Circle has own fill=green, inherits stroke-width=3
        let circle_style = styles.get(&circle_id).unwrap();
        assert_eq!(circle_style.fill, Paint::Color(Color::GREEN)); // own
        assert_eq!(circle_style.stroke_width, 3.0); // inherited
        assert_eq!(circle_style.opacity, 1.0); // NOT inherited, default
    }

    #[test]
    fn inline_style_parsing_in_tree() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"
                  fill="red" style="fill: blue; stroke-width: 5; opacity: 0.7"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root_id = doc.root().unwrap();
        let rect_id = doc.children(root_id)[0];
        let rect_style = styles.get(&rect_id).unwrap();

        // Inline style overrides presentation attr
        assert_eq!(rect_style.fill, Paint::Color(Color::BLUE));
        assert_eq!(rect_style.stroke_width, 5.0);
        assert_eq!(rect_style.opacity, 0.7);
    }

    #[test]
    fn apply_presentation_attrs_stroke_details() {
        let mut computed = ComputedStyle::default();
        let pres = PresentationAttrs {
            stroke_linecap: Some(LineCap::Round),
            stroke_linejoin: Some(LineJoin::Bevel),
            stroke_miterlimit: Some(8.0),
            stroke_dasharray: Some(vec![5.0, 10.0]),
            stroke_dashoffset: Some(2.0),
            fill_rule: Some(FillRule::EvenOdd),
            clip_rule: Some(FillRule::EvenOdd),
            ..PresentationAttrs::default()
        };

        apply_presentation_attrs(&mut computed, &pres);

        assert_eq!(computed.stroke_linecap, LineCap::Round);
        assert_eq!(computed.stroke_linejoin, LineJoin::Bevel);
        assert_eq!(computed.stroke_miterlimit, 8.0);
        assert_eq!(computed.stroke_dasharray, Some(vec![5.0, 10.0]));
        assert_eq!(computed.stroke_dashoffset, 2.0);
        assert_eq!(computed.fill_rule, FillRule::EvenOdd);
        assert_eq!(computed.clip_rule, FillRule::EvenOdd);
    }

    #[test]
    fn apply_inline_style_various_properties() {
        let mut computed = ComputedStyle::default();
        let mut inline = HashMap::new();
        inline.insert("fill".to_string(), "#ff0000".to_string());
        inline.insert("stroke".to_string(), "blue".to_string());
        inline.insert("stroke-width".to_string(), "3px".to_string());
        inline.insert("fill-opacity".to_string(), "0.5".to_string());
        inline.insert("stroke-linecap".to_string(), "round".to_string());
        inline.insert("font-family".to_string(), "Arial".to_string());
        inline.insert("font-size".to_string(), "24px".to_string());
        inline.insert("font-weight".to_string(), "bold".to_string());
        inline.insert("text-anchor".to_string(), "middle".to_string());
        inline.insert("display".to_string(), "none".to_string());
        inline.insert("visibility".to_string(), "hidden".to_string());
        inline.insert("mix-blend-mode".to_string(), "multiply".to_string());

        apply_inline_style(&mut computed, &inline);

        assert_eq!(computed.fill, Paint::Color(Color::RED));
        assert_eq!(computed.stroke, Paint::Color(Color::BLUE));
        assert_eq!(computed.stroke_width, 3.0);
        assert_eq!(computed.fill_opacity, 0.5);
        assert_eq!(computed.stroke_linecap, LineCap::Round);
        assert_eq!(computed.font_family, "Arial");
        assert_eq!(computed.font_size, 24.0);
        assert_eq!(computed.font_weight, FontWeight::Bold);
        assert_eq!(computed.text_anchor, TextAnchor::Middle);
        assert_eq!(computed.display, Display::None);
        assert_eq!(computed.visibility, Visibility::Hidden);
        assert_eq!(computed.mix_blend_mode, BlendMode::Multiply);
    }

    #[test]
    fn url_reference_parsing() {
        assert_eq!(
            parse_url_reference("url(#clip1)"),
            Some("#clip1".to_string())
        );
        assert_eq!(
            parse_url_reference("url('#filter')"),
            Some("#filter".to_string())
        );
        assert_eq!(parse_url_reference("none"), None);
        assert_eq!(parse_url_reference(""), None);
    }

    #[test]
    fn deep_inheritance_chain() {
        // Build: svg > g1 (fill=red) > g2 (stroke=blue) > rect
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let g1_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g1_id);
        if let Some(g1) = doc.get_node_mut(g1_id) {
            g1.presentation.fill = Some(Paint::Color(Color::RED));
            g1.presentation.fill_rule = Some(FillRule::EvenOdd);
        }

        let g2_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(g1_id, g2_id);
        if let Some(g2) = doc.get_node_mut(g2_id) {
            g2.presentation.stroke = Some(Paint::Color(Color::BLUE));
            g2.presentation.stroke_width = Some(SvgLength::px(5.0));
        }

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g2_id, rect_id);

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        // fill=red inherited from g1, through g2
        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
        assert_eq!(rect_style.fill_rule, FillRule::EvenOdd);
        // stroke=blue inherited from g2
        assert_eq!(rect_style.stroke, Paint::Color(Color::BLUE));
        assert_eq!(rect_style.stroke_width, 5.0);
    }

    // -------------------------------------------------------------------
    // currentColor resolution tests
    // -------------------------------------------------------------------

    #[test]
    fn current_color_resolves_fill() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        // Group with color=red
        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g_id);
        if let Some(g) = doc.get_node_mut(g_id) {
            g.presentation.color = Some(Color::RED);
        }

        // Rect with fill=currentColor
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g_id, rect_id);
        if let Some(r) = doc.get_node_mut(rect_id) {
            r.presentation.fill = Some(Paint::CurrentColor);
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        // currentColor should resolve to the inherited color=red
        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
        assert_eq!(rect_style.color, Color::RED);
    }

    #[test]
    fn current_color_resolves_stroke() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(svg_id, rect_id);
        if let Some(r) = doc.get_node_mut(rect_id) {
            r.presentation.color = Some(Color::BLUE);
            r.presentation.stroke = Some(Paint::CurrentColor);
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.stroke, Paint::Color(Color::BLUE));
    }

    #[test]
    fn current_color_default_is_black() {
        // When no color property is set, currentColor defaults to black
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(svg_id, rect_id);
        if let Some(r) = doc.get_node_mut(rect_id) {
            r.presentation.fill = Some(Paint::CurrentColor);
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.fill, Paint::Color(Color::BLACK));
    }

    #[test]
    fn current_color_inherits_through_chain() {
        // svg (color=green) > g > rect (fill=currentColor)
        // rect should get fill=green via inherited color
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        if let Some(svg) = doc.get_node_mut(svg_id) {
            svg.presentation.color = Some(Color::GREEN);
        }

        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g_id, rect_id);
        if let Some(r) = doc.get_node_mut(rect_id) {
            r.presentation.fill = Some(Paint::CurrentColor);
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        // color=green inherited from svg through g
        assert_eq!(rect_style.fill, Paint::Color(Color::GREEN));
    }

    #[test]
    fn current_color_via_inline_style() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"
                  color="red" style="fill: currentColor"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root_id = doc.root().unwrap();
        let rect_id = doc.children(root_id)[0];
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
    }

    #[test]
    fn color_property_inherits() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g_id);
        if let Some(g) = doc.get_node_mut(g_id) {
            g.presentation.color = Some(Color::ORANGE);
        }

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g_id, rect_id);

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();

        // color should be inherited from parent
        assert_eq!(rect_style.color, Color::ORANGE);
    }

    // -------------------------------------------------------------------
    // inherit / initial / unset keyword tests
    // -------------------------------------------------------------------

    #[test]
    fn initial_keyword_resets_inherited_property() {
        // Parent sets fill=red, child uses fill: initial → should be black (default)
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g fill="red">
                <rect x="0" y="0" width="10" height="10" style="fill: initial"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // initial fill = black (SVG default)
        assert_eq!(rect_style.fill, Paint::Color(Color::BLACK));
    }

    #[test]
    fn inherit_keyword_forces_non_inherited_property() {
        // Parent sets opacity=0.5, child uses opacity: inherit
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g opacity="0.5">
                <rect x="0" y="0" width="10" height="10" style="opacity: inherit"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // opacity is not normally inherited, but `inherit` forces it
        assert_eq!(rect_style.opacity, 0.5);
    }

    #[test]
    fn unset_on_inherited_property_acts_as_inherit() {
        // fill is inherited, so unset should keep parent's fill=red
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g fill="red">
                <rect x="0" y="0" width="10" height="10" style="fill: unset"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
    }

    #[test]
    fn unset_on_non_inherited_property_acts_as_initial() {
        // opacity is non-inherited, so unset should reset to initial (1.0)
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g opacity="0.5">
                <rect x="0" y="0" width="10" height="10"
                      opacity="0.3" style="opacity: unset"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // unset on non-inherited => initial => 1.0
        assert_eq!(rect_style.opacity, 1.0);
    }

    #[test]
    fn initial_keyword_on_font_size() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 32px">
                <text style="font-size: initial">test</text>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let text_id = doc.children(g_id)[0];

        let text_style = styles.get(&text_id).unwrap();
        // initial font-size = 16.0
        assert_eq!(text_style.font_size, 16.0);
    }

    // -------------------------------------------------------------------
    // Relative length resolution (em, ex, %) tests
    // -------------------------------------------------------------------

    #[test]
    fn font_size_em_resolves_to_parent() {
        // Parent font-size=20px, child font-size=2em => 40px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 20px">
                <rect x="0" y="0" width="10" height="10" style="font-size: 2em"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.font_size - 40.0).abs() < 0.001);
    }

    #[test]
    fn font_size_percent_resolves_to_parent() {
        // Parent font-size=20px, child font-size=150% => 30px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 20px">
                <rect x="0" y="0" width="10" height="10" style="font-size: 150%"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.font_size - 30.0).abs() < 0.001);
    }

    #[test]
    fn stroke_width_em_uses_element_font_size() {
        // Font-size=24px, stroke-width=0.5em => 12px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"
                  style="font-size: 24px; stroke-width: 0.5em"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.stroke_width - 12.0).abs() < 0.001);
    }

    #[test]
    fn stroke_width_em_inherits_font_size() {
        // Parent font-size=30px, child stroke-width=0.1em => 3px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 30px">
                <rect x="0" y="0" width="10" height="10" style="stroke-width: 0.1em"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.stroke_width - 3.0).abs() < 0.001);
    }

    #[test]
    fn font_size_ex_resolves_to_half_em() {
        // Parent font-size=20px, child font-size=4ex => 4 * 10 = 40px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 20px">
                <rect x="0" y="0" width="10" height="10" style="font-size: 4ex"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // ex ≈ 0.5 * em, so 4ex with parent 20px = 4 * 10 = 40px
        assert!((rect_style.font_size - 40.0).abs() < 0.001);
    }

    #[test]
    fn letter_spacing_em_uses_font_size() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"
                  style="font-size: 20px; letter-spacing: 0.1em"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // 0.1em * 20px = 2px
        assert!((rect_style.letter_spacing - 2.0).abs() < 0.001);
    }

    #[test]
    fn font_size_em_chain() {
        // Root default=16px > g1 (2em=32px) > g2 (1.5em=48px) > rect inherits 48px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g style="font-size: 2em">
                <g style="font-size: 1.5em">
                    <rect x="0" y="0" width="10" height="10"/>
                </g>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g1 = doc.children(root)[0];
        let g2 = doc.children(g1)[0];
        let rect_id = doc.children(g2)[0];

        let g1_style = styles.get(&g1).unwrap();
        assert!((g1_style.font_size - 32.0).abs() < 0.001); // 16 * 2

        let g2_style = styles.get(&g2).unwrap();
        assert!((g2_style.font_size - 48.0).abs() < 0.001); // 32 * 1.5

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.font_size - 48.0).abs() < 0.001); // inherited from g2
    }

    #[test]
    fn stroke_width_presentation_attr_em() {
        // Test em resolution in presentation attributes (not just inline style)
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        // Group with font-size=20px via inline style
        let g_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        doc.append_child(svg_id, g_id);
        if let Some(g) = doc.get_node_mut(g_id) {
            g.attributes
                .insert("style".to_string(), "font-size: 20px".to_string());
        }

        // Rect with stroke-width=0.5em via presentation attribute
        use sivgo_dom::LengthUnit;
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(g_id, rect_id);
        if let Some(r) = doc.get_node_mut(rect_id) {
            r.presentation.stroke_width = Some(SvgLength::new(0.5, LengthUnit::Em));
        }

        let styles = resolve_styles(&doc);
        let rect_style = styles.get(&rect_id).unwrap();
        // 0.5em * 20px (inherited font-size) = 10px
        assert!((rect_style.stroke_width - 10.0).abs() < 0.001);
    }

    #[test]
    fn font_size_pt_resolves_correctly() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" style="font-size: 12pt"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        // 12pt = 12 * 96/72 = 16px
        assert!((rect_style.font_size - 16.0).abs() < 0.001);
    }

    // -------------------------------------------------------------------
    // Presentation attribute tests for new attributes
    // -------------------------------------------------------------------

    #[test]
    fn clip_path_presentation_attr_parsed() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" clip-path="url(#clip1)"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.clip_path, Some("url(#clip1)".to_string()));
    }

    #[test]
    fn marker_attributes_parsed() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <path d="M0,0 L10,10" marker-start="url(#arrow)" marker-end="url(#dot)"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let path_id = doc.children(root)[0];
        let path_style = styles.get(&path_id).unwrap();

        assert_eq!(path_style.marker_start, Some("url(#arrow)".to_string()));
        assert_eq!(path_style.marker_end, Some("url(#dot)".to_string()));
        assert_eq!(path_style.marker_mid, None);
    }

    #[test]
    fn font_family_presentation_attr() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" font-family="Arial"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.font_family, "Arial");
    }

    #[test]
    fn font_size_presentation_attr_em() {
        // Parent font-size=24px via presentation attr, child font-size=0.5em => 12px
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g font-size="24px">
                <rect x="0" y="0" width="10" height="10" font-size="0.5em"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert!((rect_style.font_size - 12.0).abs() < 0.001);
    }

    #[test]
    fn mix_blend_mode_presentation_attr() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" mix-blend-mode="multiply"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];
        let rect_style = styles.get(&rect_id).unwrap();

        assert_eq!(rect_style.mix_blend_mode, BlendMode::Multiply);
    }

    #[test]
    fn font_properties_inherit_through_group() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g font-family="Helvetica" font-size="24px" font-weight="bold" font-style="italic">
                <rect x="0" y="0" width="10" height="10"/>
            </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let rect_id = doc.children(g_id)[0];

        let rect_style = styles.get(&rect_id).unwrap();
        assert_eq!(rect_style.font_family, "Helvetica");
        assert!((rect_style.font_size - 24.0).abs() < 0.001);
        assert_eq!(rect_style.font_weight, FontWeight::Bold);
        assert_eq!(rect_style.font_style, FontStyle::Italic);
    }

    // -------------------------------------------------------------------
    // Embedded <style> element CSS tests
    // -------------------------------------------------------------------

    #[test]
    fn style_element_applies_to_elements() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>rect { fill: red; stroke-width: 3 }</style>
            <rect x="0" y="0" width="10" height="10"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        // Find the rect (skip style element)
        let children = doc.children(root);
        let rect_id = children
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        let rect_style = styles.get(&rect_id).unwrap();
        assert_eq!(rect_style.fill, Paint::Color(Color::RED));
        assert_eq!(rect_style.stroke_width, 3.0);
    }

    #[test]
    fn style_element_class_selector() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>.highlight { fill: blue; opacity: 0.7 }</style>
            <rect class="highlight" x="0" y="0" width="10" height="10"/>
            <rect x="0" y="0" width="10" height="10"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let children = doc.children(root);
        let rects: Vec<_> = children
            .iter()
            .filter(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .collect();

        assert_eq!(rects.len(), 2);

        // First rect has class="highlight" -> should get blue fill
        let r1_style = styles.get(&rects[0]).unwrap();
        assert_eq!(r1_style.fill, Paint::Color(Color::BLUE));
        assert_eq!(r1_style.opacity, 0.7);

        // Second rect has no class -> default fill (black)
        let r2_style = styles.get(&rects[1]).unwrap();
        assert_eq!(r2_style.fill, Paint::Color(Color::BLACK));
        assert_eq!(r2_style.opacity, 1.0);
    }

    #[test]
    fn style_element_id_selector() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>#special { fill: green }</style>
            <rect id="special" x="0" y="0" width="10" height="10"/>
            <rect x="0" y="0" width="10" height="10"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let children = doc.children(root);
        let rects: Vec<_> = children
            .iter()
            .filter(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .collect();

        let r1_style = styles.get(&rects[0]).unwrap();
        assert_eq!(r1_style.fill, Paint::Color(Color::GREEN));

        let r2_style = styles.get(&rects[1]).unwrap();
        assert_eq!(r2_style.fill, Paint::Color(Color::BLACK));
    }

    #[test]
    fn presentation_attr_overrides_stylesheet() {
        // Presentation attributes have higher priority than stylesheet rules
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>rect { fill: red }</style>
            <rect x="0" y="0" width="10" height="10" fill="blue"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        let rect_style = styles.get(&rect_id).unwrap();
        // Presentation attribute overrides stylesheet
        assert_eq!(rect_style.fill, Paint::Color(Color::BLUE));
    }

    #[test]
    fn inline_style_overrides_stylesheet() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>rect { fill: red }</style>
            <rect x="0" y="0" width="10" height="10" style="fill: green"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        let rect_style = styles.get(&rect_id).unwrap();
        assert_eq!(rect_style.fill, Paint::Color(Color::GREEN));
    }

    #[test]
    fn specificity_id_beats_class() {
        let svg_str = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>
                .cls { fill: red }
                #special { fill: green }
            </style>
            <rect id="special" class="cls" x="0" y="0" width="10" height="10"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg_str).unwrap();
        let styles = resolve_styles(&doc);

        let root = doc.root().unwrap();
        let rect_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        let rect_style = styles.get(&rect_id).unwrap();
        // ID selector (#special) has higher specificity than class selector (.cls)
        assert_eq!(rect_style.fill, Paint::Color(Color::GREEN));
    }

    #[test]
    fn cascade_descendant_selector() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>g rect { fill: blue }</style>
            <g>
                <rect x="0" y="0" width="10" height="10"/>
            </g>
            <rect x="0" y="0" width="10" height="10"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let styles = resolve_styles(&doc);
        let root = doc.root().unwrap();
        let g_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("g"))
            .copied()
            .unwrap();
        let nested_rect = doc.children(g_id)[0];
        let top_rect = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        // Nested rect should match "g rect" → fill: blue
        let nested_style = styles.get(&nested_rect).unwrap();
        assert_eq!(nested_style.fill, Paint::Color(Color::BLUE));

        // Top-level rect should NOT match "g rect" → fill: default (black)
        let top_style = styles.get(&top_rect).unwrap();
        assert_eq!(top_style.fill, Paint::Color(Color::BLACK));
    }

    #[test]
    fn cascade_child_selector() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>svg > rect { fill: red }</style>
            <g>
                <rect x="0" y="0" width="10" height="10"/>
            </g>
            <rect x="0" y="0" width="10" height="10"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let styles = resolve_styles(&doc);
        let root = doc.root().unwrap();
        let g_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("g"))
            .copied()
            .unwrap();
        let nested_rect = doc.children(g_id)[0];
        let top_rect = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("rect"))
            .copied()
            .unwrap();

        // Top-level rect is direct child of svg → fill: red
        let top_style = styles.get(&top_rect).unwrap();
        assert_eq!(top_style.fill, Paint::Color(Color::RED));

        // Nested rect is child of g, not svg → fill: default (black)
        let nested_style = styles.get(&nested_rect).unwrap();
        assert_eq!(nested_style.fill, Paint::Color(Color::BLACK));
    }

    #[test]
    fn cascade_sibling_selector() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>rect + circle { fill: green }</style>
            <rect x="0" y="0" width="10" height="10"/>
            <circle cx="5" cy="5" r="3"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let styles = resolve_styles(&doc);
        let root = doc.root().unwrap();
        let circle_id = doc
            .children(root)
            .iter()
            .find(|&&id| doc.get_node(id).map(|n| n.element.tag_name()) == Some("circle"))
            .copied()
            .unwrap();

        // Circle immediately follows rect → fill: green
        let circle_style = styles.get(&circle_id).unwrap();
        assert_eq!(circle_style.fill, Paint::Color(Color::GREEN));
    }
}
