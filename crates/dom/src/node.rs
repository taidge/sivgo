//! SVG node, element variants, and presentation attributes.

use std::collections::HashMap;

use sivgo_math::{BezPath, Transform2D};

use crate::types::*;

// ---------------------------------------------------------------------------
// NodeId
// ---------------------------------------------------------------------------

/// Unique identifier for a node within an [`crate::SvgDocument`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub uuid::Uuid);

impl NodeId {
    /// Generate a new random (v4) node id.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// SvgNode
// ---------------------------------------------------------------------------

/// A single node in the SVG document tree.
#[derive(Debug, Clone)]
pub struct SvgNode {
    pub id: NodeId,
    /// The SVG `id` attribute value (e.g. `id="myRect"`), distinct from [`NodeId`].
    pub xml_id: Option<String>,
    pub element: SvgElement,
    /// Presentation attributes shared by all elements.
    pub presentation: PresentationAttrs,
    /// Non-standard or unrecognized attributes preserved for round-tripping.
    /// Keys use qualified names with namespace prefix (e.g. `"inkscape:label"`).
    pub attributes: HashMap<String, String>,
    /// XML namespace declarations on this element (prefix → URI).
    /// The default namespace uses an empty-string key.
    pub namespaces: HashMap<String, String>,
    /// SVG conditional processing attributes.
    pub required_features: Option<Vec<String>>,
    pub required_extensions: Option<Vec<String>>,
    pub system_language: Option<Vec<String>>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Whether this node is locked (cannot be selected or moved interactively).
    pub locked: bool,
    /// Whether this node is hidden (not rendered in the editor canvas).
    pub hidden: bool,
}

impl SvgNode {
    pub fn new(id: NodeId, element: SvgElement) -> Self {
        Self {
            id,
            xml_id: None,
            element,
            presentation: PresentationAttrs::default(),
            attributes: HashMap::new(),
            namespaces: HashMap::new(),
            required_features: None,
            required_extensions: None,
            system_language: None,
            parent: None,
            children: Vec::new(),
            locked: false,
            hidden: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PresentationAttrs
// ---------------------------------------------------------------------------

/// Presentation attributes that apply to every graphical SVG element.
#[derive(Debug, Clone, Default)]
pub struct PresentationAttrs {
    /// The CSS `color` property (used to resolve `currentColor`).
    pub color: Option<Color>,
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    pub stroke_width: Option<SvgLength>,
    pub opacity: Option<f64>,
    pub transform: Option<Transform2D>,
    pub display: Option<Display>,
    pub visibility: Option<Visibility>,
    pub fill_opacity: Option<f64>,
    pub stroke_opacity: Option<f64>,
    pub stroke_linecap: Option<LineCap>,
    pub stroke_linejoin: Option<LineJoin>,
    pub stroke_miterlimit: Option<f64>,
    pub stroke_dasharray: Option<Vec<f64>>,
    pub stroke_dashoffset: Option<f64>,
    pub fill_rule: Option<FillRule>,
    pub clip_rule: Option<FillRule>,

    // -- Clipping, masking, filter references --
    pub clip_path: Option<String>,
    pub mask: Option<String>,
    pub filter: Option<String>,

    // -- Marker references --
    pub marker_start: Option<String>,
    pub marker_mid: Option<String>,
    pub marker_end: Option<String>,

    // -- Blend mode --
    pub mix_blend_mode: Option<BlendMode>,

    // -- Isolation --
    pub isolation: Option<bool>,

    // -- Font / text properties --
    pub font_family: Option<String>,
    pub font_size: Option<SvgLength>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_anchor: Option<TextAnchor>,
    pub letter_spacing: Option<SvgLength>,
    pub word_spacing: Option<SvgLength>,
    pub text_decoration: Option<TextDecoration>,

    // -- Vector effect --
    pub vector_effect: Option<String>,
}

impl PresentationAttrs {
    /// Fill in unset inheritable properties from `parent`.
    ///
    /// SVG paint and text properties inherit down the tree, so a shape with no
    /// `fill` of its own takes the one set on an ancestor group. Properties
    /// that do not inherit — `opacity`, `transform`, `display`, `clip-path`,
    /// `mask`, `filter`, `mix-blend-mode`, `isolation`, the marker references
    /// and `vector-effect` — keep this element's own value, which is `None`
    /// when unset so each element starts from the initial value.
    ///
    /// Overlay `stronger` on top of this set.
    ///
    /// Every property `stronger` sets replaces this one's; properties it
    /// leaves unset keep the value here. Used to fold `<style>` element rules
    /// over plain presentation attributes, which they outrank.
    #[must_use]
    pub fn overridden_by(&self, stronger: &PresentationAttrs) -> PresentationAttrs {
        PresentationAttrs {
            color: stronger.color.or(self.color),
            fill: stronger.fill.clone().or_else(|| self.fill.clone()),
            stroke: stronger.stroke.clone().or_else(|| self.stroke.clone()),
            stroke_width: stronger.stroke_width.or(self.stroke_width),
            opacity: stronger.opacity.or(self.opacity),
            transform: stronger.transform.or(self.transform),
            display: stronger.display.or(self.display),
            visibility: stronger.visibility.or(self.visibility),
            fill_opacity: stronger.fill_opacity.or(self.fill_opacity),
            stroke_opacity: stronger.stroke_opacity.or(self.stroke_opacity),
            stroke_linecap: stronger.stroke_linecap.or(self.stroke_linecap),
            stroke_linejoin: stronger.stroke_linejoin.or(self.stroke_linejoin),
            stroke_miterlimit: stronger.stroke_miterlimit.or(self.stroke_miterlimit),
            stroke_dasharray: stronger
                .stroke_dasharray
                .clone()
                .or_else(|| self.stroke_dasharray.clone()),
            stroke_dashoffset: stronger.stroke_dashoffset.or(self.stroke_dashoffset),
            fill_rule: stronger.fill_rule.or(self.fill_rule),
            clip_rule: stronger.clip_rule.or(self.clip_rule),
            clip_path: stronger
                .clip_path
                .clone()
                .or_else(|| self.clip_path.clone()),
            mask: stronger.mask.clone().or_else(|| self.mask.clone()),
            filter: stronger.filter.clone().or_else(|| self.filter.clone()),
            marker_start: stronger
                .marker_start
                .clone()
                .or_else(|| self.marker_start.clone()),
            marker_mid: stronger
                .marker_mid
                .clone()
                .or_else(|| self.marker_mid.clone()),
            marker_end: stronger
                .marker_end
                .clone()
                .or_else(|| self.marker_end.clone()),
            mix_blend_mode: stronger.mix_blend_mode.or(self.mix_blend_mode),
            isolation: stronger.isolation.or(self.isolation),
            font_family: stronger
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            font_size: stronger.font_size.or(self.font_size),
            font_weight: stronger.font_weight.or(self.font_weight),
            font_style: stronger.font_style.or(self.font_style),
            text_anchor: stronger.text_anchor.or(self.text_anchor),
            letter_spacing: stronger.letter_spacing.or(self.letter_spacing),
            word_spacing: stronger.word_spacing.or(self.word_spacing),
            text_decoration: stronger.text_decoration.or(self.text_decoration),
            vector_effect: stronger
                .vector_effect
                .clone()
                .or_else(|| self.vector_effect.clone()),
        }
    }

    /// The inherited set matches `sivgo_style::is_inherited`.
    #[must_use]
    pub fn inheriting_from(&self, parent: &PresentationAttrs) -> PresentationAttrs {
        PresentationAttrs {
            color: self.color.or(parent.color),
            fill: self.fill.clone().or_else(|| parent.fill.clone()),
            stroke: self.stroke.clone().or_else(|| parent.stroke.clone()),
            stroke_width: self.stroke_width.or(parent.stroke_width),
            fill_opacity: self.fill_opacity.or(parent.fill_opacity),
            stroke_opacity: self.stroke_opacity.or(parent.stroke_opacity),
            stroke_linecap: self.stroke_linecap.or(parent.stroke_linecap),
            stroke_linejoin: self.stroke_linejoin.or(parent.stroke_linejoin),
            stroke_miterlimit: self.stroke_miterlimit.or(parent.stroke_miterlimit),
            stroke_dasharray: self
                .stroke_dasharray
                .clone()
                .or_else(|| parent.stroke_dasharray.clone()),
            stroke_dashoffset: self.stroke_dashoffset.or(parent.stroke_dashoffset),
            fill_rule: self.fill_rule.or(parent.fill_rule),
            clip_rule: self.clip_rule.or(parent.clip_rule),
            visibility: self.visibility.or(parent.visibility),
            font_family: self
                .font_family
                .clone()
                .or_else(|| parent.font_family.clone()),
            font_size: self.font_size.or(parent.font_size),
            font_weight: self.font_weight.or(parent.font_weight),
            font_style: self.font_style.or(parent.font_style),
            text_anchor: self.text_anchor.or(parent.text_anchor),
            letter_spacing: self.letter_spacing.or(parent.letter_spacing),
            word_spacing: self.word_spacing.or(parent.word_spacing),

            // Not inherited: taken from this element only.
            opacity: self.opacity,
            transform: self.transform,
            display: self.display,
            clip_path: self.clip_path.clone(),
            mask: self.mask.clone(),
            filter: self.filter.clone(),
            marker_start: self.marker_start.clone(),
            marker_mid: self.marker_mid.clone(),
            marker_end: self.marker_end.clone(),
            mix_blend_mode: self.mix_blend_mode,
            isolation: self.isolation,
            text_decoration: self.text_decoration,
            vector_effect: self.vector_effect.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// SvgElement
// ---------------------------------------------------------------------------

/// The element-specific data for each SVG element type.
#[derive(Debug, Clone)]
pub enum SvgElement {
    // Phase 1 elements
    Svg(SvgSvgAttrs),
    Rect(RectAttrs),
    Circle(CircleAttrs),
    Ellipse(EllipseAttrs),
    Line(LineAttrs),
    Polyline(PolylineAttrs),
    Polygon(PolygonAttrs),
    Path(PathAttrs),
    Group(GroupAttrs),
    Defs(DefsAttrs),

    // Phase 2: Gradient elements
    LinearGradient(LinearGradientAttrs),
    RadialGradient(RadialGradientAttrs),
    Stop(StopAttrs),

    // Phase 2: Pattern
    Pattern(PatternAttrs),

    // Phase 2: ClipPath & Mask
    ClipPath(ClipPathAttrs),
    Mask(MaskAttrs),

    // Phase 2: Use & Symbol
    Use(UseAttrs),
    Symbol(SymbolAttrs),

    // Phase 2: Image
    Image(ImageAttrs),

    // Phase 2: Text elements
    Text(TextAttrs),
    TSpan(TSpanAttrs),
    TextPath(TextPathAttrs),

    // Phase 2: Marker
    Marker(MarkerAttrs),

    // Phase 2: Filter container
    Filter(FilterAttrs),

    // Phase 2: Filter primitives
    FeGaussianBlur(FeGaussianBlurAttrs),
    FeOffset(FeOffsetAttrs),
    FeFlood(FeFloodAttrs),
    FeBlend(FeBlendAttrs),
    FeComposite(FeCompositeAttrs),
    FeColorMatrix(FeColorMatrixAttrs),
    FeMerge(FeMergeAttrs),
    FeMergeNode(FeMergeNodeAttrs),
    FeMorphology(FeMorphologyAttrs),
    FeDropShadow(FeDropShadowAttrs),
    FeTurbulence(FeTurbulenceAttrs),
    FeDisplacementMap(FeDisplacementMapAttrs),
    FeImage(FeImageAttrs),
    FeTile(FeTileAttrs),
    FeComponentTransfer(FeComponentTransferAttrs),
    FeConvolveMatrix(FeConvolveMatrixAttrs),
    FeDiffuseLighting(FeDiffuseLightingAttrs),
    FeSpecularLighting(FeSpecularLightingAttrs),
    FePointLight(FePointLightAttrs),
    FeDistantLight(FeDistantLightAttrs),
    FeSpotLight(FeSpotLightAttrs),

    // Phase 2: Metadata elements
    Title(TitleAttrs),
    Desc(DescAttrs),
    Metadata(MetadataAttrs),

    // Phase 2: Style element
    StyleElement(StyleElementAttrs),

    // Phase 2: Anchor
    Anchor(AnchorAttrs),

    // Phase 2: Switch, ForeignObject, Script
    Switch(SwitchAttrs),
    ForeignObject(ForeignObjectAttrs),
    Script(ScriptAttrs),

    // Phase 2.2: Inkscape-specific extensions
    SodipodiNamedView(NamedViewAttrs),
    InkscapePage(InkscapePageAttrs),
    InkscapeGrid(InkscapeGridAttrs),
    SodipodiGuide(SodipodiGuideAttrs),

    // SVG 1.2 flowed text
    FlowRoot(FlowRootAttrs),
    FlowRegion(FlowRegionAttrs),
    FlowPara(FlowParaAttrs),
    FlowSpan(FlowSpanAttrs),

    // SVG 2 mesh gradients
    MeshGradient(MeshGradientAttrs),
    MeshRow(MeshRowAttrs),
    MeshPatch(MeshPatchAttrs),

    // SVG font elements (deprecated, for legacy compatibility)
    SvgFont(SvgFontAttrs),
    FontFace(FontFaceAttrs),
    Glyph(GlyphAttrs),
    MissingGlyph(GlyphAttrs),
    HKern(KernAttrs),
    VKern(KernAttrs),

    // 3D Box (Inkscape SPBox3D)
    Box3d(Box3dAttrs),

    /// An element the parser does not model.
    ///
    /// Kept verbatim so a document survives a round trip. Mapping these onto
    /// `Group` destroyed anything the editor did not understand: `<metadata>`
    /// contents, `<inkscape:path-effect>` definitions and `<foreignObject>`
    /// bodies all came back as `<g>`.
    Unknown(UnknownAttrs),
}

/// An element whose tag the parser does not recognize.
#[derive(Debug, Clone, Default)]
pub struct UnknownAttrs {
    /// The element's name exactly as it appeared, prefix included.
    pub qualified_name: String,
    /// Any text directly inside the element, kept so it survives a round trip.
    pub text_content: String,
}

impl SvgElement {
    /// Returns `true` for shape elements: Rect, Circle, Ellipse, Line, Polyline, Polygon, Path.
    pub fn is_shape(&self) -> bool {
        matches!(
            self,
            SvgElement::Rect(_)
                | SvgElement::Circle(_)
                | SvgElement::Ellipse(_)
                | SvgElement::Line(_)
                | SvgElement::Polyline(_)
                | SvgElement::Polygon(_)
                | SvgElement::Path(_)
        )
    }

    /// Returns `true` for the Group element.
    pub fn is_group(&self) -> bool {
        matches!(self, SvgElement::Group(_))
    }

    /// Returns `true` for container elements: Svg, Group, Defs, Symbol, Anchor,
    /// ClipPath, Mask, Marker, Pattern, Filter, SodipodiNamedView.
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            SvgElement::Svg(_)
                | SvgElement::Group(_)
                | SvgElement::Defs(_)
                | SvgElement::Symbol(_)
                | SvgElement::Anchor(_)
                | SvgElement::ClipPath(_)
                | SvgElement::Mask(_)
                | SvgElement::Marker(_)
                | SvgElement::Pattern(_)
                | SvgElement::Filter(_)
                | SvgElement::Switch(_)
                | SvgElement::ForeignObject(_)
                | SvgElement::SodipodiNamedView(_)
        )
    }

    /// Returns `true` for paint server elements: LinearGradient, RadialGradient, Pattern, MeshGradient.
    pub fn is_paint_server(&self) -> bool {
        matches!(
            self,
            SvgElement::LinearGradient(_)
                | SvgElement::RadialGradient(_)
                | SvgElement::Pattern(_)
                | SvgElement::MeshGradient(_)
        )
    }

    /// Returns `true` for text elements: Text, TSpan, TextPath.
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            SvgElement::Text(_) | SvgElement::TSpan(_) | SvgElement::TextPath(_)
        )
    }

    /// Returns `true` for filter primitive elements (all Fe* variants).
    pub fn is_filter_primitive(&self) -> bool {
        matches!(
            self,
            SvgElement::FeGaussianBlur(_)
                | SvgElement::FeOffset(_)
                | SvgElement::FeFlood(_)
                | SvgElement::FeBlend(_)
                | SvgElement::FeComposite(_)
                | SvgElement::FeColorMatrix(_)
                | SvgElement::FeMerge(_)
                | SvgElement::FeMergeNode(_)
                | SvgElement::FeMorphology(_)
                | SvgElement::FeDropShadow(_)
                | SvgElement::FeTurbulence(_)
                | SvgElement::FeDisplacementMap(_)
                | SvgElement::FeImage(_)
                | SvgElement::FeTile(_)
                | SvgElement::FeComponentTransfer(_)
                | SvgElement::FeConvolveMatrix(_)
                | SvgElement::FeDiffuseLighting(_)
                | SvgElement::FeSpecularLighting(_)
                | SvgElement::FePointLight(_)
                | SvgElement::FeDistantLight(_)
                | SvgElement::FeSpotLight(_)
        )
    }

    /// Returns `true` for renderable elements: shapes, text, image, use, group, svg, anchor, flowRoot, box3d.
    pub fn is_renderable(&self) -> bool {
        self.is_shape()
            || self.is_text()
            || self.is_flow_text()
            || matches!(
                self,
                SvgElement::Image(_)
                    | SvgElement::Use(_)
                    | SvgElement::Group(_)
                    | SvgElement::Svg(_)
                    | SvgElement::Anchor(_)
                    | SvgElement::Box3d(_)
            )
    }

    /// Returns `true` for SVG 1.2 flowed text elements: FlowRoot, FlowRegion, FlowPara, FlowSpan.
    pub fn is_flow_text(&self) -> bool {
        matches!(
            self,
            SvgElement::FlowRoot(_)
                | SvgElement::FlowRegion(_)
                | SvgElement::FlowPara(_)
                | SvgElement::FlowSpan(_)
        )
    }

    /// SVG tag name for this element.
    pub fn tag_name(&self) -> &str {
        match self {
            SvgElement::Svg(_) => "svg",
            SvgElement::Rect(_) => "rect",
            SvgElement::Circle(_) => "circle",
            SvgElement::Ellipse(_) => "ellipse",
            SvgElement::Line(_) => "line",
            SvgElement::Polyline(_) => "polyline",
            SvgElement::Polygon(_) => "polygon",
            SvgElement::Path(_) => "path",
            SvgElement::Group(_) => "g",
            SvgElement::Defs(_) => "defs",
            SvgElement::LinearGradient(_) => "linearGradient",
            SvgElement::RadialGradient(_) => "radialGradient",
            SvgElement::Stop(_) => "stop",
            SvgElement::Pattern(_) => "pattern",
            SvgElement::ClipPath(_) => "clipPath",
            SvgElement::Mask(_) => "mask",
            SvgElement::Use(_) => "use",
            SvgElement::Symbol(_) => "symbol",
            SvgElement::Image(_) => "image",
            SvgElement::Text(_) => "text",
            SvgElement::TSpan(_) => "tspan",
            SvgElement::TextPath(_) => "textPath",
            SvgElement::Marker(_) => "marker",
            SvgElement::Filter(_) => "filter",
            SvgElement::FeGaussianBlur(_) => "feGaussianBlur",
            SvgElement::FeOffset(_) => "feOffset",
            SvgElement::FeFlood(_) => "feFlood",
            SvgElement::FeBlend(_) => "feBlend",
            SvgElement::FeComposite(_) => "feComposite",
            SvgElement::FeColorMatrix(_) => "feColorMatrix",
            SvgElement::FeMerge(_) => "feMerge",
            SvgElement::FeMergeNode(_) => "feMergeNode",
            SvgElement::FeMorphology(_) => "feMorphology",
            SvgElement::FeDropShadow(_) => "feDropShadow",
            SvgElement::FeTurbulence(_) => "feTurbulence",
            SvgElement::FeDisplacementMap(_) => "feDisplacementMap",
            SvgElement::FeImage(_) => "feImage",
            SvgElement::FeTile(_) => "feTile",
            SvgElement::FeComponentTransfer(_) => "feComponentTransfer",
            SvgElement::FeConvolveMatrix(_) => "feConvolveMatrix",
            SvgElement::FeDiffuseLighting(_) => "feDiffuseLighting",
            SvgElement::FeSpecularLighting(_) => "feSpecularLighting",
            SvgElement::FePointLight(_) => "fePointLight",
            SvgElement::FeDistantLight(_) => "feDistantLight",
            SvgElement::FeSpotLight(_) => "feSpotLight",
            SvgElement::Title(_) => "title",
            SvgElement::Desc(_) => "desc",
            SvgElement::Metadata(_) => "metadata",
            SvgElement::StyleElement(_) => "style",
            SvgElement::Anchor(_) => "a",
            SvgElement::Switch(_) => "switch",
            SvgElement::ForeignObject(_) => "foreignObject",
            SvgElement::Script(_) => "script",
            SvgElement::SodipodiNamedView(_) => "sodipodi:namedview",
            SvgElement::InkscapePage(_) => "inkscape:page",
            SvgElement::InkscapeGrid(_) => "inkscape:grid",
            SvgElement::SodipodiGuide(_) => "sodipodi:guide",
            SvgElement::FlowRoot(_) => "flowRoot",
            SvgElement::FlowRegion(_) => "flowRegion",
            SvgElement::FlowPara(_) => "flowPara",
            SvgElement::FlowSpan(_) => "flowSpan",
            SvgElement::MeshGradient(_) => "meshGradient",
            SvgElement::MeshRow(_) => "meshrow",
            SvgElement::MeshPatch(_) => "meshpatch",
            SvgElement::SvgFont(_) => "font",
            SvgElement::FontFace(_) => "font-face",
            SvgElement::Glyph(_) => "glyph",
            SvgElement::MissingGlyph(_) => "missing-glyph",
            SvgElement::HKern(_) => "hkern",
            SvgElement::VKern(_) => "vkern",
            SvgElement::Box3d(_) => "SPBox3D",
            SvgElement::Unknown(a) => a.qualified_name.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-element attribute structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SvgSvgAttrs {
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
    pub view_box: Option<ViewBox>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

#[derive(Debug, Clone)]
pub struct RectAttrs {
    pub x: SvgLength,
    pub y: SvgLength,
    pub width: SvgLength,
    pub height: SvgLength,
    pub rx: SvgLength,
    pub ry: SvgLength,
}

impl Default for RectAttrs {
    fn default() -> Self {
        Self {
            x: SvgLength::zero(),
            y: SvgLength::zero(),
            width: SvgLength::zero(),
            height: SvgLength::zero(),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircleAttrs {
    pub cx: SvgLength,
    pub cy: SvgLength,
    pub r: SvgLength,
}

impl Default for CircleAttrs {
    fn default() -> Self {
        Self {
            cx: SvgLength::zero(),
            cy: SvgLength::zero(),
            r: SvgLength::zero(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EllipseAttrs {
    pub cx: SvgLength,
    pub cy: SvgLength,
    pub rx: SvgLength,
    pub ry: SvgLength,
}

impl Default for EllipseAttrs {
    fn default() -> Self {
        Self {
            cx: SvgLength::zero(),
            cy: SvgLength::zero(),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineAttrs {
    pub x1: SvgLength,
    pub y1: SvgLength,
    pub x2: SvgLength,
    pub y2: SvgLength,
}

impl Default for LineAttrs {
    fn default() -> Self {
        Self {
            x1: SvgLength::zero(),
            y1: SvgLength::zero(),
            x2: SvgLength::zero(),
            y2: SvgLength::zero(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolylineAttrs {
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct PolygonAttrs {
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct PathAttrs {
    pub d: BezPath,
    /// Inkscape live path effect reference (e.g. `url(#lpe1);url(#lpe2)`).
    pub inkscape_path_effect: Option<String>,
    /// Inkscape original path data (before LPE application).
    pub inkscape_original_d: Option<BezPath>,
}

impl Default for PathAttrs {
    fn default() -> Self {
        Self {
            d: BezPath::new(),
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GroupAttrs {
    /// Inkscape layer mode: when set to `"layer"`, this group is a layer.
    pub inkscape_groupmode: Option<String>,
    /// Inkscape label for the layer/group (e.g. `"Layer 1"`).
    pub inkscape_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DefsAttrs;

// ---------------------------------------------------------------------------
// Phase 2: Gradient attribute structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LinearGradientAttrs {
    pub x1: Option<SvgLength>,
    pub y1: Option<SvgLength>,
    pub x2: Option<SvgLength>,
    pub y2: Option<SvgLength>,
    pub gradient_units: Option<GradientUnits>,
    pub gradient_transform: Option<Transform2D>,
    pub spread_method: Option<SpreadMethod>,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RadialGradientAttrs {
    pub cx: Option<SvgLength>,
    pub cy: Option<SvgLength>,
    pub r: Option<SvgLength>,
    pub fx: Option<SvgLength>,
    pub fy: Option<SvgLength>,
    pub gradient_units: Option<GradientUnits>,
    pub gradient_transform: Option<Transform2D>,
    pub spread_method: Option<SpreadMethod>,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StopAttrs {
    pub offset: f64,
    pub stop_color: Option<Color>,
    pub stop_opacity: Option<f64>,
    /// Path data for mesh gradient stops (SVG 2 `<stop path="...">`).
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2: Pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PatternAttrs {
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
    pub pattern_units: Option<GradientUnits>,
    pub pattern_content_units: Option<GradientUnits>,
    pub pattern_transform: Option<Transform2D>,
    pub href: Option<String>,
    pub view_box: Option<ViewBox>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

// ---------------------------------------------------------------------------
// Phase 2: ClipPath & Mask
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ClipPathAttrs {
    pub clip_path_units: Option<GradientUnits>,
}

#[derive(Debug, Clone, Default)]
pub struct MaskAttrs {
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
    pub mask_units: Option<GradientUnits>,
    pub mask_content_units: Option<GradientUnits>,
}

// ---------------------------------------------------------------------------
// Phase 2: Use & Symbol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct UseAttrs {
    pub href: Option<String>,
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolAttrs {
    pub view_box: Option<ViewBox>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

// ---------------------------------------------------------------------------
// Phase 2: Image
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ImageAttrs {
    pub href: Option<String>,
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

// ---------------------------------------------------------------------------
// Phase 2: Text elements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TextAttrs {
    pub x: Vec<SvgLength>,
    pub y: Vec<SvgLength>,
    pub dx: Vec<SvgLength>,
    pub dy: Vec<SvgLength>,
    pub font_family: Option<String>,
    pub font_size: Option<SvgLength>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_anchor: Option<TextAnchor>,
    pub text_content: String,
}

#[derive(Debug, Clone, Default)]
pub struct TSpanAttrs {
    pub x: Vec<SvgLength>,
    pub y: Vec<SvgLength>,
    pub dx: Vec<SvgLength>,
    pub dy: Vec<SvgLength>,
    pub font_family: Option<String>,
    pub font_size: Option<SvgLength>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub text_anchor: Option<TextAnchor>,
    pub text_content: String,
}

#[derive(Debug, Clone, Default)]
pub struct TextPathAttrs {
    pub href: Option<String>,
    pub start_offset: Option<SvgLength>,
    pub text_content: String,
}

// ---------------------------------------------------------------------------
// Phase 2: Marker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct MarkerAttrs {
    pub ref_x: Option<SvgLength>,
    pub ref_y: Option<SvgLength>,
    pub marker_width: Option<SvgLength>,
    pub marker_height: Option<SvgLength>,
    pub orient: Option<Orient>,
    pub marker_units: Option<MarkerUnits>,
    pub view_box: Option<ViewBox>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

// ---------------------------------------------------------------------------
// Phase 2: Filter container
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FilterAttrs {
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
    pub filter_units: Option<GradientUnits>,
    pub primitive_units: Option<GradientUnits>,
}

// ---------------------------------------------------------------------------
// Phase 2: Shared filter primitive common attributes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FilterPrimitiveAttrs {
    pub result: Option<String>,
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
}

// ---------------------------------------------------------------------------
// Phase 2: Filter primitive structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FeGaussianBlurAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub std_deviation_x: f64,
    pub std_deviation_y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeOffsetAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeFloodAttrs {
    pub common: FilterPrimitiveAttrs,
    pub flood_color: Option<Color>,
    pub flood_opacity: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct FeBlendAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub in2: Option<FilterInput>,
    pub mode: Option<BlendMode>,
}

#[derive(Debug, Clone, Default)]
pub struct FeCompositeAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub in2: Option<FilterInput>,
    pub operator: Option<CompositeOperator>,
    pub k1: f64,
    pub k2: f64,
    pub k3: f64,
    pub k4: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeColorMatrixAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub matrix_type: Option<ColorMatrixType>,
    pub values: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FeMergeAttrs {
    pub common: FilterPrimitiveAttrs,
}

#[derive(Debug, Clone, Default)]
pub struct FeMergeNodeAttrs {
    pub input: Option<FilterInput>,
}

#[derive(Debug, Clone, Default)]
pub struct FeMorphologyAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub operator: Option<MorphologyOperator>,
    pub radius_x: f64,
    pub radius_y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeDropShadowAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub dx: f64,
    pub dy: f64,
    pub std_deviation_x: f64,
    pub std_deviation_y: f64,
    pub flood_color: Option<Color>,
    pub flood_opacity: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct FeTurbulenceAttrs {
    pub common: FilterPrimitiveAttrs,
    pub turbulence_type: Option<TurbulenceType>,
    pub base_frequency_x: f64,
    pub base_frequency_y: f64,
    pub num_octaves: Option<i32>,
    pub seed: Option<f64>,
    pub stitch_tiles: Option<StitchTiles>,
}

#[derive(Debug, Clone, Default)]
pub struct FeDisplacementMapAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub in2: Option<FilterInput>,
    pub scale: Option<f64>,
    pub x_channel_selector: Option<ChannelSelector>,
    pub y_channel_selector: Option<ChannelSelector>,
}

#[derive(Debug, Clone, Default)]
pub struct FeImageAttrs {
    pub common: FilterPrimitiveAttrs,
    pub href: Option<String>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

#[derive(Debug, Clone, Default)]
pub struct FeTileAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
}

#[derive(Debug, Clone, Default)]
pub struct FeComponentTransferAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
}

#[derive(Debug, Clone, Default)]
pub struct FeConvolveMatrixAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub order_x: i32,
    pub order_y: i32,
    pub kernel_matrix: Vec<f64>,
    pub divisor: Option<f64>,
    pub bias: f64,
    pub target_x: Option<i32>,
    pub target_y: Option<i32>,
    pub edge_mode: Option<EdgeMode>,
    pub preserve_alpha: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FeDiffuseLightingAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub surface_scale: Option<f64>,
    pub diffuse_constant: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct FeSpecularLightingAttrs {
    pub common: FilterPrimitiveAttrs,
    pub input: Option<FilterInput>,
    pub surface_scale: Option<f64>,
    pub specular_constant: Option<f64>,
    pub specular_exponent: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct FePointLightAttrs {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeDistantLightAttrs {
    pub azimuth: f64,
    pub elevation: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeSpotLightAttrs {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub points_at_x: f64,
    pub points_at_y: f64,
    pub points_at_z: f64,
    pub specular_exponent: Option<f64>,
    pub limiting_cone_angle: Option<f64>,
}

// ---------------------------------------------------------------------------
// Phase 2: Metadata elements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TitleAttrs {
    pub text_content: String,
}

#[derive(Debug, Clone, Default)]
pub struct DescAttrs {
    pub text_content: String,
}

#[derive(Debug, Clone, Default)]
pub struct MetadataAttrs {
    pub raw_content: String,
}

// ---------------------------------------------------------------------------
// Phase 2: Style element
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StyleElementAttrs {
    pub content: String,
    pub style_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2: Anchor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AnchorAttrs {
    pub href: Option<String>,
    pub target: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2: Switch, ForeignObject, Script
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SwitchAttrs;

#[derive(Debug, Clone, Default)]
pub struct ForeignObjectAttrs {
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub width: Option<SvgLength>,
    pub height: Option<SvgLength>,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptAttrs {
    pub content: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2.2: Inkscape-specific extension attribute structs
// ---------------------------------------------------------------------------

/// Attributes for `<sodipodi:namedview>` (Inkscape view/grid/guide settings).
/// All attributes stored as optional strings for round-trip fidelity.
#[derive(Debug, Clone, Default)]
pub struct NamedViewAttrs {
    pub page_color: Option<String>,
    pub border_color: Option<String>,
    pub border_opacity: Option<String>,
    pub inkscape_pageopacity: Option<String>,
    pub inkscape_pageshadow: Option<String>,
    pub inkscape_zoom: Option<String>,
    pub inkscape_cx: Option<String>,
    pub inkscape_cy: Option<String>,
    pub inkscape_window_width: Option<String>,
    pub inkscape_window_height: Option<String>,
    pub inkscape_window_x: Option<String>,
    pub inkscape_window_y: Option<String>,
    pub inkscape_window_maximized: Option<String>,
    pub inkscape_current_layer: Option<String>,
    pub inkscape_document_units: Option<String>,
    pub show_grid: Option<String>,
    pub units: Option<String>,
}

/// Attributes for `<inkscape:page>` (multi-page support).
#[derive(Debug, Clone, Default)]
pub struct InkscapePageAttrs {
    pub x: Option<String>,
    pub y: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub inkscape_label: Option<String>,
}

/// Attributes for `<inkscape:grid>` definitions.
#[derive(Debug, Clone, Default)]
pub struct InkscapeGridAttrs {
    pub grid_type: Option<String>,
    pub origin_x: Option<String>,
    pub origin_y: Option<String>,
    pub spacing_x: Option<String>,
    pub spacing_y: Option<String>,
    pub units: Option<String>,
    pub visible: Option<String>,
    pub enabled: Option<String>,
    pub snap_visible_only: Option<String>,
    pub empspacing: Option<String>,
    pub dotted: Option<String>,
}

/// Attributes for `<sodipodi:guide>` (guide lines).
#[derive(Debug, Clone, Default)]
pub struct SodipodiGuideAttrs {
    pub position: Option<String>,
    pub orientation: Option<String>,
    pub inkscape_label: Option<String>,
    pub inkscape_locked: Option<String>,
    pub inkscape_color: Option<String>,
}

// ---------------------------------------------------------------------------
// SVG 1.2 Flowed text
// ---------------------------------------------------------------------------

/// Attributes for `<flowRoot>` (SVG 1.2 flowed text container).
#[derive(Debug, Clone, Default)]
pub struct FlowRootAttrs {
    /// xml:space attribute.
    pub xml_space: Option<String>,
}

/// Attributes for `<flowRegion>` (defines the region text flows into).
#[derive(Debug, Clone, Default)]
pub struct FlowRegionAttrs {}

/// Attributes for `<flowPara>` (paragraph of flowed text).
#[derive(Debug, Clone, Default)]
pub struct FlowParaAttrs {
    pub text_content: String,
}

/// Attributes for `<flowSpan>` (inline span in flowed text).
#[derive(Debug, Clone, Default)]
pub struct FlowSpanAttrs {
    pub text_content: String,
}

// ---------------------------------------------------------------------------
// SVG 2 Mesh Gradients
// ---------------------------------------------------------------------------

/// Attributes for `<meshGradient>`.
#[derive(Debug, Clone, Default)]
pub struct MeshGradientAttrs {
    pub x: Option<SvgLength>,
    pub y: Option<SvgLength>,
    pub gradient_units: Option<GradientUnits>,
    pub gradient_transform: Option<Transform2D>,
    pub href: Option<String>,
}

/// Attributes for `<meshrow>`.
#[derive(Debug, Clone, Default)]
pub struct MeshRowAttrs {}

/// Attributes for `<meshpatch>`.
#[derive(Debug, Clone, Default)]
pub struct MeshPatchAttrs {
    /// Patch stop color.
    pub stop_color: Option<Color>,
    /// Patch stop path data.
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// SVG Font Elements (deprecated / legacy)
// ---------------------------------------------------------------------------

/// Attributes for `<font>`.
#[derive(Debug, Clone, Default)]
pub struct SvgFontAttrs {
    pub horiz_adv_x: Option<f64>,
    pub horiz_origin_x: Option<f64>,
    pub horiz_origin_y: Option<f64>,
}

/// Attributes for `<font-face>`.
#[derive(Debug, Clone, Default)]
pub struct FontFaceAttrs {
    pub font_family: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub units_per_em: Option<f64>,
    pub ascent: Option<f64>,
    pub descent: Option<f64>,
    pub cap_height: Option<f64>,
    pub x_height: Option<f64>,
}

/// Attributes for `<glyph>` and `<missing-glyph>`.
#[derive(Debug, Clone, Default)]
pub struct GlyphAttrs {
    pub unicode: Option<String>,
    pub glyph_name: Option<String>,
    pub d: Option<BezPath>,
    pub horiz_adv_x: Option<f64>,
}

/// Attributes for `<hkern>` and `<vkern>`.
#[derive(Debug, Clone, Default)]
pub struct KernAttrs {
    pub u1: Option<String>,
    pub u2: Option<String>,
    pub g1: Option<String>,
    pub g2: Option<String>,
    pub k: Option<f64>,
}

// ---------------------------------------------------------------------------
// 3D Box (Inkscape SPBox3D)
// ---------------------------------------------------------------------------

/// Attributes for the Inkscape `<SPBox3D>` 3D box element.
///
/// The box is defined by three vanishing point references and eight corner
/// coordinates. Vanishing point values may be `"inf"` to indicate a parallel
/// (infinite) projection along that axis.
#[derive(Debug, Clone, Default)]
pub struct Box3dAttrs {
    /// Vanishing point X reference (coordinate string or `"inf"` for parallel projection).
    pub vp_x: Option<String>,
    /// Vanishing point Y reference.
    pub vp_y: Option<String>,
    /// Vanishing point Z reference.
    pub vp_z: Option<String>,
    /// Front-bottom-left corner (`"x,y"` coordinate pair).
    pub corner0: Option<String>,
    /// Front-bottom-right corner.
    pub corner1: Option<String>,
    /// Front-top-right corner.
    pub corner2: Option<String>,
    /// Front-top-left corner.
    pub corner3: Option<String>,
    /// Back-bottom-left corner.
    pub corner4: Option<String>,
    /// Back-bottom-right corner.
    pub corner5: Option<String>,
    /// Back-top-right corner.
    pub corner6: Option<String>,
    /// Back-top-left corner.
    pub corner7: Option<String>,
}

// ---------------------------------------------------------------------------
// Helper: parse SVG length list ("10 20 30")
// ---------------------------------------------------------------------------

pub fn parse_length_list(s: &str) -> Vec<SvgLength> {
    s.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect()
}

pub fn serialize_length_list(lengths: &[SvgLength]) -> String {
    lengths
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Helper: parse SVG points attribute ("x1,y1 x2,y2 ...")
// ---------------------------------------------------------------------------

pub fn parse_points(s: &str) -> Vec<(f64, f64)> {
    let nums: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    nums.chunks(2)
        .filter_map(|chunk| {
            if chunk.len() == 2 {
                Some((chunk[0], chunk[1]))
            } else {
                None
            }
        })
        .collect()
}

pub fn serialize_points(pts: &[(f64, f64)]) -> String {
    pts.iter()
        .map(|(x, y)| format!("{},{}", x, y))
        .collect::<Vec<_>>()
        .join(" ")
}
