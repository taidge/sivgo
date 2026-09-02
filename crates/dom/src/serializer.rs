//! Serialize an [`SvgDocument`] back to SVG XML.

use xmlwriter::XmlWriter;

use crate::document::SvgDocument;
use crate::node::*;
use crate::types::*;

/// Serialize an [`SvgDocument`] to an SVG XML string.
pub fn serialize_svg(doc: &SvgDocument) -> String {
    let mut w = XmlWriter::new(xmlwriter::Options {
        use_single_quote: false,
        ..Default::default()
    });

    // Check if any node in the document uses xlink:href -- if so, we need to
    // declare the xlink namespace on the root element.
    let needs_xlink = doc_needs_xlink(doc);

    if let Some(root_id) = doc.root() {
        serialize_node(doc, root_id, &mut w, true, needs_xlink);
    }

    w.end_document()
}

/// Check whether any node has an href that came through xlink.
/// We detect this by looking for gradient/pattern/use/image/textPath/feImage/anchor href fields.
fn doc_needs_xlink(_doc: &SvgDocument) -> bool {
    // For simplicity, we never emit xlink:href -- we always emit the modern `href`.
    // If users need xlink:href they can rely on the round-tripped extra attributes.
    false
}

/// Writes attributes for a single element, dropping duplicates.
///
/// Several attributes are reachable through more than one path: font
/// properties live on both [`TextAttrs`] and [`PresentationAttrs`], and
/// attributes that the parser stores in a typed field may also survive in the
/// node's extra-attribute map. Emitting a name twice produces XML that no
/// conformant parser will accept, so the first write of a given name wins and
/// later ones are skipped.
struct AttrWriter<'a> {
    inner: &'a mut XmlWriter,
    written: Vec<String>,
}

impl<'a> AttrWriter<'a> {
    fn new(inner: &'a mut XmlWriter) -> Self {
        AttrWriter {
            inner,
            written: Vec::new(),
        }
    }

    fn write_attribute(&mut self, name: &str, value: &str) {
        if self.written.iter().any(|n| n == name) {
            return;
        }
        self.written.push(name.to_string());
        self.inner.write_attribute(name, value);
    }
}

fn serialize_node(
    doc: &SvgDocument,
    id: NodeId,
    w: &mut XmlWriter,
    is_root: bool,
    needs_xlink: bool,
) {
    let node = match doc.get_node(id) {
        Some(n) => n,
        None => return,
    };

    let tag = node.element.tag_name();
    w.start_element(tag);

    {
        // Every attribute for this element goes through `aw`, which drops a
        // name that has already been emitted. Without it, attributes reachable
        // from two sources (font properties, `inkscape:path-effect`, ...) would
        // be written twice and the output would fail to re-parse.
        let mut aw = AttrWriter::new(w);

        // SVG namespace on root element.
        if is_root {
            aw.write_attribute("xmlns", "http://www.w3.org/2000/svg");
            if needs_xlink {
                aw.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
            }
        }

        // Namespace declarations from the original document (for round-tripping).
        // Only emit on the root element to avoid duplicates — child nodes may
        // report inherited namespace declarations from the parser.
        if is_root {
            let mut ns_keys: Vec<&String> = node.namespaces.keys().collect();
            ns_keys.sort();
            for prefix in ns_keys {
                if let Some(uri) = node.namespaces.get(prefix) {
                    if prefix.is_empty() {
                        // Default namespace — already written above for root.
                        if uri != "http://www.w3.org/2000/svg" {
                            aw.write_attribute("xmlns", uri);
                        }
                    } else {
                        // Skip xlink if already written above.
                        if !(needs_xlink && prefix == "xlink") {
                            let attr_name = format!("xmlns:{prefix}");
                            aw.write_attribute(&attr_name, uri);
                        }
                    }
                }
            }
        }

        // xml_id as the `id` attribute.
        if let Some(ref xml_id) = node.xml_id {
            aw.write_attribute("id", xml_id);
        }

        // Element-specific attributes.
        write_element_attrs(&node.element, &mut aw);

        // Presentation attributes.
        write_presentation_attrs(&node.presentation, &mut aw);

        // Conditional processing attributes.
        if let Some(ref rf) = node.required_features {
            aw.write_attribute("requiredFeatures", &rf.join(" "));
        }
        if let Some(ref re) = node.required_extensions {
            aw.write_attribute("requiredExtensions", &re.join(" "));
        }
        if let Some(ref sl) = node.system_language {
            aw.write_attribute("systemLanguage", &sl.join(","));
        }

        // Extra (unrecognized / round-tripped) attributes.
        let mut extra_keys: Vec<&String> = node.attributes.keys().collect();
        extra_keys.sort(); // deterministic order
        for key in extra_keys {
            if let Some(val) = node.attributes.get(key) {
                aw.write_attribute(key, val);
            }
        }
    }

    // Text content for text-bearing elements.
    let text_content = get_text_content(&node.element);

    // Children.
    let children = doc.children(id);
    if children.is_empty() && text_content.is_none() {
        w.end_element();
    } else {
        if let Some(text) = text_content {
            if !text.is_empty() {
                w.write_text(text);
            }
        }
        for &child_id in children {
            serialize_node(doc, child_id, w, false, needs_xlink);
        }
        w.end_element();
    }
}

