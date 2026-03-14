//! Parse SVG XML into an [`SvgDocument`].

use std::collections::HashMap;

use roxmltree::{Document as XmlDocument, Node as XmlNode};
use sivgo_math::{parse_transform, BezPath};

use crate::document::SvgDocument;
use crate::error::DomError;
use crate::node::*;
use crate::types::*;

/// Parse an SVG XML string into an [`SvgDocument`].
pub fn parse_svg(xml: &str) -> Result<SvgDocument, DomError> {
    let xml_doc = XmlDocument::parse(xml)?;
    let mut doc = SvgDocument::new();

    let root_elem = xml_doc.root_element();
    let root_id = parse_element(&root_elem, &mut doc)?;
    doc.set_root(root_id);

    Ok(doc)
}

/// Recursively parse an XML element into the document.
fn parse_element(xml_node: &XmlNode, doc: &mut SvgDocument) -> Result<NodeId, DomError> {
    let tag = xml_node.tag_name().name();

    let element = match tag {
        "svg" => SvgElement::Svg(parse_svg_attrs(xml_node)),
        "rect" => SvgElement::Rect(parse_rect_attrs(xml_node)),
        "circle" => SvgElement::Circle(parse_circle_attrs(xml_node)),
        "ellipse" => SvgElement::Ellipse(parse_ellipse_attrs(xml_node)),
        "line" => SvgElement::Line(parse_line_attrs(xml_node)),
        "polyline" => SvgElement::Polyline(parse_polyline_attrs(xml_node)),
        "polygon" => SvgElement::Polygon(parse_polygon_attrs(xml_node)),
        "path" => SvgElement::Path(parse_path_attrs(xml_node)),
        "g" => SvgElement::Group(parse_group_attrs(xml_node)),
        "defs" => SvgElement::Defs(DefsAttrs),
        // Phase 2: Gradient elements
        "linearGradient" => SvgElement::LinearGradient(parse_linear_gradient_attrs(xml_node)),
        "radialGradient" => SvgElement::RadialGradient(parse_radial_gradient_attrs(xml_node)),
        "stop" => SvgElement::Stop(parse_stop_attrs(xml_node)),
        // Phase 2: Pattern
        "pattern" => SvgElement::Pattern(parse_pattern_attrs(xml_node)),
        // Phase 2: ClipPath & Mask
        "clipPath" => SvgElement::ClipPath(parse_clip_path_attrs(xml_node)),
        "mask" => SvgElement::Mask(parse_mask_attrs(xml_node)),
        // Phase 2: Use & Symbol
        "use" => SvgElement::Use(parse_use_attrs(xml_node)),
        "symbol" => SvgElement::Symbol(parse_symbol_attrs(xml_node)),
        // Phase 2: Image
        "image" => SvgElement::Image(parse_image_attrs(xml_node)),
        // Phase 2: Text elements
        "text" => SvgElement::Text(parse_text_attrs(xml_node)),
        "tspan" => SvgElement::TSpan(parse_tspan_attrs(xml_node)),
        "textPath" => SvgElement::TextPath(parse_text_path_attrs(xml_node)),
        // Phase 2: Marker
        "marker" => SvgElement::Marker(parse_marker_attrs(xml_node)),
        // Phase 2: Filter container
        "filter" => SvgElement::Filter(parse_filter_attrs(xml_node)),
        // Phase 2: Filter primitives
        "feGaussianBlur" => SvgElement::FeGaussianBlur(parse_fe_gaussian_blur_attrs(xml_node)),
        "feOffset" => SvgElement::FeOffset(parse_fe_offset_attrs(xml_node)),
        "feFlood" => SvgElement::FeFlood(parse_fe_flood_attrs(xml_node)),
        "feBlend" => SvgElement::FeBlend(parse_fe_blend_attrs(xml_node)),
        "feComposite" => SvgElement::FeComposite(parse_fe_composite_attrs(xml_node)),
        "feColorMatrix" => SvgElement::FeColorMatrix(parse_fe_color_matrix_attrs(xml_node)),
        "feMerge" => SvgElement::FeMerge(parse_fe_merge_attrs(xml_node)),
        "feMergeNode" => SvgElement::FeMergeNode(parse_fe_merge_node_attrs(xml_node)),
        "feMorphology" => SvgElement::FeMorphology(parse_fe_morphology_attrs(xml_node)),
        "feDropShadow" => SvgElement::FeDropShadow(parse_fe_drop_shadow_attrs(xml_node)),
        "feTurbulence" => SvgElement::FeTurbulence(parse_fe_turbulence_attrs(xml_node)),
        "feDisplacementMap" => {
            SvgElement::FeDisplacementMap(parse_fe_displacement_map_attrs(xml_node))
        }
        "feImage" => SvgElement::FeImage(parse_fe_image_attrs(xml_node)),
        "feTile" => SvgElement::FeTile(parse_fe_tile_attrs(xml_node)),
        "feComponentTransfer" => {
            SvgElement::FeComponentTransfer(parse_fe_component_transfer_attrs(xml_node))
        }
        "feConvolveMatrix" => {
            SvgElement::FeConvolveMatrix(parse_fe_convolve_matrix_attrs(xml_node))
        }
        "feDiffuseLighting" => {
            SvgElement::FeDiffuseLighting(parse_fe_diffuse_lighting_attrs(xml_node))
        }
        "feSpecularLighting" => {
            SvgElement::FeSpecularLighting(parse_fe_specular_lighting_attrs(xml_node))
        }
        "fePointLight" => SvgElement::FePointLight(parse_fe_point_light_attrs(xml_node)),
        "feDistantLight" => SvgElement::FeDistantLight(parse_fe_distant_light_attrs(xml_node)),
        "feSpotLight" => SvgElement::FeSpotLight(parse_fe_spot_light_attrs(xml_node)),
        // Phase 2: Metadata elements
        "title" => SvgElement::Title(parse_title_attrs(xml_node)),
        "desc" => SvgElement::Desc(parse_desc_attrs(xml_node)),
        "metadata" => SvgElement::Metadata(parse_metadata_attrs(xml_node)),
        // Phase 2: Style element
        "style" => SvgElement::StyleElement(parse_style_element_attrs(xml_node)),
        // Phase 2: Anchor
        "a" => SvgElement::Anchor(parse_anchor_attrs(xml_node)),
        // Phase 2: Switch, ForeignObject, Script
        "switch" => SvgElement::Switch(SwitchAttrs),
        "foreignObject" => SvgElement::ForeignObject(parse_foreign_object_attrs(xml_node)),
        "script" => SvgElement::Script(parse_script_attrs(xml_node)),
        // Phase 2.2: Inkscape-specific extensions
        "namedview" => SvgElement::SodipodiNamedView(parse_named_view_attrs(xml_node)),
        "page" => SvgElement::InkscapePage(parse_inkscape_page_attrs(xml_node)),
        "grid" => SvgElement::InkscapeGrid(parse_inkscape_grid_attrs(xml_node)),
        "guide" => SvgElement::SodipodiGuide(parse_sodipodi_guide_attrs(xml_node)),
        // SVG 1.2 flowed text
        "flowRoot" => SvgElement::FlowRoot(parse_flow_root_attrs(xml_node)),
        "flowRegion" => SvgElement::FlowRegion(FlowRegionAttrs {}),
        "flowPara" => SvgElement::FlowPara(FlowParaAttrs {
            text_content: collect_text_content(xml_node),
        }),
        "flowSpan" => SvgElement::FlowSpan(FlowSpanAttrs {
            text_content: collect_text_content(xml_node),
        }),
        // SVG 2 mesh gradients
        "meshGradient" | "meshgradient" => {
            SvgElement::MeshGradient(parse_mesh_gradient_attrs(xml_node))
        }
        "meshrow" => SvgElement::MeshRow(MeshRowAttrs {}),
        "meshpatch" => SvgElement::MeshPatch(parse_mesh_patch_attrs(xml_node)),
        // SVG font elements (legacy)
        "font" => SvgElement::SvgFont(parse_svg_font_attrs(xml_node)),
        "font-face" => SvgElement::FontFace(parse_font_face_attrs(xml_node)),
        "glyph" => SvgElement::Glyph(parse_glyph_attrs(xml_node)),
        "missing-glyph" => SvgElement::MissingGlyph(parse_glyph_attrs(xml_node)),
        "hkern" => SvgElement::HKern(parse_kern_attrs(xml_node)),
        "vkern" => SvgElement::VKern(parse_kern_attrs(xml_node)),
        // 3D Box (Inkscape SPBox3D)
        "SPBox3D" => SvgElement::Box3d(parse_box3d_attrs(xml_node)),
        _ => {
            // Unknown element -- treat as a group so children are still parsed.
            SvgElement::Group(GroupAttrs::default())
        }
    };

    let id = NodeId::new();
    let xml_id = xml_node.attribute("id").map(|s| s.to_string());
    let presentation = parse_presentation_attrs(xml_node);
    let extra_attrs = collect_extra_attrs(xml_node, tag);
    let ns_decls = collect_namespace_decls(xml_node);

    let mut node = SvgNode::new(id, element);
    node.xml_id = xml_id;
    node.presentation = presentation;
    node.attributes = extra_attrs;
    node.namespaces = ns_decls;
    // Conditional processing attributes.
    node.required_features = parse_space_list_attr(xml_node, "requiredFeatures");
    node.required_extensions = parse_space_list_attr(xml_node, "requiredExtensions");
    node.system_language = parse_space_list_attr(xml_node, "systemLanguage");
    doc.insert_node(node);

    // Recurse into child elements.
    for child in xml_node.children().filter(|c| c.is_element()) {
        let child_id = parse_element(&child, doc)?;
        doc.append_child(id, child_id);
    }

    Ok(id)
}

