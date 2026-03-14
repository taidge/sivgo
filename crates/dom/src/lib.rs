//! sivgo-dom: SVG document object model for the Sivgo editor.
//!
//! Provides a tree-based representation of SVG documents with typed elements,
//! presentation attributes, and round-trip SVG parsing/serialization.
//!
//! # Quick start
//!
//! ```no_run
//! use sivgo_dom::{parse_svg, serialize_svg};
//!
//! let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
//!   <rect x="10" y="10" width="80" height="80" fill="red"/>
//! </svg>"#;
//!
//! let doc = parse_svg(svg).unwrap();
//! let xml = serialize_svg(&doc);
//! ```

pub mod document;
pub mod error;
pub mod node;
pub mod observer;
pub mod parser;
pub mod serializer;
pub mod types;

// Re-export the primary API at the crate root.
pub use document::SvgDocument;
pub use error::DomError;
pub use node::{
    AnchorAttrs,
    // 3D Box
    Box3dAttrs,
    // Phase 1
    CircleAttrs,
    // Phase 2: Clipping & masking
    ClipPathAttrs,
    DefsAttrs,
    DescAttrs,
    EllipseAttrs,
    FeBlendAttrs,
    FeColorMatrixAttrs,
    FeComponentTransferAttrs,
    FeCompositeAttrs,
    FeConvolveMatrixAttrs,
    FeDiffuseLightingAttrs,
    FeDisplacementMapAttrs,
    FeDistantLightAttrs,
    FeDropShadowAttrs,
    FeFloodAttrs,
    FeGaussianBlurAttrs,
    FeImageAttrs,
    FeMergeAttrs,
    FeMergeNodeAttrs,
    FeMorphologyAttrs,
    FeOffsetAttrs,
    FePointLightAttrs,
    FeSpecularLightingAttrs,
    FeSpotLightAttrs,
    FeTileAttrs,
    FeTurbulenceAttrs,
    // Phase 2: Filter
    FilterAttrs,
    FilterPrimitiveAttrs,
    FlowParaAttrs,
    FlowRegionAttrs,
    // SVG 1.2 Flowed text
    FlowRootAttrs,
    FlowSpanAttrs,
    ForeignObjectAttrs,
    GroupAttrs,
    // Phase 2: Image
    ImageAttrs,
    InkscapeGridAttrs,
    InkscapePageAttrs,
    LineAttrs,
    // Phase 2: Gradient & paint server attrs
    LinearGradientAttrs,
    // Phase 2: Marker
    MarkerAttrs,
    MaskAttrs,
    // SVG 2 Mesh gradients
    MeshGradientAttrs,
    MeshPatchAttrs,
    MeshRowAttrs,
    MetadataAttrs,
    // Phase 2.2: Inkscape-specific extensions
    NamedViewAttrs,
    NodeId,
    PathAttrs,
    PatternAttrs,
    PolygonAttrs,
    PolylineAttrs,
    PresentationAttrs,
    RadialGradientAttrs,
    RectAttrs,
    ScriptAttrs,
    SodipodiGuideAttrs,
    StopAttrs,
    // Phase 2: Style & Anchor
    StyleElementAttrs,
    SvgElement,
    SvgNode,
    SvgSvgAttrs,
    // Phase 2: Switch, ForeignObject, Script
    SwitchAttrs,
    SymbolAttrs,
    TSpanAttrs,
    // Phase 2: Text
    TextAttrs,
    TextPathAttrs,
    // Phase 2: Metadata
    TitleAttrs,
    // Phase 2: Use & Symbol
    UseAttrs,
};
pub use parser::parse_svg;
pub use serializer::serialize_svg;
pub use types::{
    // Phase 2
    Align,
    BlendMode,
    ChannelSelector,
    // Phase 1
    Color,
    ColorMatrixType,
    CompositeOperator,
    Display,
    EdgeMode,
    FillRule,
    FilterInput,
    FontStyle,
    FontWeight,
    GradientUnits,
    LengthUnit,
    LineCap,
    LineJoin,
    MarkerUnits,
    MeetOrSlice,
    MorphologyOperator,
    Orient,
    Paint,
    PreserveAspectRatio,
    SpreadMethod,
    StitchTiles,
    SvgLength,
    TextAnchor,
    TextDecoration,
    TurbulenceType,
    ViewBox,
    Visibility,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Basic parse + inspect ---

    #[test]
    fn parse_simple_rect() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
            <rect x="10" y="20" width="30" height="40" fill="red" stroke="#00ff00" stroke-width="2"/>
        </svg>"##;

        let doc = parse_svg(svg).unwrap();
        let root_id = doc.root().expect("no root");
        let root = doc.get_node(root_id).unwrap();

        // Root should be <svg>
        assert_eq!(root.element.tag_name(), "svg");
        match &root.element {
            SvgElement::Svg(a) => {
                assert_eq!(a.width, Some(SvgLength::new(200.0, LengthUnit::None)));
                assert_eq!(a.height, Some(SvgLength::new(100.0, LengthUnit::None)));
            }
            _ => panic!("expected SvgElement::Svg"),
        }

        // Single child: <rect>
        let children = doc.children(root_id);
        assert_eq!(children.len(), 1);

        let rect_id = children[0];
        let rect_node = doc.get_node(rect_id).unwrap();
        assert_eq!(rect_node.element.tag_name(), "rect");

        match &rect_node.element {
            SvgElement::Rect(a) => {
                assert_eq!(a.x.value, 10.0);
                assert_eq!(a.y.value, 20.0);
                assert_eq!(a.width.value, 30.0);
                assert_eq!(a.height.value, 40.0);
            }
            _ => panic!("expected SvgElement::Rect"),
        }

        // Presentation attrs
        assert_eq!(rect_node.presentation.fill, Some(Paint::Color(Color::RED)));
        assert_eq!(
            rect_node.presentation.stroke,
            Some(Paint::Color(Color::rgb(0, 255, 0)))
        );
        assert_eq!(
            rect_node.presentation.stroke_width,
            Some(SvgLength::new(2.0, LengthUnit::None))
        );
    }

    #[test]
    fn parse_circle_and_ellipse() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <circle cx="50" cy="50" r="30" fill="blue"/>
            <ellipse cx="100" cy="100" rx="40" ry="20" fill="green"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        assert_eq!(children.len(), 2);

        let circle = doc.get_node(children[0]).unwrap();
        match &circle.element {
            SvgElement::Circle(a) => {
                assert_eq!(a.cx.value, 50.0);
                assert_eq!(a.cy.value, 50.0);
                assert_eq!(a.r.value, 30.0);
            }
            _ => panic!("expected Circle"),
        }
        assert_eq!(circle.presentation.fill, Some(Paint::Color(Color::BLUE)));

        let ellipse = doc.get_node(children[1]).unwrap();
        match &ellipse.element {
            SvgElement::Ellipse(a) => {
                assert_eq!(a.cx.value, 100.0);
                assert_eq!(a.rx.value, 40.0);
                assert_eq!(a.ry.value, 20.0);
            }
            _ => panic!("expected Ellipse"),
        }
    }

    #[test]
    fn parse_line() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <line x1="0" y1="0" x2="100" y2="100" stroke="black" stroke-width="1"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let line_id = doc.children(root)[0];
        let line = doc.get_node(line_id).unwrap();

        match &line.element {
            SvgElement::Line(a) => {
                assert_eq!(a.x1.value, 0.0);
                assert_eq!(a.y2.value, 100.0);
            }
            _ => panic!("expected Line"),
        }
    }

    #[test]
    fn parse_polyline_and_polygon() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <polyline points="0,0 10,20 30,40" fill="none" stroke="red"/>
            <polygon points="50,50 60,70 80,90" fill="yellow"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        assert_eq!(children.len(), 2);

        let polyline = doc.get_node(children[0]).unwrap();
        match &polyline.element {
            SvgElement::Polyline(a) => {
                assert_eq!(a.points.len(), 3);
                assert_eq!(a.points[0], (0.0, 0.0));
                assert_eq!(a.points[1], (10.0, 20.0));
                assert_eq!(a.points[2], (30.0, 40.0));
            }
            _ => panic!("expected Polyline"),
        }

        let polygon = doc.get_node(children[1]).unwrap();
        match &polygon.element {
            SvgElement::Polygon(a) => {
                assert_eq!(a.points.len(), 3);
                assert_eq!(a.points[2], (80.0, 90.0));
            }
            _ => panic!("expected Polygon"),
        }
    }

    #[test]
    fn parse_path() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <path d="M 10 20 L 30 40 Z" fill="none" stroke="black"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let path_id = doc.children(root)[0];
        let path_node = doc.get_node(path_id).unwrap();

        match &path_node.element {
            SvgElement::Path(a) => {
                let svg_d = a.d.to_svg_string();
                assert!(!svg_d.is_empty(), "path d should not be empty");
            }
            _ => panic!("expected Path"),
        }
    }

    #[test]
    fn parse_group_with_children() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g transform="translate(10,20)">
                <rect x="0" y="0" width="50" height="50"/>
                <circle cx="25" cy="25" r="10"/>
            </g>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let g_node = doc.get_node(g_id).unwrap();

        assert_eq!(g_node.element.tag_name(), "g");
        assert!(g_node.presentation.transform.is_some());

        let g_children = doc.children(g_id);
        assert_eq!(g_children.len(), 2);
        assert_eq!(
            doc.get_node(g_children[0]).unwrap().element.tag_name(),
            "rect"
        );
        assert_eq!(
            doc.get_node(g_children[1]).unwrap().element.tag_name(),
            "circle"
        );

        // Parent tracking
        assert_eq!(doc.parent(g_children[0]), Some(g_id));
    }

    #[test]
    fn parse_defs() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <defs>
                <rect x="0" y="0" width="10" height="10"/>
            </defs>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let defs_id = doc.children(root)[0];
        let defs = doc.get_node(defs_id).unwrap();
        assert_eq!(defs.element.tag_name(), "defs");
        assert_eq!(doc.children(defs_id).len(), 1);
    }

    #[test]
    fn parse_presentation_attrs_detailed() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10"
                  fill="#ff0000" fill-opacity="0.5" fill-rule="evenodd"
                  stroke="blue" stroke-width="2px" stroke-opacity="0.8"
                  stroke-linecap="round" stroke-linejoin="bevel"
                  stroke-miterlimit="4" stroke-dasharray="5,10,15"
                  stroke-dashoffset="2"
                  opacity="0.9" display="inline" visibility="visible"
                  clip-rule="nonzero"/>
        </svg>"##;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let rect = doc.get_node(doc.children(root)[0]).unwrap();
        let p = &rect.presentation;

        assert_eq!(p.fill, Some(Paint::Color(Color::RED)));
        assert_eq!(p.fill_opacity, Some(0.5));
        assert_eq!(p.fill_rule, Some(FillRule::EvenOdd));
        assert_eq!(p.stroke, Some(Paint::Color(Color::BLUE)));
        assert_eq!(p.stroke_width, Some(SvgLength::new(2.0, LengthUnit::Px)));
        assert_eq!(p.stroke_opacity, Some(0.8));
        assert_eq!(p.stroke_linecap, Some(LineCap::Round));
        assert_eq!(p.stroke_linejoin, Some(LineJoin::Bevel));
        assert_eq!(p.stroke_miterlimit, Some(4.0));
        assert_eq!(p.stroke_dasharray, Some(vec![5.0, 10.0, 15.0]));
        assert_eq!(p.stroke_dashoffset, Some(2.0));
        assert_eq!(p.opacity, Some(0.9));
        assert_eq!(p.display, Some(Display::Inline));
        assert_eq!(p.visibility, Some(Visibility::Visible));
        assert_eq!(p.clip_rule, Some(FillRule::NonZero));
    }

    // --- Serialization ---

    #[test]
    fn serialize_empty_doc() {
        let doc = SvgDocument::new();
        let xml = serialize_svg(&doc);
        assert!(xml.is_empty() || xml.trim().is_empty());
    }

    #[test]
    fn serialize_simple_rect() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
            <rect x="10" y="20" width="30" height="40" fill="red"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);

        // The output should be valid SVG we can re-parse.
        assert!(xml.contains("<svg"));
        assert!(xml.contains("xmlns="));
        assert!(xml.contains("<rect"));
        assert!(xml.contains("width=\"30\""));
        assert!(xml.contains("fill=\"#ff0000\""));
    }

    // --- Round-trip ---

    #[test]
    fn roundtrip_complex_svg() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300" viewBox="0 0 400 300">
            <defs/>
            <g transform="translate(10,20)">
                <rect x="0" y="0" width="100" height="50" fill="#336699" stroke="black" stroke-width="2"/>
                <circle cx="50" cy="25" r="15" fill="white" opacity="0.8"/>
            </g>
            <line x1="0" y1="0" x2="400" y2="300" stroke="red" stroke-width="1"/>
            <ellipse cx="200" cy="150" rx="80" ry="40" fill="none" stroke="blue" stroke-dasharray="5,3"/>
            <polyline points="10,10 20,30 40,20" fill="none" stroke="green"/>
            <polygon points="100,100 150,50 200,100" fill="yellow"/>
            <path d="M 10 80 C 40 10 65 10 95 80" fill="none" stroke="purple" stroke-width="3"/>
        </svg>"##;

        let doc1 = parse_svg(svg).unwrap();
        let xml1 = serialize_svg(&doc1);

        // Re-parse the serialized output.
        let doc2 = parse_svg(&xml1).expect("failed to re-parse serialized SVG");
        let xml2 = serialize_svg(&doc2);

        // The second serialization should be identical to the first
        // (idempotent serialization).
        assert_eq!(xml1, xml2, "serialization is not idempotent");

        // Verify structural equivalence: same number of nodes.
        assert_eq!(doc1.len(), doc2.len());

        // Verify the root is an <svg> with expected attrs.
        let root2 = doc2.get_node(doc2.root().unwrap()).unwrap();
        match &root2.element {
            SvgElement::Svg(a) => {
                assert_eq!(a.width, Some(SvgLength::new(400.0, LengthUnit::None)));
                assert_eq!(a.height, Some(SvgLength::new(300.0, LengthUnit::None)));
                assert_eq!(a.view_box, Some(ViewBox::new(0.0, 0.0, 400.0, 300.0)));
            }
            _ => panic!("expected Svg root"),
        }
    }

    #[test]
    fn roundtrip_preserves_paint_none() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" fill="none"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);
        assert!(xml.contains("fill=\"none\""));

        let doc2 = parse_svg(&xml).unwrap();
        let root = doc2.root().unwrap();
        let rect = doc2.get_node(doc2.children(root)[0]).unwrap();
        assert_eq!(rect.presentation.fill, Some(Paint::None));
    }

    // --- Document manipulation ---

    #[test]
    fn add_and_remove_node() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs::default()));
        doc.append_child(svg_id, rect_id);

        assert_eq!(doc.children(svg_id).len(), 1);
        assert_eq!(doc.parent(rect_id), Some(svg_id));

        doc.remove_node(rect_id);
        assert_eq!(doc.children(svg_id).len(), 0);
        assert!(doc.get_node(rect_id).is_none());
    }

    #[test]
    fn traverse_pre_order() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g>
                <rect x="0" y="0" width="10" height="10"/>
                <circle cx="5" cy="5" r="3"/>
            </g>
            <line x1="0" y1="0" x2="10" y2="10"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let order = doc.traverse_pre_order(root);

        // svg, g, rect, circle, line = 5 nodes
        assert_eq!(order.len(), 5);

        let tags: Vec<&str> = order
            .iter()
            .map(|id| doc.get_node(*id).unwrap().element.tag_name())
            .collect();
        assert_eq!(tags, vec!["svg", "g", "rect", "circle", "line"]);
    }

    #[test]
    fn extra_attrs_preserved() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" id="my-rect" class="highlight" data-custom="42"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let rect = doc.get_node(doc.children(root)[0]).unwrap();

        // The `id` attribute is now parsed into `xml_id`.
        assert_eq!(rect.xml_id.as_deref(), Some("my-rect"));
        assert_eq!(rect.attributes.get("class"), Some(&"highlight".to_string()));
        assert_eq!(rect.attributes.get("data-custom"), Some(&"42".to_string()));

        // Round-trip preserves them.
        let xml = serialize_svg(&doc);
        assert!(xml.contains("id=\"my-rect\""));
        assert!(xml.contains("class=\"highlight\""));
        assert!(xml.contains("data-custom=\"42\""));
    }

    #[test]
    fn url_paint_roundtrip() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" fill="url(#grad1)"/>
        </svg>"##;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let rect = doc.get_node(doc.children(root)[0]).unwrap();
        assert_eq!(
            rect.presentation.fill,
            Some(Paint::Url("#grad1".to_string()))
        );

        let xml = serialize_svg(&doc);
        assert!(xml.contains("fill=\"url(#grad1)\""));
    }

    #[test]
    fn viewbox_roundtrip() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="10 20 300 400"/>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);
        assert!(xml.contains("viewBox=\"10 20 300 400\""));
    }

    #[test]
    fn transform_roundtrip() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <g transform="translate(10,20)">
                <rect x="0" y="0" width="10" height="10"/>
            </g>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let g_id = doc.children(root)[0];
        let g = doc.get_node(g_id).unwrap();

        // Verify the transform was parsed.
        let t = g.presentation.transform.as_ref().expect("no transform");
        let c = t.as_coeffs();
        // translate(10,20) => matrix(1,0,0,1,10,20)
        assert!((c[0] - 1.0).abs() < 1e-9);
        assert!((c[4] - 10.0).abs() < 1e-9);
        assert!((c[5] - 20.0).abs() < 1e-9);

        // Serialize and re-parse.
        let xml = serialize_svg(&doc);
        let doc2 = parse_svg(&xml).unwrap();
        let root2 = doc2.root().unwrap();
        let g2_id = doc2.children(root2)[0];
        let g2 = doc2.get_node(g2_id).unwrap();
        let t2 = g2
            .presentation
            .transform
            .as_ref()
            .expect("no transform after roundtrip");
        let c2 = t2.as_coeffs();
        for i in 0..6 {
            assert!(
                (c[i] - c2[i]).abs() < 1e-9,
                "transform coeff {} differs: {} vs {}",
                i,
                c[i],
                c2[i]
            );
        }
    }

    // --- data-* custom attribute preservation (2.3.4) ---

    #[test]
    fn data_custom_attrs_parsed_into_attributes_map() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" data-custom="foo" data-layer="bg"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let rect_id = doc.children(root)[0];
        let rect = doc.get_node(rect_id).unwrap();

        assert_eq!(
            rect.attributes.get("data-custom").map(|s| s.as_str()),
            Some("foo"),
            "data-custom should be preserved in the attributes map"
        );
        assert_eq!(
            rect.attributes.get("data-layer").map(|s| s.as_str()),
            Some("bg"),
            "data-layer should be preserved in the attributes map"
        );
    }

    #[test]
    fn data_custom_attrs_roundtrip() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect x="0" y="0" width="10" height="10" data-custom="foo" data-id="123"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);

        // Verify data-* attributes appear in serialized output
        assert!(
            xml.contains(r#"data-custom="foo""#),
            "data-custom should survive serialization, got: {}",
            xml
        );
        assert!(
            xml.contains(r#"data-id="123""#),
            "data-id should survive serialization, got: {}",
            xml
        );

        // Re-parse and verify they still exist
        let doc2 = parse_svg(&xml).unwrap();
        let root2 = doc2.root().unwrap();
        let rect2_id = doc2.children(root2)[0];
        let rect2 = doc2.get_node(rect2_id).unwrap();
        assert_eq!(
            rect2.attributes.get("data-custom").map(|s| s.as_str()),
            Some("foo")
        );
        assert_eq!(
            rect2.attributes.get("data-id").map(|s| s.as_str()),
            Some("123")
        );
    }

    // --- vector-effect="non-scaling-stroke" (2.3.14) ---

    #[test]
    fn vector_effect_parsed() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <line x1="0" y1="0" x2="100" y2="100" vector-effect="non-scaling-stroke"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let line_id = doc.children(root)[0];
        let line = doc.get_node(line_id).unwrap();

        assert_eq!(
            line.presentation.vector_effect.as_deref(),
            Some("non-scaling-stroke"),
            "vector-effect should be parsed into presentation attrs"
        );
        // Should NOT appear in the extra attributes map
        assert!(
            !line.attributes.contains_key("vector-effect"),
            "vector-effect should be handled, not in extra attrs"
        );
    }

    #[test]
    fn vector_effect_roundtrip() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <circle cx="50" cy="50" r="30" vector-effect="non-scaling-stroke" stroke="red"/>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);

        assert!(
            xml.contains(r#"vector-effect="non-scaling-stroke""#),
            "vector-effect should survive serialization, got: {}",
            xml
        );

        // Full round-trip: re-parse and check
        let doc2 = parse_svg(&xml).unwrap();
        let root2 = doc2.root().unwrap();
        let circle2_id = doc2.children(root2)[0];
        let circle2 = doc2.get_node(circle2_id).unwrap();
        assert_eq!(
            circle2.presentation.vector_effect.as_deref(),
            Some("non-scaling-stroke")
        );
    }

    // --- Phase 2.2: Inkscape-specific extensions ---

    #[test]
    fn parse_sodipodi_namedview() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd"
             width="800" height="600">
          <sodipodi:namedview
              id="namedview1"
              pagecolor="#ffffff"
              bordercolor="#666666"
              borderopacity="1.0"
              inkscape:pageopacity="0"
              inkscape:pageshadow="2"
              inkscape:zoom="1.5"
              inkscape:cx="400"
              inkscape:cy="300"
              inkscape:document-units="px"
              inkscape:current-layer="layer1"
              showgrid="true"
              units="px"
              inkscape:window-width="1920"
              inkscape:window-height="1080"
              inkscape:window-x="0"
              inkscape:window-y="0"
              inkscape:window-maximized="1"/>
        </svg>"##;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        assert_eq!(children.len(), 1);

        let nv_id = children[0];
        let nv = doc.get_node(nv_id).unwrap();
        assert_eq!(nv.element.tag_name(), "sodipodi:namedview");
        assert_eq!(nv.xml_id.as_deref(), Some("namedview1"));

        match &nv.element {
            SvgElement::SodipodiNamedView(a) => {
                assert_eq!(a.page_color.as_deref(), Some("#ffffff"), "page_color");
                assert_eq!(a.border_color.as_deref(), Some("#666666"), "border_color");
                assert_eq!(a.border_opacity.as_deref(), Some("1.0"));
                assert_eq!(a.inkscape_pageopacity.as_deref(), Some("0"));
                assert_eq!(a.inkscape_pageshadow.as_deref(), Some("2"));
                assert_eq!(a.inkscape_zoom.as_deref(), Some("1.5"));
                assert_eq!(a.inkscape_cx.as_deref(), Some("400"));
                assert_eq!(a.inkscape_cy.as_deref(), Some("300"));
                assert_eq!(a.inkscape_document_units.as_deref(), Some("px"));
                assert_eq!(a.inkscape_current_layer.as_deref(), Some("layer1"));
                assert_eq!(a.show_grid.as_deref(), Some("true"));
                assert_eq!(a.units.as_deref(), Some("px"));
                assert_eq!(a.inkscape_window_width.as_deref(), Some("1920"));
                assert_eq!(a.inkscape_window_height.as_deref(), Some("1080"));
                assert_eq!(a.inkscape_window_maximized.as_deref(), Some("1"));
            }
            _ => panic!("expected SodipodiNamedView"),
        }
    }

    #[test]
    fn parse_inkscape_page() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd">
          <sodipodi:namedview id="nv1">
            <inkscape:page id="page1" x="0" y="0" width="210" height="297" inkscape:label="Page 1"/>
            <inkscape:page id="page2" x="220" y="0" width="210" height="297" inkscape:label="Page 2"/>
          </sodipodi:namedview>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let nv_id = doc.children(root)[0];
        let pages = doc.children(nv_id);
        assert_eq!(pages.len(), 2);

        let p1 = doc.get_node(pages[0]).unwrap();
        assert_eq!(p1.element.tag_name(), "inkscape:page");
        match &p1.element {
            SvgElement::InkscapePage(a) => {
                assert_eq!(a.x.as_deref(), Some("0"));
                assert_eq!(a.y.as_deref(), Some("0"));
                assert_eq!(a.width.as_deref(), Some("210"));
                assert_eq!(a.height.as_deref(), Some("297"));
                assert_eq!(a.inkscape_label.as_deref(), Some("Page 1"));
            }
            _ => panic!("expected InkscapePage"),
        }

        let p2 = doc.get_node(pages[1]).unwrap();
        match &p2.element {
            SvgElement::InkscapePage(a) => {
                assert_eq!(a.x.as_deref(), Some("220"));
                assert_eq!(a.inkscape_label.as_deref(), Some("Page 2"));
            }
            _ => panic!("expected InkscapePage"),
        }
    }

    #[test]
    fn parse_inkscape_grid() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd">
          <sodipodi:namedview id="nv1">
            <inkscape:grid id="grid1" type="xygrid" originx="0" originy="0"
                           spacingx="10" spacingy="10" units="px"
                           visible="true" enabled="true" empspacing="5"/>
          </sodipodi:namedview>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let nv_id = doc.children(root)[0];
        let grids = doc.children(nv_id);
        assert_eq!(grids.len(), 1);

        let grid = doc.get_node(grids[0]).unwrap();
        assert_eq!(grid.element.tag_name(), "inkscape:grid");
        match &grid.element {
            SvgElement::InkscapeGrid(a) => {
                assert_eq!(a.grid_type.as_deref(), Some("xygrid"));
                assert_eq!(a.origin_x.as_deref(), Some("0"));
                assert_eq!(a.origin_y.as_deref(), Some("0"));
                assert_eq!(a.spacing_x.as_deref(), Some("10"));
                assert_eq!(a.spacing_y.as_deref(), Some("10"));
                assert_eq!(a.units.as_deref(), Some("px"));
                assert_eq!(a.visible.as_deref(), Some("true"));
                assert_eq!(a.enabled.as_deref(), Some("true"));
                assert_eq!(a.empspacing.as_deref(), Some("5"));
            }
            _ => panic!("expected InkscapeGrid"),
        }
    }

    #[test]
    fn parse_sodipodi_guide() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd">
          <sodipodi:namedview id="nv1">
            <sodipodi:guide id="guide1" position="100,0" orientation="0,1"
                            inkscape:label="Horizontal 100"
                            inkscape:locked="false"
                            inkscape:color="rgb(0,0,255)"/>
          </sodipodi:namedview>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let nv_id = doc.children(root)[0];
        let guides = doc.children(nv_id);
        assert_eq!(guides.len(), 1);

        let guide = doc.get_node(guides[0]).unwrap();
        assert_eq!(guide.element.tag_name(), "sodipodi:guide");
        match &guide.element {
            SvgElement::SodipodiGuide(a) => {
                assert_eq!(a.position.as_deref(), Some("100,0"));
                assert_eq!(a.orientation.as_deref(), Some("0,1"));
                assert_eq!(a.inkscape_label.as_deref(), Some("Horizontal 100"));
                assert_eq!(a.inkscape_locked.as_deref(), Some("false"));
                assert_eq!(a.inkscape_color.as_deref(), Some("rgb(0,0,255)"));
            }
            _ => panic!("expected SodipodiGuide"),
        }
    }

    #[test]
    fn parse_inkscape_layer_metadata() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape">
          <g inkscape:groupmode="layer" inkscape:label="Background" id="layer1">
            <rect x="0" y="0" width="100" height="100"/>
          </g>
          <g inkscape:groupmode="layer" inkscape:label="Foreground" id="layer2">
            <circle cx="50" cy="50" r="25"/>
          </g>
        </svg>"#;

        let doc = parse_svg(svg).unwrap();
        let root = doc.root().unwrap();
        let children = doc.children(root);
        assert_eq!(children.len(), 2);

        let layer1 = doc.get_node(children[0]).unwrap();
        assert_eq!(layer1.element.tag_name(), "g");
        assert_eq!(layer1.xml_id.as_deref(), Some("layer1"));
        match &layer1.element {
            SvgElement::Group(a) => {
                assert_eq!(a.inkscape_groupmode.as_deref(), Some("layer"));
                assert_eq!(a.inkscape_label.as_deref(), Some("Background"));
            }
            _ => panic!("expected Group"),
        }

        let layer2 = doc.get_node(children[1]).unwrap();
        match &layer2.element {
            SvgElement::Group(a) => {
                assert_eq!(a.inkscape_groupmode.as_deref(), Some("layer"));
                assert_eq!(a.inkscape_label.as_deref(), Some("Foreground"));
            }
            _ => panic!("expected Group"),
        }
    }

    #[test]
    fn roundtrip_inkscape_namedview_with_children() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd" width="800" height="600"><sodipodi:namedview id="nv1" pagecolor="#ffffff" inkscape:zoom="1.5" inkscape:document-units="px" showgrid="true"><inkscape:grid id="grid1" type="xygrid" spacingx="10" spacingy="10"/><sodipodi:guide id="guide1" position="100,0" orientation="0,1" inkscape:label="H100"/><inkscape:page id="page1" x="0" y="0" width="800" height="600" inkscape:label="Main"/></sodipodi:namedview><g inkscape:groupmode="layer" inkscape:label="Layer 1" id="layer1"><rect x="0" y="0" width="100" height="100"/></g></svg>"##;

        let doc1 = parse_svg(svg).unwrap();
        let xml1 = serialize_svg(&doc1);

        // Re-parse the serialized output.
        let doc2 = parse_svg(&xml1).expect("failed to re-parse serialized Inkscape SVG");
        let xml2 = serialize_svg(&doc2);

        // Idempotent serialization.
        assert_eq!(
            xml1, xml2,
            "Inkscape extension serialization is not idempotent"
        );

        // Verify structure.
        assert_eq!(doc1.len(), doc2.len());

        // Verify key round-tripped content.
        assert!(
            xml1.contains("sodipodi:namedview"),
            "namedview missing from output"
        );
        assert!(xml1.contains("inkscape:grid"), "grid missing from output");
        assert!(xml1.contains("sodipodi:guide"), "guide missing from output");
        assert!(xml1.contains("inkscape:page"), "page missing from output");
        assert!(
            xml1.contains("inkscape:groupmode=\"layer\""),
            "layer groupmode missing"
        );
        assert!(
            xml1.contains("inkscape:label=\"Layer 1\""),
            "layer label missing"
        );
        assert!(xml1.contains("pagecolor=\"#ffffff\""), "pagecolor missing");
        assert!(xml1.contains("inkscape:zoom=\"1.5\""), "zoom missing");
    }

    #[test]
    fn inkscape_layer_roundtrip_preserves_attrs() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"><g inkscape:groupmode="layer" inkscape:label="My Layer" id="layer1"><rect x="0" y="0" width="50" height="50" fill="red"/></g></svg>"#;

        let doc = parse_svg(svg).unwrap();
        let xml = serialize_svg(&doc);

        assert!(xml.contains("inkscape:groupmode=\"layer\""));
        assert!(xml.contains("inkscape:label=\"My Layer\""));

        // Verify roundtrip.
        let doc2 = parse_svg(&xml).unwrap();
        let xml2 = serialize_svg(&doc2);
        assert_eq!(xml, xml2, "layer metadata roundtrip not idempotent");
    }
}