/// Extract text content from elements that carry it.
fn get_text_content(elem: &SvgElement) -> Option<&str> {
    match elem {
        SvgElement::Text(a) => Some(&a.text_content),
        SvgElement::TSpan(a) => Some(&a.text_content),
        SvgElement::TextPath(a) => Some(&a.text_content),
        SvgElement::Title(a) => Some(&a.text_content),
        SvgElement::Desc(a) => Some(&a.text_content),
        SvgElement::Metadata(a) => Some(&a.raw_content),
        SvgElement::StyleElement(a) => Some(&a.content),
        SvgElement::Script(a) => a.content.as_deref(),
        SvgElement::FlowPara(a) => Some(&a.text_content),
        SvgElement::FlowSpan(a) => Some(&a.text_content),
        SvgElement::Unknown(a) => Some(&a.text_content),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Element attributes
// ---------------------------------------------------------------------------

fn write_element_attrs(elem: &SvgElement, w: &mut AttrWriter) {
    match elem {
        SvgElement::Svg(a) => {
            if let Some(ref width) = a.width {
                w.write_attribute("width", &width.to_string());
            }
            if let Some(ref height) = a.height {
                w.write_attribute("height", &height.to_string());
            }
            if let Some(ref vb) = a.view_box {
                w.write_attribute("viewBox", &vb.to_svg());
            }
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
        }
        SvgElement::Rect(a) => {
            write_len_attr(w, "x", &a.x);
            write_len_attr(w, "y", &a.y);
            write_len_attr(w, "width", &a.width);
            write_len_attr(w, "height", &a.height);
            write_len_attr_nonzero(w, "rx", &a.rx);
            write_len_attr_nonzero(w, "ry", &a.ry);
        }
        SvgElement::Circle(a) => {
            write_len_attr(w, "cx", &a.cx);
            write_len_attr(w, "cy", &a.cy);
            write_len_attr(w, "r", &a.r);
        }
        SvgElement::Ellipse(a) => {
            write_len_attr(w, "cx", &a.cx);
            write_len_attr(w, "cy", &a.cy);
            write_len_attr(w, "rx", &a.rx);
            write_len_attr(w, "ry", &a.ry);
        }
        SvgElement::Line(a) => {
            write_len_attr(w, "x1", &a.x1);
            write_len_attr(w, "y1", &a.y1);
            write_len_attr(w, "x2", &a.x2);
            write_len_attr(w, "y2", &a.y2);
        }
        SvgElement::Polyline(a) => {
            if !a.points.is_empty() {
                w.write_attribute("points", &serialize_points(&a.points));
            }
        }
        SvgElement::Polygon(a) => {
            if !a.points.is_empty() {
                w.write_attribute("points", &serialize_points(&a.points));
            }
        }
        SvgElement::Path(a) => {
            let d = a.d.to_svg_string();
            if !d.is_empty() {
                w.write_attribute("d", &d);
            }
            if let Some(ref pe) = a.inkscape_path_effect {
                w.write_attribute("inkscape:path-effect", pe);
            }
            if let Some(ref od) = a.inkscape_original_d {
                let s = od.to_svg_string();
                if !s.is_empty() {
                    w.write_attribute("inkscape:original-d", &s);
                }
            }
        }
        SvgElement::Group(a) => {
            if let Some(ref gm) = a.inkscape_groupmode {
                w.write_attribute("inkscape:groupmode", gm);
            }
            if let Some(ref lbl) = a.inkscape_label {
                w.write_attribute("inkscape:label", lbl);
            }
        }
        SvgElement::Defs(_) => {}

        // -- Phase 2 elements --
        SvgElement::LinearGradient(a) => {
            write_opt_len_attr(w, "x1", &a.x1);
            write_opt_len_attr(w, "y1", &a.y1);
            write_opt_len_attr(w, "x2", &a.x2);
            write_opt_len_attr(w, "y2", &a.y2);
            if let Some(ref gu) = a.gradient_units {
                w.write_attribute("gradientUnits", gu.to_svg());
            }
            if let Some(ref gt) = a.gradient_transform {
                let c = gt.as_coeffs();
                w.write_attribute(
                    "gradientTransform",
                    &format!(
                        "matrix({},{},{},{},{},{})",
                        c[0], c[1], c[2], c[3], c[4], c[5]
                    ),
                );
            }
            if let Some(ref sm) = a.spread_method {
                w.write_attribute("spreadMethod", sm.to_svg());
            }
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
        }
        SvgElement::RadialGradient(a) => {
            write_opt_len_attr(w, "cx", &a.cx);
            write_opt_len_attr(w, "cy", &a.cy);
            write_opt_len_attr(w, "r", &a.r);
            write_opt_len_attr(w, "fx", &a.fx);
            write_opt_len_attr(w, "fy", &a.fy);
            if let Some(ref gu) = a.gradient_units {
                w.write_attribute("gradientUnits", gu.to_svg());
            }
            if let Some(ref gt) = a.gradient_transform {
                let c = gt.as_coeffs();
                w.write_attribute(
                    "gradientTransform",
                    &format!(
                        "matrix({},{},{},{},{},{})",
                        c[0], c[1], c[2], c[3], c[4], c[5]
                    ),
                );
            }
            if let Some(ref sm) = a.spread_method {
                w.write_attribute("spreadMethod", sm.to_svg());
            }
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
        }
        SvgElement::Stop(a) => {
            w.write_attribute("offset", &a.offset.to_string());
            if let Some(ref c) = a.stop_color {
                w.write_attribute("stop-color", &c.to_hex());
            }
            if let Some(op) = a.stop_opacity {
                w.write_attribute("stop-opacity", &op.to_string());
            }
            if let Some(ref p) = a.path {
                w.write_attribute("path", p);
            }
        }
        SvgElement::Pattern(a) => {
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
            if let Some(ref pu) = a.pattern_units {
                w.write_attribute("patternUnits", pu.to_svg());
            }
            if let Some(ref pcu) = a.pattern_content_units {
                w.write_attribute("patternContentUnits", pcu.to_svg());
            }
            if let Some(ref pt) = a.pattern_transform {
                let c = pt.as_coeffs();
                w.write_attribute(
                    "patternTransform",
                    &format!(
                        "matrix({},{},{},{},{},{})",
                        c[0], c[1], c[2], c[3], c[4], c[5]
                    ),
                );
            }
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            if let Some(ref vb) = a.view_box {
                w.write_attribute("viewBox", &vb.to_svg());
            }
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
        }
        SvgElement::ClipPath(a) => {
            if let Some(ref cu) = a.clip_path_units {
                w.write_attribute("clipPathUnits", cu.to_svg());
            }
        }
        SvgElement::Mask(a) => {
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
            if let Some(ref mu) = a.mask_units {
                w.write_attribute("maskUnits", mu.to_svg());
            }
            if let Some(ref mcu) = a.mask_content_units {
                w.write_attribute("maskContentUnits", mcu.to_svg());
            }
        }
        SvgElement::Use(a) => {
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
        }
        SvgElement::Symbol(a) => {
            if let Some(ref vb) = a.view_box {
                w.write_attribute("viewBox", &vb.to_svg());
            }
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
        }
        SvgElement::Image(a) => {
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
        }
        SvgElement::Text(a) => {
            if !a.x.is_empty() {
                w.write_attribute("x", &serialize_length_list(&a.x));
            }
            if !a.y.is_empty() {
                w.write_attribute("y", &serialize_length_list(&a.y));
            }
            if !a.dx.is_empty() {
                w.write_attribute("dx", &serialize_length_list(&a.dx));
            }
            if !a.dy.is_empty() {
                w.write_attribute("dy", &serialize_length_list(&a.dy));
            }
            if let Some(ref ff) = a.font_family {
                w.write_attribute("font-family", ff);
            }
            if let Some(ref fs) = a.font_size {
                w.write_attribute("font-size", &fs.to_string());
            }
            if let Some(ref fw) = a.font_weight {
                w.write_attribute("font-weight", &fw.to_svg());
            }
            if let Some(ref fst) = a.font_style {
                w.write_attribute("font-style", fst.to_svg());
            }
            if let Some(ref ta) = a.text_anchor {
                w.write_attribute("text-anchor", ta.to_svg());
            }
        }
        SvgElement::TSpan(a) => {
            if !a.x.is_empty() {
                w.write_attribute("x", &serialize_length_list(&a.x));
            }
            if !a.y.is_empty() {
                w.write_attribute("y", &serialize_length_list(&a.y));
            }
            if !a.dx.is_empty() {
                w.write_attribute("dx", &serialize_length_list(&a.dx));
            }
            if !a.dy.is_empty() {
                w.write_attribute("dy", &serialize_length_list(&a.dy));
            }
            if let Some(ref ff) = a.font_family {
                w.write_attribute("font-family", ff);
            }
            if let Some(ref fs) = a.font_size {
                w.write_attribute("font-size", &fs.to_string());
            }
            if let Some(ref fw) = a.font_weight {
                w.write_attribute("font-weight", &fw.to_svg());
            }
            if let Some(ref fst) = a.font_style {
                w.write_attribute("font-style", fst.to_svg());
            }
            if let Some(ref ta) = a.text_anchor {
                w.write_attribute("text-anchor", ta.to_svg());
            }
        }
        SvgElement::TextPath(a) => {
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            if let Some(ref so) = a.start_offset {
                w.write_attribute("startOffset", &so.to_string());
            }
        }
        SvgElement::Marker(a) => {
            write_opt_len_attr(w, "refX", &a.ref_x);
            write_opt_len_attr(w, "refY", &a.ref_y);
            write_opt_len_attr(w, "markerWidth", &a.marker_width);
            write_opt_len_attr(w, "markerHeight", &a.marker_height);
            if let Some(ref o) = a.orient {
                w.write_attribute("orient", &o.to_svg());
            }
            if let Some(ref mu) = a.marker_units {
                w.write_attribute("markerUnits", mu.to_svg());
            }
            if let Some(ref vb) = a.view_box {
                w.write_attribute("viewBox", &vb.to_svg());
            }
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
        }
        SvgElement::Filter(a) => {
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
            if let Some(ref fu) = a.filter_units {
                w.write_attribute("filterUnits", fu.to_svg());
            }
            if let Some(ref pu) = a.primitive_units {
                w.write_attribute("primitiveUnits", pu.to_svg());
            }
        }
        SvgElement::FeGaussianBlur(a) => {
            write_filter_input(w, "in", &a.input);
            write_std_deviation(w, a.std_deviation_x, a.std_deviation_y);
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeOffset(a) => {
            write_filter_input(w, "in", &a.input);
            if a.dx != 0.0 {
                w.write_attribute("dx", &a.dx.to_string());
            }
            if a.dy != 0.0 {
                w.write_attribute("dy", &a.dy.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeFlood(a) => {
            if let Some(ref c) = a.flood_color {
                w.write_attribute("flood-color", &c.to_hex());
            }
            if let Some(op) = a.flood_opacity {
                w.write_attribute("flood-opacity", &op.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeBlend(a) => {
            write_filter_input(w, "in", &a.input);
            write_filter_input(w, "in2", &a.in2);
            if let Some(ref m) = a.mode {
                w.write_attribute("mode", m.to_svg());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeComposite(a) => {
            write_filter_input(w, "in", &a.input);
            write_filter_input(w, "in2", &a.in2);
            if let Some(ref op) = a.operator {
                w.write_attribute("operator", op.to_svg());
            }
            if a.k1 != 0.0 {
                w.write_attribute("k1", &a.k1.to_string());
            }
            if a.k2 != 0.0 {
                w.write_attribute("k2", &a.k2.to_string());
            }
            if a.k3 != 0.0 {
                w.write_attribute("k3", &a.k3.to_string());
            }
            if a.k4 != 0.0 {
                w.write_attribute("k4", &a.k4.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeColorMatrix(a) => {
            write_filter_input(w, "in", &a.input);
            if let Some(ref mt) = a.matrix_type {
                w.write_attribute("type", mt.to_svg());
            }
            if let Some(ref v) = a.values {
                w.write_attribute("values", v);
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeMerge(a) => {
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeMergeNode(a) => {
            write_filter_input(w, "in", &a.input);
        }
        SvgElement::FeMorphology(a) => {
            write_filter_input(w, "in", &a.input);
            if let Some(ref op) = a.operator {
                w.write_attribute("operator", op.to_svg());
            }
            if a.radius_x != 0.0 || a.radius_y != 0.0 {
                if (a.radius_x - a.radius_y).abs() < f64::EPSILON {
                    w.write_attribute("radius", &a.radius_x.to_string());
                } else {
                    w.write_attribute("radius", &format!("{} {}", a.radius_x, a.radius_y));
                }
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeDropShadow(a) => {
            write_filter_input(w, "in", &a.input);
            if a.dx != 0.0 {
                w.write_attribute("dx", &a.dx.to_string());
            }
            if a.dy != 0.0 {
                w.write_attribute("dy", &a.dy.to_string());
            }
            write_std_deviation(w, a.std_deviation_x, a.std_deviation_y);
            if let Some(ref c) = a.flood_color {
                w.write_attribute("flood-color", &c.to_hex());
            }
            if let Some(op) = a.flood_opacity {
                w.write_attribute("flood-opacity", &op.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeTurbulence(a) => {
            if let Some(ref tt) = a.turbulence_type {
                w.write_attribute("type", tt.to_svg());
            }
            if a.base_frequency_x != 0.0 || a.base_frequency_y != 0.0 {
                if (a.base_frequency_x - a.base_frequency_y).abs() < f64::EPSILON {
                    w.write_attribute("baseFrequency", &a.base_frequency_x.to_string());
                } else {
                    w.write_attribute(
                        "baseFrequency",
                        &format!("{} {}", a.base_frequency_x, a.base_frequency_y),
                    );
                }
            }
            if let Some(no) = a.num_octaves {
                w.write_attribute("numOctaves", &no.to_string());
            }
            if let Some(seed) = a.seed {
                w.write_attribute("seed", &seed.to_string());
            }
            if let Some(ref st) = a.stitch_tiles {
                w.write_attribute("stitchTiles", st.to_svg());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeDisplacementMap(a) => {
            write_filter_input(w, "in", &a.input);
            write_filter_input(w, "in2", &a.in2);
            if let Some(s) = a.scale {
                w.write_attribute("scale", &s.to_string());
            }
            if let Some(ref xc) = a.x_channel_selector {
                w.write_attribute("xChannelSelector", xc.to_svg());
            }
            if let Some(ref yc) = a.y_channel_selector {
                w.write_attribute("yChannelSelector", yc.to_svg());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeImage(a) => {
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            if let Some(ref par) = a.preserve_aspect_ratio {
                w.write_attribute("preserveAspectRatio", &par.to_svg());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeTile(a) => {
            write_filter_input(w, "in", &a.input);
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeComponentTransfer(a) => {
            write_filter_input(w, "in", &a.input);
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeConvolveMatrix(a) => {
            write_filter_input(w, "in", &a.input);
            if a.order_x != 3 || a.order_y != 3 {
                if a.order_x == a.order_y {
                    w.write_attribute("order", &a.order_x.to_string());
                } else {
                    w.write_attribute("order", &format!("{} {}", a.order_x, a.order_y));
                }
            }
            if !a.kernel_matrix.is_empty() {
                let km: Vec<String> = a.kernel_matrix.iter().map(|v| v.to_string()).collect();
                w.write_attribute("kernelMatrix", &km.join(" "));
            }
            if let Some(d) = a.divisor {
                w.write_attribute("divisor", &d.to_string());
            }
            if a.bias != 0.0 {
                w.write_attribute("bias", &a.bias.to_string());
            }
            if let Some(tx) = a.target_x {
                w.write_attribute("targetX", &tx.to_string());
            }
            if let Some(ty_) = a.target_y {
                w.write_attribute("targetY", &ty_.to_string());
            }
            if let Some(ref em) = a.edge_mode {
                w.write_attribute("edgeMode", em.to_svg());
            }
            if a.preserve_alpha {
                w.write_attribute("preserveAlpha", "true");
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeDiffuseLighting(a) => {
            write_filter_input(w, "in", &a.input);
            if let Some(ss) = a.surface_scale {
                w.write_attribute("surfaceScale", &ss.to_string());
            }
            if let Some(dc) = a.diffuse_constant {
                w.write_attribute("diffuseConstant", &dc.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FeSpecularLighting(a) => {
            write_filter_input(w, "in", &a.input);
            if let Some(ss) = a.surface_scale {
                w.write_attribute("surfaceScale", &ss.to_string());
            }
            if let Some(sc) = a.specular_constant {
                w.write_attribute("specularConstant", &sc.to_string());
            }
            if let Some(se) = a.specular_exponent {
                w.write_attribute("specularExponent", &se.to_string());
            }
            write_filter_primitive_common(w, &a.common);
        }
        SvgElement::FePointLight(a) => {
            w.write_attribute("x", &a.x.to_string());
            w.write_attribute("y", &a.y.to_string());
            w.write_attribute("z", &a.z.to_string());
        }
        SvgElement::FeDistantLight(a) => {
            w.write_attribute("azimuth", &a.azimuth.to_string());
            w.write_attribute("elevation", &a.elevation.to_string());
        }
        SvgElement::FeSpotLight(a) => {
            w.write_attribute("x", &a.x.to_string());
            w.write_attribute("y", &a.y.to_string());
            w.write_attribute("z", &a.z.to_string());
            w.write_attribute("pointsAtX", &a.points_at_x.to_string());
            w.write_attribute("pointsAtY", &a.points_at_y.to_string());
            w.write_attribute("pointsAtZ", &a.points_at_z.to_string());
            if let Some(se) = a.specular_exponent {
                w.write_attribute("specularExponent", &se.to_string());
            }
            if let Some(lca) = a.limiting_cone_angle {
                w.write_attribute("limitingConeAngle", &lca.to_string());
            }
        }
        SvgElement::Title(_) | SvgElement::Desc(_) | SvgElement::Metadata(_) => {
            // Text content handled separately.
        }
        SvgElement::StyleElement(a) => {
            if let Some(ref t) = a.style_type {
                w.write_attribute("type", t);
            }
            // Content handled separately via get_text_content.
        }
        SvgElement::Anchor(a) => {
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
            if let Some(ref target) = a.target {
                w.write_attribute("target", target);
            }
        }
        SvgElement::Switch(_) => {
            // No element-specific attributes; children handled by tree walk.
        }
        SvgElement::ForeignObject(a) => {
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            write_opt_len_attr(w, "width", &a.width);
            write_opt_len_attr(w, "height", &a.height);
        }
        SvgElement::Script(_) => {
            // Content handled separately via get_text_content.
        }

        // -- Phase 2.2: Inkscape-specific extensions --
        SvgElement::SodipodiNamedView(a) => {
            if let Some(ref v) = a.page_color {
                w.write_attribute("pagecolor", v);
            }
            if let Some(ref v) = a.border_color {
                w.write_attribute("bordercolor", v);
            }
            if let Some(ref v) = a.border_opacity {
                w.write_attribute("borderopacity", v);
            }
            if let Some(ref v) = a.inkscape_pageopacity {
                w.write_attribute("inkscape:pageopacity", v);
            }
            if let Some(ref v) = a.inkscape_pageshadow {
                w.write_attribute("inkscape:pageshadow", v);
            }
            if let Some(ref v) = a.inkscape_zoom {
                w.write_attribute("inkscape:zoom", v);
            }
            if let Some(ref v) = a.inkscape_cx {
                w.write_attribute("inkscape:cx", v);
            }
            if let Some(ref v) = a.inkscape_cy {
                w.write_attribute("inkscape:cy", v);
            }
            if let Some(ref v) = a.inkscape_window_width {
                w.write_attribute("inkscape:window-width", v);
            }
            if let Some(ref v) = a.inkscape_window_height {
                w.write_attribute("inkscape:window-height", v);
            }
            if let Some(ref v) = a.inkscape_window_x {
                w.write_attribute("inkscape:window-x", v);
            }
            if let Some(ref v) = a.inkscape_window_y {
                w.write_attribute("inkscape:window-y", v);
            }
            if let Some(ref v) = a.inkscape_window_maximized {
                w.write_attribute("inkscape:window-maximized", v);
            }
            if let Some(ref v) = a.inkscape_current_layer {
                w.write_attribute("inkscape:current-layer", v);
            }
            if let Some(ref v) = a.inkscape_document_units {
                w.write_attribute("inkscape:document-units", v);
            }
            if let Some(ref v) = a.show_grid {
                w.write_attribute("showgrid", v);
            }
            if let Some(ref v) = a.units {
                w.write_attribute("units", v);
            }
        }
        SvgElement::InkscapePage(a) => {
            if let Some(ref v) = a.x {
                w.write_attribute("x", v);
            }
            if let Some(ref v) = a.y {
                w.write_attribute("y", v);
            }
            if let Some(ref v) = a.width {
                w.write_attribute("width", v);
            }
            if let Some(ref v) = a.height {
                w.write_attribute("height", v);
            }
            if let Some(ref v) = a.inkscape_label {
                w.write_attribute("inkscape:label", v);
            }
        }
        SvgElement::InkscapeGrid(a) => {
            if let Some(ref v) = a.grid_type {
                w.write_attribute("type", v);
            }
            if let Some(ref v) = a.origin_x {
                w.write_attribute("originx", v);
            }
            if let Some(ref v) = a.origin_y {
                w.write_attribute("originy", v);
            }
            if let Some(ref v) = a.spacing_x {
                w.write_attribute("spacingx", v);
            }
            if let Some(ref v) = a.spacing_y {
                w.write_attribute("spacingy", v);
            }
            if let Some(ref v) = a.units {
                w.write_attribute("units", v);
            }
            if let Some(ref v) = a.visible {
                w.write_attribute("visible", v);
            }
            if let Some(ref v) = a.enabled {
                w.write_attribute("enabled", v);
            }
            if let Some(ref v) = a.snap_visible_only {
                w.write_attribute("inkscape:snap-visible-only", v);
            }
            if let Some(ref v) = a.empspacing {
                w.write_attribute("empspacing", v);
            }
            if let Some(ref v) = a.dotted {
                w.write_attribute("dotted", v);
            }
        }
        SvgElement::SodipodiGuide(a) => {
            if let Some(ref v) = a.position {
                w.write_attribute("position", v);
            }
            if let Some(ref v) = a.orientation {
                w.write_attribute("orientation", v);
            }
            if let Some(ref v) = a.inkscape_label {
                w.write_attribute("inkscape:label", v);
            }
            if let Some(ref v) = a.inkscape_locked {
                w.write_attribute("inkscape:locked", v);
            }
            if let Some(ref v) = a.inkscape_color {
                w.write_attribute("inkscape:color", v);
            }
        }
        // SVG 1.2 flowed text
        SvgElement::FlowRoot(a) => {
            if let Some(ref xs) = a.xml_space {
                w.write_attribute("xml:space", xs);
            }
        }
        SvgElement::FlowRegion(_) => {}
        SvgElement::FlowPara(_) => {}
        SvgElement::FlowSpan(_) => {}
        // SVG 2 mesh gradients
        SvgElement::MeshGradient(a) => {
            write_opt_len_attr(w, "x", &a.x);
            write_opt_len_attr(w, "y", &a.y);
            if let Some(ref gu) = a.gradient_units {
                w.write_attribute("gradientUnits", gu.to_svg());
            }
            if let Some(ref gt) = a.gradient_transform {
                let c = gt.as_coeffs();
                w.write_attribute(
                    "gradientTransform",
                    &format!(
                        "matrix({},{},{},{},{},{})",
                        c[0], c[1], c[2], c[3], c[4], c[5]
                    ),
                );
            }
            if let Some(ref href) = a.href {
                w.write_attribute("href", href);
            }
        }
        SvgElement::MeshRow(_) => {}
        SvgElement::MeshPatch(a) => {
            if let Some(ref sc) = a.stop_color {
                w.write_attribute("stop-color", &sc.to_hex());
            }
            if let Some(ref p) = a.path {
                w.write_attribute("path", p);
            }
        }
        // SVG font elements
        SvgElement::SvgFont(a) => {
            if let Some(v) = a.horiz_adv_x {
                w.write_attribute("horiz-adv-x", &v.to_string());
            }
            if let Some(v) = a.horiz_origin_x {
                w.write_attribute("horiz-origin-x", &v.to_string());
            }
            if let Some(v) = a.horiz_origin_y {
                w.write_attribute("horiz-origin-y", &v.to_string());
            }
        }
        SvgElement::FontFace(a) => {
            if let Some(ref v) = a.font_family {
                w.write_attribute("font-family", v);
            }
            if let Some(ref v) = a.font_style {
                w.write_attribute("font-style", v);
            }
            if let Some(ref v) = a.font_weight {
                w.write_attribute("font-weight", v);
            }
            if let Some(v) = a.units_per_em {
                w.write_attribute("units-per-em", &v.to_string());
            }
            if let Some(v) = a.ascent {
                w.write_attribute("ascent", &v.to_string());
            }
            if let Some(v) = a.descent {
                w.write_attribute("descent", &v.to_string());
            }
            if let Some(v) = a.cap_height {
                w.write_attribute("cap-height", &v.to_string());
            }
            if let Some(v) = a.x_height {
                w.write_attribute("x-height", &v.to_string());
            }
        }
        SvgElement::Glyph(a) | SvgElement::MissingGlyph(a) => {
            if let Some(ref u) = a.unicode {
                w.write_attribute("unicode", u);
            }
            if let Some(ref gn) = a.glyph_name {
                w.write_attribute("glyph-name", gn);
            }
            if let Some(ref d) = a.d {
                let s = d.to_svg_string();
                if !s.is_empty() {
                    w.write_attribute("d", &s);
                }
            }
            if let Some(v) = a.horiz_adv_x {
                w.write_attribute("horiz-adv-x", &v.to_string());
            }
        }
        SvgElement::HKern(a) | SvgElement::VKern(a) => {
            if let Some(ref v) = a.u1 {
                w.write_attribute("u1", v);
            }
            if let Some(ref v) = a.u2 {
                w.write_attribute("u2", v);
            }
            if let Some(ref v) = a.g1 {
                w.write_attribute("g1", v);
            }
            if let Some(ref v) = a.g2 {
                w.write_attribute("g2", v);
            }
            if let Some(v) = a.k {
                w.write_attribute("k", &v.to_string());
            }
        }
        // 3D Box (Inkscape SPBox3D)
        SvgElement::Box3d(a) => {
            if let Some(ref v) = a.vp_x {
                w.write_attribute("vp_x", v);
            }
            if let Some(ref v) = a.vp_y {
                w.write_attribute("vp_y", v);
            }
            if let Some(ref v) = a.vp_z {
                w.write_attribute("vp_z", v);
            }
            if let Some(ref v) = a.corner0 {
                w.write_attribute("inkscape:corner0", v);
            }
            if let Some(ref v) = a.corner1 {
                w.write_attribute("inkscape:corner1", v);
            }
            if let Some(ref v) = a.corner2 {
                w.write_attribute("inkscape:corner2", v);
            }
            if let Some(ref v) = a.corner3 {
                w.write_attribute("inkscape:corner3", v);
            }
            if let Some(ref v) = a.corner4 {
                w.write_attribute("inkscape:corner4", v);
            }
            if let Some(ref v) = a.corner5 {
                w.write_attribute("inkscape:corner5", v);
            }
            if let Some(ref v) = a.corner6 {
                w.write_attribute("inkscape:corner6", v);
            }
            if let Some(ref v) = a.corner7 {
                w.write_attribute("inkscape:corner7", v);
            }
        }
        SvgElement::Unknown(_) => {
            // Everything this element carried is preserved in the node's
            // extra-attribute map and written from there.
        }
    }
}

// ---------------------------------------------------------------------------
// Filter primitive helpers
// ---------------------------------------------------------------------------

fn write_filter_primitive_common(w: &mut AttrWriter, c: &FilterPrimitiveAttrs) {
    if let Some(ref r) = c.result {
        w.write_attribute("result", r);
    }
    write_opt_len_attr(w, "x", &c.x);
    write_opt_len_attr(w, "y", &c.y);
    write_opt_len_attr(w, "width", &c.width);
    write_opt_len_attr(w, "height", &c.height);
}

fn write_filter_input(w: &mut AttrWriter, attr: &str, input: &Option<FilterInput>) {
    if let Some(ref fi) = input {
        w.write_attribute(attr, &fi.to_svg());
    }
}

fn write_std_deviation(w: &mut AttrWriter, x: f64, y: f64) {
    if x != 0.0 || y != 0.0 {
        if (x - y).abs() < f64::EPSILON {
            w.write_attribute("stdDeviation", &x.to_string());
        } else {
            w.write_attribute("stdDeviation", &format!("{} {}", x, y));
        }
    }
}

// ---------------------------------------------------------------------------
// Length attribute helpers
// ---------------------------------------------------------------------------

fn write_len_attr(w: &mut AttrWriter, name: &str, len: &SvgLength) {
    w.write_attribute(name, &len.to_string());
}

fn write_len_attr_nonzero(w: &mut AttrWriter, name: &str, len: &SvgLength) {
    if len.value != 0.0 {
        w.write_attribute(name, &len.to_string());
    }
}

fn write_opt_len_attr(w: &mut AttrWriter, name: &str, len: &Option<SvgLength>) {
    if let Some(ref l) = len {
        w.write_attribute(name, &l.to_string());
    }
}

// ---------------------------------------------------------------------------
// Presentation attributes
// ---------------------------------------------------------------------------

fn write_presentation_attrs(p: &PresentationAttrs, w: &mut AttrWriter) {
    if let Some(ref color) = p.color {
        w.write_attribute("color", &color.to_hex());
    }
    if let Some(ref fill) = p.fill {
        w.write_attribute("fill", &fill.to_svg());
    }
    if let Some(ref stroke) = p.stroke {
        w.write_attribute("stroke", &stroke.to_svg());
    }
    if let Some(ref sw) = p.stroke_width {
        w.write_attribute("stroke-width", &sw.to_string());
    }
    if let Some(opacity) = p.opacity {
        w.write_attribute("opacity", &opacity.to_string());
    }
    if let Some(ref t) = p.transform {
        let c = t.as_coeffs();
        w.write_attribute(
            "transform",
            &format!(
                "matrix({},{},{},{},{},{})",
                c[0], c[1], c[2], c[3], c[4], c[5]
            ),
        );
    }
    if let Some(ref d) = p.display {
        w.write_attribute("display", d.to_svg());
    }
    if let Some(ref v) = p.visibility {
        w.write_attribute("visibility", v.to_svg());
    }
    if let Some(fo) = p.fill_opacity {
        w.write_attribute("fill-opacity", &fo.to_string());
    }
    if let Some(so) = p.stroke_opacity {
        w.write_attribute("stroke-opacity", &so.to_string());
    }
    if let Some(ref lc) = p.stroke_linecap {
        w.write_attribute("stroke-linecap", lc.to_svg());
    }
    if let Some(ref lj) = p.stroke_linejoin {
        w.write_attribute("stroke-linejoin", lj.to_svg());
    }
    if let Some(ml) = p.stroke_miterlimit {
        w.write_attribute("stroke-miterlimit", &ml.to_string());
    }
    if let Some(ref da) = p.stroke_dasharray {
        let s: Vec<String> = da.iter().map(|v| v.to_string()).collect();
        w.write_attribute("stroke-dasharray", &s.join(","));
    }
    if let Some(do_) = p.stroke_dashoffset {
        w.write_attribute("stroke-dashoffset", &do_.to_string());
    }
    if let Some(ref fr) = p.fill_rule {
        w.write_attribute("fill-rule", fr.to_svg());
    }
    if let Some(ref cr) = p.clip_rule {
        w.write_attribute("clip-rule", cr.to_svg());
    }
    // Clipping, masking, filter references
    if let Some(ref cp) = p.clip_path {
        w.write_attribute("clip-path", cp);
    }
    if let Some(ref m) = p.mask {
        w.write_attribute("mask", m);
    }
    if let Some(ref f) = p.filter {
        w.write_attribute("filter", f);
    }
    // Marker references
    if let Some(ref ms) = p.marker_start {
        w.write_attribute("marker-start", ms);
    }
    if let Some(ref mm) = p.marker_mid {
        w.write_attribute("marker-mid", mm);
    }
    if let Some(ref me) = p.marker_end {
        w.write_attribute("marker-end", me);
    }
    // Blend mode
    if let Some(ref bm) = p.mix_blend_mode {
        w.write_attribute("mix-blend-mode", bm.to_svg());
    }
    if let Some(isolate) = p.isolation {
        w.write_attribute("isolation", if isolate { "isolate" } else { "auto" });
    }
    // Font / text properties
    if let Some(ref ff) = p.font_family {
        w.write_attribute("font-family", ff);
    }
    if let Some(ref fs) = p.font_size {
        w.write_attribute("font-size", &fs.to_string());
    }
    if let Some(ref fw) = p.font_weight {
        w.write_attribute("font-weight", &fw.to_svg());
    }
    if let Some(ref fst) = p.font_style {
        w.write_attribute("font-style", fst.to_svg());
    }
    if let Some(ref ta) = p.text_anchor {
        w.write_attribute("text-anchor", ta.to_svg());
    }
    if let Some(ref ls) = p.letter_spacing {
        w.write_attribute("letter-spacing", &ls.to_string());
    }
    if let Some(ref ws) = p.word_spacing {
        w.write_attribute("word-spacing", &ws.to_string());
    }
    if let Some(ref td) = p.text_decoration {
        if !td.is_none() {
            w.write_attribute("text-decoration", &td.to_svg());
        }
    }
    // Vector effect
    if let Some(ref ve) = p.vector_effect {
        w.write_attribute("vector-effect", ve);
    }
}