// ---------------------------------------------------------------------------
// Phase 1: Element-specific attribute parsing
// ---------------------------------------------------------------------------

fn parse_svg_attrs(n: &XmlNode) -> SvgSvgAttrs {
    SvgSvgAttrs {
        width: n.attribute("width").and_then(|v| v.parse().ok()),
        height: n.attribute("height").and_then(|v| v.parse().ok()),
        view_box: n.attribute("viewBox").and_then(ViewBox::parse),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

fn parse_rect_attrs(n: &XmlNode) -> RectAttrs {
    RectAttrs {
        x: parse_len_attr(n, "x"),
        y: parse_len_attr(n, "y"),
        width: parse_len_attr(n, "width"),
        height: parse_len_attr(n, "height"),
        rx: parse_len_attr(n, "rx"),
        ry: parse_len_attr(n, "ry"),
    }
}

fn parse_circle_attrs(n: &XmlNode) -> CircleAttrs {
    CircleAttrs {
        cx: parse_len_attr(n, "cx"),
        cy: parse_len_attr(n, "cy"),
        r: parse_len_attr(n, "r"),
    }
}

fn parse_ellipse_attrs(n: &XmlNode) -> EllipseAttrs {
    EllipseAttrs {
        cx: parse_len_attr(n, "cx"),
        cy: parse_len_attr(n, "cy"),
        rx: parse_len_attr(n, "rx"),
        ry: parse_len_attr(n, "ry"),
    }
}

fn parse_line_attrs(n: &XmlNode) -> LineAttrs {
    LineAttrs {
        x1: parse_len_attr(n, "x1"),
        y1: parse_len_attr(n, "y1"),
        x2: parse_len_attr(n, "x2"),
        y2: parse_len_attr(n, "y2"),
    }
}

fn parse_polyline_attrs(n: &XmlNode) -> PolylineAttrs {
    PolylineAttrs {
        points: n.attribute("points").map(parse_points).unwrap_or_default(),
    }
}

fn parse_polygon_attrs(n: &XmlNode) -> PolygonAttrs {
    PolygonAttrs {
        points: n.attribute("points").map(parse_points).unwrap_or_default(),
    }
}

fn parse_path_attrs(n: &XmlNode) -> PathAttrs {
    PathAttrs {
        d: n.attribute("d")
            .and_then(|d| BezPath::from_svg_path(d).ok())
            .unwrap_or_default(),
        inkscape_path_effect: inkscape_attr(n, "path-effect"),
        inkscape_original_d: inkscape_attr(n, "original-d")
            .and_then(|d| BezPath::from_svg_path(&d).ok()),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Gradient elements
// ---------------------------------------------------------------------------

fn resolve_href(n: &XmlNode) -> Option<String> {
    n.attribute("href")
        .or_else(|| n.attribute(("http://www.w3.org/1999/xlink", "href")))
        .map(|s| s.to_string())
}

fn parse_linear_gradient_attrs(n: &XmlNode) -> LinearGradientAttrs {
    LinearGradientAttrs {
        x1: parse_opt_len_attr(n, "x1"),
        y1: parse_opt_len_attr(n, "y1"),
        x2: parse_opt_len_attr(n, "x2"),
        y2: parse_opt_len_attr(n, "y2"),
        gradient_units: n.attribute("gradientUnits").and_then(GradientUnits::parse),
        gradient_transform: n
            .attribute("gradientTransform")
            .and_then(|v| parse_transform(v).ok()),
        spread_method: n.attribute("spreadMethod").and_then(SpreadMethod::parse),
        href: resolve_href(n),
    }
}

fn parse_radial_gradient_attrs(n: &XmlNode) -> RadialGradientAttrs {
    RadialGradientAttrs {
        cx: parse_opt_len_attr(n, "cx"),
        cy: parse_opt_len_attr(n, "cy"),
        r: parse_opt_len_attr(n, "r"),
        fx: parse_opt_len_attr(n, "fx"),
        fy: parse_opt_len_attr(n, "fy"),
        gradient_units: n.attribute("gradientUnits").and_then(GradientUnits::parse),
        gradient_transform: n
            .attribute("gradientTransform")
            .and_then(|v| parse_transform(v).ok()),
        spread_method: n.attribute("spreadMethod").and_then(SpreadMethod::parse),
        href: resolve_href(n),
    }
}

fn parse_stop_attrs(n: &XmlNode) -> StopAttrs {
    let offset = n
        .attribute("offset")
        .and_then(|s| {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.trim().parse::<f64>().ok().map(|v| v / 100.0)
            } else {
                s.parse::<f64>().ok()
            }
        })
        .unwrap_or(0.0);

    StopAttrs {
        offset,
        stop_color: n.attribute("stop-color").and_then(Color::parse),
        stop_opacity: n.attribute("stop-opacity").and_then(|v| v.parse().ok()),
        path: n.attribute("path").map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Pattern
// ---------------------------------------------------------------------------

fn parse_pattern_attrs(n: &XmlNode) -> PatternAttrs {
    PatternAttrs {
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
        pattern_units: n.attribute("patternUnits").and_then(GradientUnits::parse),
        pattern_content_units: n
            .attribute("patternContentUnits")
            .and_then(GradientUnits::parse),
        pattern_transform: n
            .attribute("patternTransform")
            .and_then(|v| parse_transform(v).ok()),
        href: resolve_href(n),
        view_box: n.attribute("viewBox").and_then(ViewBox::parse),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: ClipPath & Mask
// ---------------------------------------------------------------------------

fn parse_clip_path_attrs(n: &XmlNode) -> ClipPathAttrs {
    ClipPathAttrs {
        clip_path_units: n.attribute("clipPathUnits").and_then(GradientUnits::parse),
    }
}

fn parse_mask_attrs(n: &XmlNode) -> MaskAttrs {
    MaskAttrs {
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
        mask_units: n.attribute("maskUnits").and_then(GradientUnits::parse),
        mask_content_units: n
            .attribute("maskContentUnits")
            .and_then(GradientUnits::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Use & Symbol
// ---------------------------------------------------------------------------

fn parse_use_attrs(n: &XmlNode) -> UseAttrs {
    UseAttrs {
        href: resolve_href(n),
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
    }
}

fn parse_symbol_attrs(n: &XmlNode) -> SymbolAttrs {
    SymbolAttrs {
        view_box: n.attribute("viewBox").and_then(ViewBox::parse),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Image
// ---------------------------------------------------------------------------

fn parse_image_attrs(n: &XmlNode) -> ImageAttrs {
    ImageAttrs {
        href: resolve_href(n),
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Text elements
// ---------------------------------------------------------------------------

fn collect_text_content(n: &XmlNode) -> String {
    let mut text = String::new();
    for child in n.children() {
        if child.is_text() {
            if let Some(t) = child.text() {
                text.push_str(t);
            }
        }
    }
    text
}

fn parse_length_list_attr(n: &XmlNode, attr: &str) -> Vec<SvgLength> {
    n.attribute(attr).map(parse_length_list).unwrap_or_default()
}

fn parse_text_attrs(n: &XmlNode) -> TextAttrs {
    TextAttrs {
        x: parse_length_list_attr(n, "x"),
        y: parse_length_list_attr(n, "y"),
        dx: parse_length_list_attr(n, "dx"),
        dy: parse_length_list_attr(n, "dy"),
        font_family: n.attribute("font-family").map(|s| s.to_string()),
        font_size: parse_opt_len_attr(n, "font-size"),
        font_weight: n.attribute("font-weight").and_then(FontWeight::parse),
        font_style: n.attribute("font-style").and_then(FontStyle::parse),
        text_anchor: n.attribute("text-anchor").and_then(TextAnchor::parse),
        text_content: collect_text_content(n),
    }
}

fn parse_tspan_attrs(n: &XmlNode) -> TSpanAttrs {
    TSpanAttrs {
        x: parse_length_list_attr(n, "x"),
        y: parse_length_list_attr(n, "y"),
        dx: parse_length_list_attr(n, "dx"),
        dy: parse_length_list_attr(n, "dy"),
        font_family: n.attribute("font-family").map(|s| s.to_string()),
        font_size: parse_opt_len_attr(n, "font-size"),
        font_weight: n.attribute("font-weight").and_then(FontWeight::parse),
        font_style: n.attribute("font-style").and_then(FontStyle::parse),
        text_anchor: n.attribute("text-anchor").and_then(TextAnchor::parse),
        text_content: collect_text_content(n),
    }
}

fn parse_text_path_attrs(n: &XmlNode) -> TextPathAttrs {
    TextPathAttrs {
        href: resolve_href(n),
        start_offset: parse_opt_len_attr(n, "startOffset"),
        text_content: collect_text_content(n),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Marker
// ---------------------------------------------------------------------------

fn parse_marker_attrs(n: &XmlNode) -> MarkerAttrs {
    MarkerAttrs {
        ref_x: parse_opt_len_attr(n, "refX"),
        ref_y: parse_opt_len_attr(n, "refY"),
        marker_width: parse_opt_len_attr(n, "markerWidth"),
        marker_height: parse_opt_len_attr(n, "markerHeight"),
        orient: n.attribute("orient").and_then(Orient::parse),
        marker_units: n.attribute("markerUnits").and_then(MarkerUnits::parse),
        view_box: n.attribute("viewBox").and_then(ViewBox::parse),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Filter container
// ---------------------------------------------------------------------------

fn parse_filter_attrs(n: &XmlNode) -> FilterAttrs {
    FilterAttrs {
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
        filter_units: n.attribute("filterUnits").and_then(GradientUnits::parse),
        primitive_units: n.attribute("primitiveUnits").and_then(GradientUnits::parse),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Filter primitive common attributes
// ---------------------------------------------------------------------------

fn parse_filter_primitive_common(n: &XmlNode) -> FilterPrimitiveAttrs {
    FilterPrimitiveAttrs {
        result: n.attribute("result").map(|s| s.to_string()),
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        width: parse_opt_len_attr(n, "width"),
        height: parse_opt_len_attr(n, "height"),
    }
}

fn parse_filter_input(n: &XmlNode, attr: &str) -> Option<FilterInput> {
    n.attribute(attr).map(FilterInput::parse)
}

/// Parse "stdDeviation" which can be one or two numbers (x, y).
fn parse_std_deviation(n: &XmlNode) -> (f64, f64) {
    n.attribute("stdDeviation")
        .map(|s| {
            let nums: Vec<f64> = s
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            match nums.len() {
                0 => (0.0, 0.0),
                1 => (nums[0], nums[0]),
                _ => (nums[0], nums[1]),
            }
        })
        .unwrap_or((0.0, 0.0))
}

// ---------------------------------------------------------------------------
// Phase 2: Filter primitive parsers
// ---------------------------------------------------------------------------

fn parse_fe_gaussian_blur_attrs(n: &XmlNode) -> FeGaussianBlurAttrs {
    let (sx, sy) = parse_std_deviation(n);
    FeGaussianBlurAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        std_deviation_x: sx,
        std_deviation_y: sy,
    }
}

fn parse_fe_offset_attrs(n: &XmlNode) -> FeOffsetAttrs {
    FeOffsetAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        dx: n
            .attribute("dx")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        dy: n
            .attribute("dy")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
    }
}

fn parse_fe_flood_attrs(n: &XmlNode) -> FeFloodAttrs {
    FeFloodAttrs {
        common: parse_filter_primitive_common(n),
        flood_color: n.attribute("flood-color").and_then(Color::parse),
        flood_opacity: n.attribute("flood-opacity").and_then(|v| v.parse().ok()),
    }
}

fn parse_fe_blend_attrs(n: &XmlNode) -> FeBlendAttrs {
    FeBlendAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        in2: parse_filter_input(n, "in2"),
        mode: n.attribute("mode").and_then(BlendMode::parse),
    }
}

fn parse_fe_composite_attrs(n: &XmlNode) -> FeCompositeAttrs {
    FeCompositeAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        in2: parse_filter_input(n, "in2"),
        operator: n.attribute("operator").and_then(CompositeOperator::parse),
        k1: n
            .attribute("k1")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        k2: n
            .attribute("k2")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        k3: n
            .attribute("k3")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        k4: n
            .attribute("k4")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
    }
}

fn parse_fe_color_matrix_attrs(n: &XmlNode) -> FeColorMatrixAttrs {
    FeColorMatrixAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        matrix_type: n.attribute("type").and_then(ColorMatrixType::parse),
        values: n.attribute("values").map(|s| s.to_string()),
    }
}

fn parse_fe_merge_attrs(n: &XmlNode) -> FeMergeAttrs {
    FeMergeAttrs {
        common: parse_filter_primitive_common(n),
    }
}

fn parse_fe_merge_node_attrs(n: &XmlNode) -> FeMergeNodeAttrs {
    FeMergeNodeAttrs {
        input: parse_filter_input(n, "in"),
    }
}

fn parse_fe_morphology_attrs(n: &XmlNode) -> FeMorphologyAttrs {
    let (rx, ry) = n
        .attribute("radius")
        .map(|s| {
            let nums: Vec<f64> = s
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            match nums.len() {
                0 => (0.0, 0.0),
                1 => (nums[0], nums[0]),
                _ => (nums[0], nums[1]),
            }
        })
        .unwrap_or((0.0, 0.0));

    FeMorphologyAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        operator: n.attribute("operator").and_then(MorphologyOperator::parse),
        radius_x: rx,
        radius_y: ry,
    }
}

fn parse_fe_drop_shadow_attrs(n: &XmlNode) -> FeDropShadowAttrs {
    let (sx, sy) = parse_std_deviation(n);
    FeDropShadowAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        dx: n
            .attribute("dx")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        dy: n
            .attribute("dy")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        std_deviation_x: sx,
        std_deviation_y: sy,
        flood_color: n.attribute("flood-color").and_then(Color::parse),
        flood_opacity: n.attribute("flood-opacity").and_then(|v| v.parse().ok()),
    }
}

fn parse_fe_turbulence_attrs(n: &XmlNode) -> FeTurbulenceAttrs {
    let (bfx, bfy) = n
        .attribute("baseFrequency")
        .map(|s| {
            let nums: Vec<f64> = s
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            match nums.len() {
                0 => (0.0, 0.0),
                1 => (nums[0], nums[0]),
                _ => (nums[0], nums[1]),
            }
        })
        .unwrap_or((0.0, 0.0));

    FeTurbulenceAttrs {
        common: parse_filter_primitive_common(n),
        turbulence_type: n.attribute("type").and_then(TurbulenceType::parse),
        base_frequency_x: bfx,
        base_frequency_y: bfy,
        num_octaves: n.attribute("numOctaves").and_then(|v| v.parse().ok()),
        seed: n.attribute("seed").and_then(|v| v.parse().ok()),
        stitch_tiles: n.attribute("stitchTiles").and_then(StitchTiles::parse),
    }
}

fn parse_fe_displacement_map_attrs(n: &XmlNode) -> FeDisplacementMapAttrs {
    FeDisplacementMapAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        in2: parse_filter_input(n, "in2"),
        scale: n.attribute("scale").and_then(|v| v.parse().ok()),
        x_channel_selector: n
            .attribute("xChannelSelector")
            .and_then(ChannelSelector::parse),
        y_channel_selector: n
            .attribute("yChannelSelector")
            .and_then(ChannelSelector::parse),
    }
}

fn parse_fe_image_attrs(n: &XmlNode) -> FeImageAttrs {
    FeImageAttrs {
        common: parse_filter_primitive_common(n),
        href: resolve_href(n),
        preserve_aspect_ratio: n
            .attribute("preserveAspectRatio")
            .and_then(PreserveAspectRatio::parse),
    }
}

fn parse_fe_tile_attrs(n: &XmlNode) -> FeTileAttrs {
    FeTileAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
    }
}

fn parse_fe_component_transfer_attrs(n: &XmlNode) -> FeComponentTransferAttrs {
    FeComponentTransferAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
    }
}

fn parse_fe_convolve_matrix_attrs(n: &XmlNode) -> FeConvolveMatrixAttrs {
    let (ox, oy) = n
        .attribute("order")
        .map(|s| {
            let nums: Vec<i32> = s
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            match nums.len() {
                0 => (3, 3),
                1 => (nums[0], nums[0]),
                _ => (nums[0], nums[1]),
            }
        })
        .unwrap_or((3, 3));

    let kernel_matrix: Vec<f64> = n
        .attribute("kernelMatrix")
        .map(|s| {
            s.split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect()
        })
        .unwrap_or_default();

    FeConvolveMatrixAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        order_x: ox,
        order_y: oy,
        kernel_matrix,
        divisor: n.attribute("divisor").and_then(|v| v.parse().ok()),
        bias: n
            .attribute("bias")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        target_x: n.attribute("targetX").and_then(|v| v.parse().ok()),
        target_y: n.attribute("targetY").and_then(|v| v.parse().ok()),
        edge_mode: n.attribute("edgeMode").and_then(EdgeMode::parse),
        preserve_alpha: n
            .attribute("preserveAlpha")
            .map(|v| v == "true")
            .unwrap_or(false),
    }
}

fn parse_fe_diffuse_lighting_attrs(n: &XmlNode) -> FeDiffuseLightingAttrs {
    FeDiffuseLightingAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        surface_scale: n.attribute("surfaceScale").and_then(|v| v.parse().ok()),
        diffuse_constant: n.attribute("diffuseConstant").and_then(|v| v.parse().ok()),
    }
}

fn parse_fe_specular_lighting_attrs(n: &XmlNode) -> FeSpecularLightingAttrs {
    FeSpecularLightingAttrs {
        common: parse_filter_primitive_common(n),
        input: parse_filter_input(n, "in"),
        surface_scale: n.attribute("surfaceScale").and_then(|v| v.parse().ok()),
        specular_constant: n.attribute("specularConstant").and_then(|v| v.parse().ok()),
        specular_exponent: n.attribute("specularExponent").and_then(|v| v.parse().ok()),
    }
}

fn parse_fe_point_light_attrs(n: &XmlNode) -> FePointLightAttrs {
    FePointLightAttrs {
        x: n.attribute("x").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        y: n.attribute("y").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        z: n.attribute("z").and_then(|v| v.parse().ok()).unwrap_or(0.0),
    }
}

fn parse_fe_distant_light_attrs(n: &XmlNode) -> FeDistantLightAttrs {
    FeDistantLightAttrs {
        azimuth: n
            .attribute("azimuth")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        elevation: n
            .attribute("elevation")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
    }
}

fn parse_fe_spot_light_attrs(n: &XmlNode) -> FeSpotLightAttrs {
    FeSpotLightAttrs {
        x: n.attribute("x").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        y: n.attribute("y").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        z: n.attribute("z").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        points_at_x: n
            .attribute("pointsAtX")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        points_at_y: n
            .attribute("pointsAtY")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        points_at_z: n
            .attribute("pointsAtZ")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0),
        specular_exponent: n.attribute("specularExponent").and_then(|v| v.parse().ok()),
        limiting_cone_angle: n
            .attribute("limitingConeAngle")
            .and_then(|v| v.parse().ok()),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Metadata elements
// ---------------------------------------------------------------------------

fn parse_title_attrs(n: &XmlNode) -> TitleAttrs {
    TitleAttrs {
        text_content: collect_text_content(n),
    }
}

fn parse_desc_attrs(n: &XmlNode) -> DescAttrs {
    DescAttrs {
        text_content: collect_text_content(n),
    }
}

fn parse_metadata_attrs(n: &XmlNode) -> MetadataAttrs {
    // Preserve raw text content from the metadata element.
    MetadataAttrs {
        raw_content: collect_text_content(n),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Style element
// ---------------------------------------------------------------------------

fn parse_style_element_attrs(n: &XmlNode) -> StyleElementAttrs {
    StyleElementAttrs {
        content: collect_text_content(n),
        style_type: n.attribute("type").map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Anchor
// ---------------------------------------------------------------------------

fn parse_anchor_attrs(n: &XmlNode) -> AnchorAttrs {
    AnchorAttrs {
        href: resolve_href(n),
        target: n.attribute("target").map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: ForeignObject & Script
// ---------------------------------------------------------------------------

fn parse_foreign_object_attrs(n: &XmlNode) -> ForeignObjectAttrs {
    ForeignObjectAttrs {
        x: n.attribute("x").and_then(|v| v.parse().ok()),
        y: n.attribute("y").and_then(|v| v.parse().ok()),
        width: n.attribute("width").and_then(|v| v.parse().ok()),
        height: n.attribute("height").and_then(|v| v.parse().ok()),
    }
}

fn parse_script_attrs(n: &XmlNode) -> ScriptAttrs {
    // Capture script text content but it's ignored for execution.
    let text = collect_text_content(n);
    ScriptAttrs {
        content: if text.is_empty() { None } else { Some(text) },
    }
}

// ---------------------------------------------------------------------------
// Phase 2.2: Inkscape-specific extensions
// ---------------------------------------------------------------------------

fn parse_group_attrs(n: &XmlNode) -> GroupAttrs {
    GroupAttrs {
        inkscape_groupmode: n
            .attribute(("http://www.inkscape.org/namespaces/inkscape", "groupmode"))
            .map(|s| s.to_string()),
        inkscape_label: n
            .attribute(("http://www.inkscape.org/namespaces/inkscape", "label"))
            .map(|s| s.to_string()),
    }
}

/// Helper to look up an attribute by its Inkscape namespace URI.
fn inkscape_attr(n: &XmlNode, local: &str) -> Option<String> {
    n.attribute(("http://www.inkscape.org/namespaces/inkscape", local))
        .map(|s| s.to_string())
}

/// Helper to look up an attribute by its Sodipodi namespace URI.
#[allow(dead_code)]
fn sodipodi_attr(n: &XmlNode, local: &str) -> Option<String> {
    n.attribute((
        "http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd",
        local,
    ))
    .map(|s| s.to_string())
}

fn parse_named_view_attrs(n: &XmlNode) -> NamedViewAttrs {
    NamedViewAttrs {
        page_color: n.attribute("pagecolor").map(|s| s.to_string()),
        border_color: n.attribute("bordercolor").map(|s| s.to_string()),
        border_opacity: n.attribute("borderopacity").map(|s| s.to_string()),
        inkscape_pageopacity: inkscape_attr(n, "pageopacity"),
        inkscape_pageshadow: inkscape_attr(n, "pageshadow"),
        inkscape_zoom: inkscape_attr(n, "zoom"),
        inkscape_cx: inkscape_attr(n, "cx"),
        inkscape_cy: inkscape_attr(n, "cy"),
        inkscape_window_width: inkscape_attr(n, "window-width"),
        inkscape_window_height: inkscape_attr(n, "window-height"),
        inkscape_window_x: inkscape_attr(n, "window-x"),
        inkscape_window_y: inkscape_attr(n, "window-y"),
        inkscape_window_maximized: inkscape_attr(n, "window-maximized"),
        inkscape_current_layer: inkscape_attr(n, "current-layer"),
        inkscape_document_units: inkscape_attr(n, "document-units"),
        show_grid: n.attribute("showgrid").map(|s| s.to_string()),
        units: n.attribute("units").map(|s| s.to_string()),
    }
}

fn parse_inkscape_page_attrs(n: &XmlNode) -> InkscapePageAttrs {
    InkscapePageAttrs {
        x: n.attribute("x").map(|s| s.to_string()),
        y: n.attribute("y").map(|s| s.to_string()),
        width: n.attribute("width").map(|s| s.to_string()),
        height: n.attribute("height").map(|s| s.to_string()),
        inkscape_label: inkscape_attr(n, "label"),
    }
}

fn parse_inkscape_grid_attrs(n: &XmlNode) -> InkscapeGridAttrs {
    InkscapeGridAttrs {
        grid_type: n.attribute("type").map(|s| s.to_string()),
        origin_x: n.attribute("originx").map(|s| s.to_string()),
        origin_y: n.attribute("originy").map(|s| s.to_string()),
        spacing_x: n.attribute("spacingx").map(|s| s.to_string()),
        spacing_y: n.attribute("spacingy").map(|s| s.to_string()),
        units: n.attribute("units").map(|s| s.to_string()),
        visible: n.attribute("visible").map(|s| s.to_string()),
        enabled: n.attribute("enabled").map(|s| s.to_string()),
        snap_visible_only: inkscape_attr(n, "snap-visible-only"),
        empspacing: n.attribute("empspacing").map(|s| s.to_string()),
        dotted: n.attribute("dotted").map(|s| s.to_string()),
    }
}

fn parse_sodipodi_guide_attrs(n: &XmlNode) -> SodipodiGuideAttrs {
    SodipodiGuideAttrs {
        position: n.attribute("position").map(|s| s.to_string()),
        orientation: n.attribute("orientation").map(|s| s.to_string()),
        inkscape_label: inkscape_attr(n, "label"),
        inkscape_locked: inkscape_attr(n, "locked"),
        inkscape_color: inkscape_attr(n, "color"),
    }
}

// ---------------------------------------------------------------------------
// SVG 1.2 Flowed Text
// ---------------------------------------------------------------------------

fn parse_flow_root_attrs(n: &XmlNode) -> FlowRootAttrs {
    FlowRootAttrs {
        xml_space: n
            .attribute(("http://www.w3.org/XML/1998/namespace", "space"))
            .or_else(|| n.attribute("xml:space"))
            .map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// SVG 2 Mesh Gradients
// ---------------------------------------------------------------------------

fn parse_mesh_gradient_attrs(n: &XmlNode) -> MeshGradientAttrs {
    MeshGradientAttrs {
        x: parse_opt_len_attr(n, "x"),
        y: parse_opt_len_attr(n, "y"),
        gradient_units: n.attribute("gradientUnits").and_then(GradientUnits::parse),
        gradient_transform: n
            .attribute("gradientTransform")
            .and_then(|v| parse_transform(v).ok()),
        href: resolve_href(n),
    }
}

fn parse_mesh_patch_attrs(n: &XmlNode) -> MeshPatchAttrs {
    MeshPatchAttrs {
        stop_color: n.attribute("stop-color").and_then(Color::parse),
        path: n.attribute("path").map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// SVG Font Elements (legacy)
// ---------------------------------------------------------------------------

fn parse_svg_font_attrs(n: &XmlNode) -> SvgFontAttrs {
    SvgFontAttrs {
        horiz_adv_x: n.attribute("horiz-adv-x").and_then(|v| v.parse().ok()),
        horiz_origin_x: n.attribute("horiz-origin-x").and_then(|v| v.parse().ok()),
        horiz_origin_y: n.attribute("horiz-origin-y").and_then(|v| v.parse().ok()),
    }
}

fn parse_font_face_attrs(n: &XmlNode) -> FontFaceAttrs {
    FontFaceAttrs {
        font_family: n.attribute("font-family").map(|s| s.to_string()),
        font_style: n.attribute("font-style").map(|s| s.to_string()),
        font_weight: n.attribute("font-weight").map(|s| s.to_string()),
        units_per_em: n.attribute("units-per-em").and_then(|v| v.parse().ok()),
        ascent: n.attribute("ascent").and_then(|v| v.parse().ok()),
        descent: n.attribute("descent").and_then(|v| v.parse().ok()),
        cap_height: n.attribute("cap-height").and_then(|v| v.parse().ok()),
        x_height: n.attribute("x-height").and_then(|v| v.parse().ok()),
    }
}

fn parse_glyph_attrs(n: &XmlNode) -> GlyphAttrs {
    GlyphAttrs {
        unicode: n.attribute("unicode").map(|s| s.to_string()),
        glyph_name: n.attribute("glyph-name").map(|s| s.to_string()),
        d: n.attribute("d")
            .and_then(|d| BezPath::from_svg_path(d).ok()),
        horiz_adv_x: n.attribute("horiz-adv-x").and_then(|v| v.parse().ok()),
    }
}

fn parse_kern_attrs(n: &XmlNode) -> KernAttrs {
    KernAttrs {
        u1: n.attribute("u1").map(|s| s.to_string()),
        u2: n.attribute("u2").map(|s| s.to_string()),
        g1: n.attribute("g1").map(|s| s.to_string()),
        g2: n.attribute("g2").map(|s| s.to_string()),
        k: n.attribute("k").and_then(|v| v.parse().ok()),
    }
}

// ---------------------------------------------------------------------------
// 3D Box (Inkscape SPBox3D)
// ---------------------------------------------------------------------------

fn parse_box3d_attrs(n: &XmlNode) -> Box3dAttrs {
    Box3dAttrs {
        vp_x: n
            .attribute("inkscape:perspectiveID")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("vp_x").map(|s| s.to_string())),
        vp_y: n.attribute("vp_y").map(|s| s.to_string()),
        vp_z: n.attribute("vp_z").map(|s| s.to_string()),
        corner0: n
            .attribute("inkscape:corner0")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner0").map(|s| s.to_string())),
        corner1: n
            .attribute("inkscape:corner1")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner1").map(|s| s.to_string())),
        corner2: n
            .attribute("inkscape:corner2")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner2").map(|s| s.to_string())),
        corner3: n
            .attribute("inkscape:corner3")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner3").map(|s| s.to_string())),
        corner4: n
            .attribute("inkscape:corner4")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner4").map(|s| s.to_string())),
        corner5: n
            .attribute("inkscape:corner5")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner5").map(|s| s.to_string())),
        corner6: n
            .attribute("inkscape:corner6")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner6").map(|s| s.to_string())),
        corner7: n
            .attribute("inkscape:corner7")
            .map(|s| s.to_string())
            .or_else(|| n.attribute("corner7").map(|s| s.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Presentation attributes
// ---------------------------------------------------------------------------

fn parse_presentation_attrs(n: &XmlNode) -> PresentationAttrs {
    PresentationAttrs {
        color: n.attribute("color").and_then(Color::parse),
        fill: n.attribute("fill").and_then(Paint::parse),
        stroke: n.attribute("stroke").and_then(Paint::parse),
        stroke_width: n.attribute("stroke-width").and_then(|v| v.parse().ok()),
        opacity: n.attribute("opacity").and_then(|v| v.parse().ok()),
        transform: n
            .attribute("transform")
            .and_then(|v| parse_transform(v).ok()),
        display: n.attribute("display").and_then(Display::parse),
        visibility: n.attribute("visibility").and_then(Visibility::parse),
        fill_opacity: n.attribute("fill-opacity").and_then(|v| v.parse().ok()),
        stroke_opacity: n.attribute("stroke-opacity").and_then(|v| v.parse().ok()),
        stroke_linecap: n.attribute("stroke-linecap").and_then(LineCap::parse),
        stroke_linejoin: n.attribute("stroke-linejoin").and_then(LineJoin::parse),
        stroke_miterlimit: n
            .attribute("stroke-miterlimit")
            .and_then(|v| v.parse().ok()),
        stroke_dasharray: n.attribute("stroke-dasharray").and_then(parse_dasharray),
        stroke_dashoffset: n
            .attribute("stroke-dashoffset")
            .and_then(|v| v.parse().ok()),
        fill_rule: n.attribute("fill-rule").and_then(FillRule::parse),
        clip_rule: n.attribute("clip-rule").and_then(FillRule::parse),
        // Clipping, masking, filter references
        clip_path: parse_url_pres_attr(n, "clip-path"),
        mask: parse_url_pres_attr(n, "mask"),
        filter: parse_url_pres_attr(n, "filter"),
        // Marker references
        marker_start: parse_url_pres_attr(n, "marker-start"),
        marker_mid: parse_url_pres_attr(n, "marker-mid"),
        marker_end: parse_url_pres_attr(n, "marker-end"),
        // Blend mode
        mix_blend_mode: n.attribute("mix-blend-mode").and_then(BlendMode::parse),
        // Isolation
        isolation: n.attribute("isolation").map(|v| v.trim() == "isolate"),
        // Font / text properties
        font_family: n
            .attribute("font-family")
            .map(|s| s.trim_matches(|c| c == '\'' || c == '"').to_string()),
        font_size: parse_opt_len_attr(n, "font-size"),
        font_weight: n.attribute("font-weight").and_then(FontWeight::parse),
        font_style: n.attribute("font-style").and_then(FontStyle::parse),
        text_anchor: n.attribute("text-anchor").and_then(TextAnchor::parse),
        letter_spacing: parse_opt_len_attr(n, "letter-spacing"),
        word_spacing: parse_opt_len_attr(n, "word-spacing"),
        text_decoration: n
            .attribute("text-decoration")
            .and_then(TextDecoration::parse),
        // Vector effect
        vector_effect: n.attribute("vector-effect").map(|v| v.to_string()),
    }
}

/// Parse a URL reference presentation attribute value (stored as raw string).
fn parse_url_pres_attr(n: &XmlNode, attr: &str) -> Option<String> {
    let val = n.attribute(attr)?.trim();
    if val.eq_ignore_ascii_case("none") || val.is_empty() {
        return None;
    }
    Some(val.to_string())
}

fn parse_dasharray(s: &str) -> Option<Vec<f64>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return None;
    }
    let vals: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_len_attr(n: &XmlNode, attr: &str) -> SvgLength {
    n.attribute(attr)
        .and_then(|v| v.parse().ok())
        .unwrap_or_default()
}

fn parse_opt_len_attr(n: &XmlNode, attr: &str) -> Option<SvgLength> {
    n.attribute(attr).and_then(|v| v.parse().ok())
}

/// Parse a space-separated list attribute (e.g., `requiredFeatures`, `systemLanguage`).
fn parse_space_list_attr(n: &XmlNode, attr: &str) -> Option<Vec<String>> {
    n.attribute(attr)
        .map(|v| v.split_whitespace().map(|s| s.to_string()).collect())
}

/// Collect attributes that are not handled by element-specific or presentation
/// parsing, so they survive a parse-serialize roundtrip.
fn collect_extra_attrs(n: &XmlNode, tag: &str) -> HashMap<String, String> {
    // Set of attribute names we already handle.
    let handled = handled_attrs(tag);
    let mut map = HashMap::new();
    for attr in n.attributes() {
        let local_name = attr.name();
        if handled.contains(&local_name) {
            continue;
        }
        // Reconstruct qualified name with namespace prefix for round-tripping.
        let qualified_name = match attr.namespace() {
            Some(ns) => {
                let prefix = ns_uri_to_prefix(ns);
                if prefix.is_empty() {
                    local_name.to_string()
                } else {
                    format!("{prefix}:{local_name}")
                }
            }
            None => local_name.to_string(),
        };
        // Also skip "xlink:href" since it's handled explicitly in element parsers.
        if qualified_name == "xlink:href" && handled.contains(&"xlink:href") {
            continue;
        }
        map.insert(qualified_name, attr.value().to_string());
    }
    map
}

/// Collect XML namespace declarations from an element for round-tripping.
fn collect_namespace_decls(n: &XmlNode) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for ns in n.namespaces() {
        let prefix = ns.name().unwrap_or("").to_string();
        let uri = ns.uri().to_string();
        // Skip the default SVG namespace if it's the standard one.
        if prefix.is_empty() && uri == "http://www.w3.org/2000/svg" {
            continue;
        }
        map.insert(prefix, uri);
    }
    map
}

/// Map well-known SVG namespace URIs to their conventional prefixes.
fn ns_uri_to_prefix(uri: &str) -> &'static str {
    match uri {
        "http://www.w3.org/1999/xlink" => "xlink",
        "http://www.inkscape.org/namespaces/inkscape" => "inkscape",
        "http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd" => "sodipodi",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#" => "rdf",
        "http://creativecommons.org/ns#" => "cc",
        "http://purl.org/dc/elements/1.1/" => "dc",
        "http://www.w3.org/2000/svg" => "",
        _ => "",
    }
}

/// Return the set of attribute names that are explicitly parsed for a given tag.
/// The `id` attribute is always handled (parsed into `xml_id`).
fn handled_attrs(tag: &str) -> &'static [&'static str] {
    match tag {
        "svg" => {
            const A: &[&str] = &[
                "id",
                "width",
                "height",
                "viewBox",
                "preserveAspectRatio",
                "xmlns",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "rect" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "rx",
                "ry",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "circle" => {
            const A: &[&str] = &[
                "id",
                "cx",
                "cy",
                "r",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "ellipse" => {
            const A: &[&str] = &[
                "id",
                "cx",
                "cy",
                "rx",
                "ry",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "line" => {
            const A: &[&str] = &[
                "id",
                "x1",
                "y1",
                "x2",
                "y2",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "polyline" | "polygon" => {
            const A: &[&str] = &[
                "id",
                "points",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "path" => {
            const A: &[&str] = &[
                "id",
                "d",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "g" => {
            const A: &[&str] = &[
                "id",
                "groupmode",
                "label",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "defs" => {
            const A: &[&str] = &[
                "id",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "linearGradient" => {
            const A: &[&str] = &[
                "id",
                "x1",
                "y1",
                "x2",
                "y2",
                "gradientUnits",
                "gradientTransform",
                "spreadMethod",
                "href",
                "xlink:href",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "radialGradient" => {
            const A: &[&str] = &[
                "id",
                "cx",
                "cy",
                "r",
                "fx",
                "fy",
                "gradientUnits",
                "gradientTransform",
                "spreadMethod",
                "href",
                "xlink:href",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "stop" => {
            const A: &[&str] = &[
                "id",
                "offset",
                "stop-color",
                "stop-opacity",
                "path",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "pattern" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "patternUnits",
                "patternContentUnits",
                "patternTransform",
                "href",
                "xlink:href",
                "viewBox",
                "preserveAspectRatio",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "clipPath" => {
            const A: &[&str] = &[
                "id",
                "clipPathUnits",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "mask" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "maskUnits",
                "maskContentUnits",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "use" => {
            const A: &[&str] = &[
                "id",
                "href",
                "xlink:href",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "symbol" => {
            const A: &[&str] = &[
                "id",
                "viewBox",
                "preserveAspectRatio",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "image" => {
            const A: &[&str] = &[
                "id",
                "href",
                "xlink:href",
                "x",
                "y",
                "width",
                "height",
                "preserveAspectRatio",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "text" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "dx",
                "dy",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "tspan" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "dx",
                "dy",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "textPath" => {
            const A: &[&str] = &[
                "id",
                "href",
                "xlink:href",
                "startOffset",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "marker" => {
            const A: &[&str] = &[
                "id",
                "refX",
                "refY",
                "markerWidth",
                "markerHeight",
                "orient",
                "markerUnits",
                "viewBox",
                "preserveAspectRatio",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "filter" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "filterUnits",
                "primitiveUnits",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feGaussianBlur" => {
            const A: &[&str] = &[
                "id",
                "in",
                "stdDeviation",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feOffset" => {
            const A: &[&str] = &[
                "id",
                "in",
                "dx",
                "dy",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feFlood" => {
            const A: &[&str] = &[
                "id",
                "flood-color",
                "flood-opacity",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feBlend" => {
            const A: &[&str] = &[
                "id",
                "in",
                "in2",
                "mode",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feComposite" => {
            const A: &[&str] = &[
                "id",
                "in",
                "in2",
                "operator",
                "k1",
                "k2",
                "k3",
                "k4",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feColorMatrix" => {
            const A: &[&str] = &[
                "id",
                "in",
                "type",
                "values",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feMerge" => {
            const A: &[&str] = &[
                "id",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feMergeNode" => {
            const A: &[&str] = &[
                "id",
                "in",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feMorphology" => {
            const A: &[&str] = &[
                "id",
                "in",
                "operator",
                "radius",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feDropShadow" => {
            const A: &[&str] = &[
                "id",
                "in",
                "dx",
                "dy",
                "stdDeviation",
                "flood-color",
                "flood-opacity",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feTurbulence" => {
            const A: &[&str] = &[
                "id",
                "type",
                "baseFrequency",
                "numOctaves",
                "seed",
                "stitchTiles",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feDisplacementMap" => {
            const A: &[&str] = &[
                "id",
                "in",
                "in2",
                "scale",
                "xChannelSelector",
                "yChannelSelector",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feImage" => {
            const A: &[&str] = &[
                "id",
                "href",
                "xlink:href",
                "preserveAspectRatio",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feTile" => {
            const A: &[&str] = &[
                "id",
                "in",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feComponentTransfer" => {
            const A: &[&str] = &[
                "id",
                "in",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feConvolveMatrix" => {
            const A: &[&str] = &[
                "id",
                "in",
                "order",
                "kernelMatrix",
                "divisor",
                "bias",
                "targetX",
                "targetY",
                "edgeMode",
                "preserveAlpha",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feDiffuseLighting" => {
            const A: &[&str] = &[
                "id",
                "in",
                "surfaceScale",
                "diffuseConstant",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feSpecularLighting" => {
            const A: &[&str] = &[
                "id",
                "in",
                "surfaceScale",
                "specularConstant",
                "specularExponent",
                "result",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "fePointLight" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "z",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feDistantLight" => {
            const A: &[&str] = &[
                "id",
                "azimuth",
                "elevation",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "feSpotLight" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "z",
                "pointsAtX",
                "pointsAtY",
                "pointsAtZ",
                "specularExponent",
                "limitingConeAngle",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "title" | "desc" | "metadata" => {
            const A: &[&str] = &[
                "id",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "style" => {
            const A: &[&str] = &[
                "id",
                "type",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "a" => {
            const A: &[&str] = &[
                "id",
                "href",
                "xlink:href",
                "target",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "switch" => {
            const A: &[&str] = &[
                "id",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "foreignObject" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "script" => {
            const A: &[&str] = &[
                "id",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        // Phase 2.2: Inkscape-specific extensions
        "namedview" => {
            const A: &[&str] = &[
                "id",
                "pagecolor",
                "bordercolor",
                "borderopacity",
                "pageopacity",
                "pageshadow",
                "zoom",
                "cx",
                "cy",
                "window-width",
                "window-height",
                "window-x",
                "window-y",
                "window-maximized",
                "current-layer",
                "document-units",
                "showgrid",
                "units",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "page" => {
            const A: &[&str] = &[
                "id",
                "x",
                "y",
                "width",
                "height",
                "label",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "grid" => {
            const A: &[&str] = &[
                "id",
                "type",
                "originx",
                "originy",
                "spacingx",
                "spacingy",
                "units",
                "visible",
                "enabled",
                "snap-visible-only",
                "empspacing",
                "dotted",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "guide" => {
            const A: &[&str] = &[
                "id",
                "position",
                "orientation",
                "label",
                "locked",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        "SPBox3D" => {
            const A: &[&str] = &[
                "id",
                "vp_x",
                "vp_y",
                "vp_z",
                "corner0",
                "corner1",
                "corner2",
                "corner3",
                "corner4",
                "corner5",
                "corner6",
                "corner7",
                "perspectiveID",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
        _ => {
            const A: &[&str] = &[
                "id",
                "color",
                "fill",
                "stroke",
                "stroke-width",
                "opacity",
                "transform",
                "display",
                "visibility",
                "fill-opacity",
                "stroke-opacity",
                "stroke-linecap",
                "stroke-linejoin",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-rule",
                "clip-rule",
                "clip-path",
                "mask",
                "filter",
                "marker-start",
                "marker-mid",
                "marker-end",
                "mix-blend-mode",
                "isolation",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-anchor",
                "letter-spacing",
                "word-spacing",
                "text-decoration",
                "vector-effect",
                "requiredFeatures",
                "requiredExtensions",
                "systemLanguage",
            ];
            A
        }
    }
}
