//! Document renderer: walks an `SvgDocument` tree and dispatches drawing
//! commands to a `RenderBackend`.

use sivgo_dom::{
    Align, BlendMode as DomBlendMode, ChannelSelector as DomChannelSelector, Color,
    ColorMatrixType as DomColorMatrixType, CompositeOperator as DomCompositeOp, FilterInput,
    FontStyle as DomFontStyle, FontWeight as DomFontWeight, GradientUnits as DomGradientUnits,
    MarkerUnits, MeetOrSlice, MorphologyOperator as DomMorphologyOp, NodeId, Orient, Paint,
    PreserveAspectRatio, SpreadMethod as DomSpreadMethod, SvgDocument, SvgElement,
    TextAnchor as DomTextAnchor, TextDecoration, TurbulenceType as DomTurbulenceType, ViewBox,
};
use sivgo_filters::{FilterBuffer, FilterPipeline, FilterPrimitive};
use sivgo_math::{BezPath, Point, Rect, Transform2D};
use sivgo_text::{FontStyleKind, TextAnchor as TextEngineAnchor, TextEngine};

use crate::backend::RenderBackend;
use crate::tiny_skia_backend::TinySkiaBackend;
use crate::types::{
    FillRule, GradientStop, GradientUnits, LineCap, LineJoin, RenderPaint, SpreadMethod,
    StrokeStyle,
};

/// Stateless document renderer.
///
/// Call [`DocumentRenderer::render`] to rasterize an entire `SvgDocument`
/// onto a `RenderBackend`.
pub struct DocumentRenderer;

impl DocumentRenderer {
    /// Render the full SVG document onto the given backend.
    ///
    /// The backend is first cleared to white, then each visible element
    /// is rendered in document order with cumulative transforms applied.
    pub fn render(doc: &SvgDocument, backend: &mut dyn RenderBackend) {
        backend.clear(Color::WHITE);

        let Some(root_id) = doc.root() else {
            return;
        };

        // Compute viewBox transform if the root is an <svg> with viewBox.
        let viewbox_transform = Self::compute_viewbox_transform(doc, root_id, backend.size());

        let mut text_engine = TextEngine::new();
        Self::render_node(doc, root_id, &viewbox_transform, backend, &mut text_engine);
    }

    /// Compute the transform that maps the viewBox to the viewport (backend size).
    fn compute_viewbox_transform(
        doc: &SvgDocument,
        root_id: NodeId,
        backend_size: (u32, u32),
    ) -> Transform2D {
        let Some(node) = doc.get_node(root_id) else {
            return Transform2D::identity();
        };

        if let SvgElement::Svg(attrs) = &node.element {
            if let Some(vb) = &attrs.view_box {
                if vb.width > 0.0 && vb.height > 0.0 {
                    let vp_width = attrs
                        .width
                        .map(|l| l.to_px())
                        .unwrap_or(backend_size.0 as f64);
                    let vp_height = attrs
                        .height
                        .map(|l| l.to_px())
                        .unwrap_or(backend_size.1 as f64);

                    return compute_viewbox_to_viewport_transform(
                        vb,
                        vp_width,
                        vp_height,
                        attrs.preserve_aspect_ratio.as_ref(),
                    );
                }
            }
        }

        Transform2D::identity()
    }

    /// Recursively render a single node and its children.
    fn render_node(
        doc: &SvgDocument,
        node_id: NodeId,
        parent_transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let Some(node) = doc.get_node(node_id) else {
            return;
        };

        // display:none — skip this element AND all its children entirely.
        if let Some(sivgo_dom::Display::None) = node.presentation.display {
            return;
        }

        // visibility:hidden / visibility:collapse — skip rendering THIS
        // element's own graphics but still process children (a child may
        // override visibility back to "visible").
        let is_visible = match node.presentation.visibility {
            Some(sivgo_dom::Visibility::Hidden) | Some(sivgo_dom::Visibility::Collapse) => false,
            _ => true, // None (inherit → default visible) or Some(Visible)
        };

        // Compute cumulative transform.
        let local_transform = node
            .presentation
            .transform
            .unwrap_or_else(Transform2D::identity);
        let transform = parent_transform.then(&local_transform);

        // Handle opacity: for group elements with opacity < 1.0 we use offscreen
        // rendering so that the group is composited as a single unit. For
        // non-group elements, per-element opacity via set_opacity is sufficient.
        let group_opacity = node.presentation.opacity.unwrap_or(1.0);
        let is_group = matches!(&node.element, SvgElement::Group(_));
        let use_offscreen = is_group && group_opacity < 1.0;
        let has_opacity = node.presentation.opacity.is_some() && !use_offscreen;

        // Isolation group: `isolation: isolate` creates a new stacking context.
        // This prevents blend modes inside the group from interacting with content
        // behind the group. Implemented by rendering into an offscreen buffer.
        let use_isolation = node.presentation.isolation.unwrap_or(false) && !use_offscreen;

        // Check for mix-blend-mode on this element.
        // When a non-Normal blend mode is set, the element (and its children)
        // are rendered into an offscreen buffer and composited with the blend mode.
        let blend_mode = node
            .presentation
            .mix_blend_mode
            .filter(|m| *m != DomBlendMode::Normal);
        let use_blend = blend_mode.is_some();
        if use_blend {
            backend.push_offscreen();
        }

        // Check for filter attribute on this element.
        let filter_ref = node
            .presentation
            .filter
            .as_ref()
            .and_then(|v| extract_url_id(v))
            .or_else(|| {
                node.attributes
                    .get("filter")
                    .and_then(|v| extract_url_id(v))
            });
        let has_filter = filter_ref.is_some();
        if has_filter {
            backend.push_offscreen();
        }

        if use_isolation {
            backend.push_offscreen();
        }

        if use_offscreen {
            backend.push_offscreen();
        } else if has_opacity {
            backend.save();
            backend.set_opacity(group_opacity);
        }

        // Check for clip-path: presentation attr first, then raw attributes.
        let clip_path_ref = node
            .presentation
            .clip_path
            .as_ref()
            .and_then(|v| extract_url_id(v))
            .or_else(|| {
                node.attributes
                    .get("clip-path")
                    .and_then(|v| extract_url_id(v))
            });
        let has_clip = clip_path_ref.is_some();
        if let Some(clip_id_str) = clip_path_ref {
            if let Some(clip_node_id) = doc.find_by_xml_id(&clip_id_str) {
                Self::apply_clip_path(doc, clip_node_id, &transform, backend);
            }
        }

        // Check for mask: presentation attr first, then raw attributes.
        let mask_ref = node
            .presentation
            .mask
            .as_ref()
            .and_then(|v| extract_url_id(v))
            .or_else(|| node.attributes.get("mask").and_then(|v| extract_url_id(v)));
        let has_mask = mask_ref.is_some();
        if let Some(mask_id_str) = mask_ref {
            if let Some(mask_node_id) = doc.find_by_xml_id(&mask_id_str) {
                Self::apply_mask(doc, mask_node_id, &transform, backend, text_engine);
            }
        }

        // Dispatch rendering based on element type.
        // When visibility is hidden we skip the element's own drawing
        // commands but still fall through to the child recursion below.
        match &node.element {
            SvgElement::Svg(_attrs) => {
                // The root SVG element just sets up the viewport.
                // Clear has already been done. Render children.
            }
            SvgElement::Group(_) => {
                // Groups only contribute transform and opacity (handled above).
            }
            SvgElement::Defs(_) => {
                // Defs are not rendered directly; skip children.
                if has_clip {
                    backend.clear_clip();
                }
                if has_mask {
                    backend.clear_mask();
                }
                if has_opacity {
                    backend.restore();
                }
                if has_filter {
                    let _ = backend.pop_offscreen_raw();
                }
                if let Some(mode) = blend_mode {
                    backend.pop_offscreen_with_blend(mode);
                }
                return;
            }
            // Non-rendered container elements.
            SvgElement::ClipPath(_)
            | SvgElement::Mask(_)
            | SvgElement::Symbol(_)
            | SvgElement::Marker(_)
            | SvgElement::Filter(_)
            | SvgElement::LinearGradient(_)
            | SvgElement::RadialGradient(_)
            | SvgElement::Stop(_)
            | SvgElement::Pattern(_) => {
                // These are never rendered directly; skip entirely.
                if has_clip {
                    backend.clear_clip();
                }
                if has_mask {
                    backend.clear_mask();
                }
                if has_opacity {
                    backend.restore();
                }
                if has_filter {
                    let _ = backend.pop_offscreen_raw();
                }
                if let Some(mode) = blend_mode {
                    backend.pop_offscreen_with_blend(mode);
                }
                return;
            }
            SvgElement::Use(attrs) => {
                if is_visible {
                    Self::render_use_element(
                        doc,
                        attrs,
                        &node.presentation,
                        &transform,
                        backend,
                        text_engine,
                    );
                }
                // Use element handles its own children; skip normal child traversal.
                if has_clip {
                    backend.clear_clip();
                }
                if has_mask {
                    backend.clear_mask();
                }
                if has_opacity {
                    backend.restore();
                }
                if has_filter {
                    let (w, h, premul_pixels) = backend.pop_offscreen_raw();
                    if let Some(filter_id_str) = filter_ref {
                        if let Some(filter_node_id) = doc.find_by_xml_id(&filter_id_str) {
                            let pipeline = build_filter_pipeline(doc, filter_node_id);
                            let source = premultiplied_u8_to_straight_f32(&premul_pixels, w, h);
                            let result = execute_pipeline_with_region(
                                doc,
                                filter_node_id,
                                node_id,
                                &transform,
                                &pipeline,
                                &source,
                            );
                            let result_pixels = straight_f32_to_premultiplied_u8(&result);
                            backend.composite_pixels(&result_pixels, w, h);
                        } else {
                            backend.composite_pixels(&premul_pixels, w, h);
                        }
                    }
                }
                if let Some(mode) = blend_mode {
                    backend.pop_offscreen_with_blend(mode);
                }
                return;
            }
            SvgElement::Rect(attrs) if is_visible => {
                let x = attrs.x.to_px();
                let y = attrs.y.to_px();
                let w = attrs.width.to_px();
                let h = attrs.height.to_px();
                if w > 0.0 && h > 0.0 {
                    let rect = Rect::new(x, y, x + w, y + h);
                    // Fill
                    if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                        backend.fill_rect(rect, &paint, &transform);
                    }
                    // Stroke
                    if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                        backend.stroke_rect(rect, &paint, &style, &transform);
                    }
                }
            }
            SvgElement::Circle(attrs) if is_visible => {
                let cx = attrs.cx.to_px();
                let cy = attrs.cy.to_px();
                let r = attrs.r.to_px();
                if r > 0.0 {
                    if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                        backend.fill_circle(cx, cy, r, &paint, &transform);
                    }
                    if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                        backend.stroke_circle(cx, cy, r, &paint, &style, &transform);
                    }
                }
            }
            SvgElement::Ellipse(attrs) if is_visible => {
                let cx = attrs.cx.to_px();
                let cy = attrs.cy.to_px();
                let rx = attrs.rx.to_px();
                let ry = attrs.ry.to_px();
                if rx > 0.0 && ry > 0.0 {
                    if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                        backend.fill_ellipse(cx, cy, rx, ry, &paint, &transform);
                    }
                    if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                        backend.stroke_ellipse(cx, cy, rx, ry, &paint, &style, &transform);
                    }
                }
            }
            SvgElement::Line(attrs) if is_visible => {
                let x1 = attrs.x1.to_px();
                let y1 = attrs.y1.to_px();
                let x2 = attrs.x2.to_px();
                let y2 = attrs.y2.to_px();
                // Lines only have stroke (no fill).
                if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                    let mut path = BezPath::new();
                    path.move_to(Point::new(x1, y1));
                    path.line_to(Point::new(x2, y2));
                    backend.stroke_path(&path, &paint, &style, &transform);
                }
            }
            SvgElement::Path(attrs) if is_visible => {
                let fill_rule = resolve_fill_rule(&node.presentation);
                if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                    backend.fill_path(&attrs.d, &paint, fill_rule, &transform);
                }
                if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                    backend.stroke_path(&attrs.d, &paint, &style, &transform);
                }
            }
            SvgElement::Polyline(attrs) if is_visible => {
                if attrs.points.len() >= 2 {
                    let path = points_to_bezpath(&attrs.points, false);
                    // Polyline: stroke only by default, but fill if specified.
                    if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                        let fill_rule = resolve_fill_rule(&node.presentation);
                        backend.fill_path(&path, &paint, fill_rule, &transform);
                    }
                    if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                        backend.stroke_path(&path, &paint, &style, &transform);
                    }
                }
            }
            SvgElement::Polygon(attrs) if is_visible => {
                if attrs.points.len() >= 3 {
                    let path = points_to_bezpath(&attrs.points, true);
                    if let Some(paint) = resolve_fill_paint(doc, &node.presentation) {
                        let fill_rule = resolve_fill_rule(&node.presentation);
                        backend.fill_path(&path, &paint, fill_rule, &transform);
                    }
                    if let Some((paint, style)) = resolve_stroke(doc, &node.presentation) {
                        backend.stroke_path(&path, &paint, &style, &transform);
                    }
                }
            }
            SvgElement::Image(attrs) if is_visible => {
                Self::render_image(attrs, &transform, backend);
            }
            SvgElement::Text(attrs) if is_visible => {
                Self::render_text_with_spans(
                    doc,
                    node_id,
                    attrs,
                    &node.presentation,
                    &transform,
                    backend,
                    text_engine,
                );
            }
            SvgElement::FlowRoot(_) => {
                // FlowRoot handles its own child traversal to avoid the
                // flowRegion's rect being rendered by the generic child loop.
                if is_visible {
                    Self::render_flow_root(
                        doc,
                        node_id,
                        &node.presentation,
                        &transform,
                        backend,
                        text_engine,
                    );
                }
                // Cleanup and return (skip normal child traversal).
                if has_clip {
                    backend.clear_clip();
                }
                if has_mask {
                    backend.clear_mask();
                }
                if use_offscreen {
                    backend.pop_offscreen_with_opacity(group_opacity);
                } else if has_opacity {
                    backend.restore();
                }
                if use_isolation {
                    backend.pop_offscreen_with_opacity(1.0);
                }
                if has_filter {
                    let (w, h, premul_pixels) = backend.pop_offscreen_raw();
                    if let Some(filter_id_str) = filter_ref {
                        if let Some(filter_node_id) = doc.find_by_xml_id(&filter_id_str) {
                            let pipeline = build_filter_pipeline(doc, filter_node_id);
                            let source = premultiplied_u8_to_straight_f32(&premul_pixels, w, h);
                            let result = execute_pipeline_with_region(
                                doc,
                                filter_node_id,
                                node_id,
                                &transform,
                                &pipeline,
                                &source,
                            );
                            let result_pixels = straight_f32_to_premultiplied_u8(&result);
                            backend.composite_pixels(&result_pixels, w, h);
                        } else {
                            backend.composite_pixels(&premul_pixels, w, h);
                        }
                    }
                }
                if let Some(mode) = blend_mode {
                    backend.pop_offscreen_with_blend(mode);
                }
                return;
            }
            SvgElement::Switch(_) => {
                // <switch> evaluates children in order and renders only the
                // first child whose conditional attributes match.
                //
                // A child matches when:
                //   - it has no `requiredFeatures` attribute (we don't
                //     implement any optional SVG features), AND
                //   - it has no `requiredExtensions` attribute (we don't
                //     support any extensions), AND
                //   - it has no `systemLanguage` attribute, OR its
                //     `systemLanguage` value is accepted.
                //
                // If no child matches, nothing is rendered.
                if is_visible {
                    let switch_children: Vec<NodeId> = doc.children(node_id).to_vec();
                    for child_id in switch_children {
                        if let Some(child_node) = doc.get_node(child_id) {
                            // Skip if requiredFeatures is specified (we
                            // support none of the optional feature strings).
                            if child_node.attributes.contains_key("requiredFeatures") {
                                continue;
                            }
                            // Skip if requiredExtensions is specified (we
                            // support no extensions).
                            if child_node.attributes.contains_key("requiredExtensions") {
                                continue;
                            }
                            // systemLanguage: if not present the child matches.
                            // If present, accept it (a real implementation
                            // would compare against the user's locale; for now
                            // we treat any value as matching so the first
                            // localised branch wins).
                            // Render this child and stop.
                            Self::render_node(doc, child_id, &transform, backend, text_engine);
                            break;
                        }
                    }
                }
                // Switch handles its own children; skip normal child traversal.
                if has_clip {
                    backend.clear_clip();
                }
                if has_mask {
                    backend.clear_mask();
                }
                if use_offscreen {
                    backend.pop_offscreen_with_opacity(group_opacity);
                } else if has_opacity {
                    backend.restore();
                }
                if use_isolation {
                    backend.pop_offscreen_with_opacity(1.0);
                }
                if has_filter {
                    let (w, h, premul_pixels) = backend.pop_offscreen_raw();
                    if let Some(filter_id_str) = filter_ref {
                        if let Some(filter_node_id) = doc.find_by_xml_id(&filter_id_str) {
                            let pipeline = build_filter_pipeline(doc, filter_node_id);
                            let source = premultiplied_u8_to_straight_f32(&premul_pixels, w, h);
                            let result = execute_pipeline_with_region(
                                doc,
                                filter_node_id,
                                node_id,
                                &transform,
                                &pipeline,
                                &source,
                            );
                            let result_pixels = straight_f32_to_premultiplied_u8(&result);
                            backend.composite_pixels(&result_pixels, w, h);
                        } else {
                            backend.composite_pixels(&premul_pixels, w, h);
                        }
                    }
                }
                if let Some(mode) = blend_mode {
                    backend.pop_offscreen_with_blend(mode);
                }
                return;
            }
            _ => {}
        }

        // Render markers for shape elements that have marker-start/mid/end.
        if is_visible && node.element.is_shape() {
            let has_markers = node.presentation.marker_start.is_some()
                || node.presentation.marker_mid.is_some()
                || node.presentation.marker_end.is_some();
            if has_markers {
                // Build the BezPath for this shape.
                if let Some(shape_path) = element_to_bezpath(&node.element) {
                    let stroke_width = node
                        .presentation
                        .stroke_width
                        .map(|l| l.to_px())
                        .unwrap_or(1.0);
                    Self::render_markers(
                        doc,
                        &shape_path,
                        &node.presentation,
                        stroke_width,
                        &transform,
                        backend,
                        text_engine,
                    );
                }
            }
        }

        // Recurse into children.
        let children: Vec<NodeId> = doc.children(node_id).to_vec();
        for child_id in children {
            Self::render_node(doc, child_id, &transform, backend, text_engine);
        }

        if has_clip {
            backend.clear_clip();
        }
        if has_mask {
            backend.clear_mask();
        }
        if use_offscreen {
            backend.pop_offscreen_with_opacity(group_opacity);
        } else if has_opacity {
            backend.restore();
        }
        if use_isolation {
            backend.pop_offscreen_with_opacity(1.0);
        }

        // Filter post-processing: pop offscreen, run pipeline, composite result.
        if has_filter {
            let (w, h, premul_pixels) = backend.pop_offscreen_raw();
            if let Some(filter_id_str) = filter_ref {
                if let Some(filter_node_id) = doc.find_by_xml_id(&filter_id_str) {
                    let pipeline = build_filter_pipeline(doc, filter_node_id);
                    // Convert premultiplied u8 RGBA → straight f32 RGBA for FilterBuffer.
                    let source = premultiplied_u8_to_straight_f32(&premul_pixels, w, h);
                    let result = execute_pipeline_with_region(
                        doc,
                        filter_node_id,
                        node_id,
                        &transform,
                        &pipeline,
                        &source,
                    );
                    // Convert straight f32 RGBA → premultiplied u8 RGBA for compositing.
                    let result_pixels = straight_f32_to_premultiplied_u8(&result);
                    backend.composite_pixels(&result_pixels, w, h);
                } else {
                    // Filter element not found — composite unfiltered content.
                    backend.composite_pixels(&premul_pixels, w, h);
                }
            }
        }

        // Blend mode post-processing: pop the offscreen buffer and composite
        // with the specified SVG blend mode.
        if let Some(mode) = blend_mode {
            backend.pop_offscreen_with_blend(mode);
        }
    }

    /// Apply a clip-path from a `<clipPath>` element to the backend.
    ///
    /// Supports nested clipping: if the `<clipPath>` element itself has a
    /// `clip-path` attribute, that outer clip is applied first (intersection).
    fn apply_clip_path(
        doc: &SvgDocument,
        clip_node_id: NodeId,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
    ) {
        let Some(clip_node) = doc.get_node(clip_node_id) else {
            return;
        };

        let SvgElement::ClipPath(clip_attrs) = &clip_node.element else {
            return;
        };

        // Handle nested clip-path: if the <clipPath> itself has a clip-path,
        // apply the outer clip first (producing the intersection of both regions).
        let nested_clip_ref = clip_node
            .presentation
            .clip_path
            .as_ref()
            .and_then(|v| extract_url_id(v))
            .or_else(|| {
                clip_node
                    .attributes
                    .get("clip-path")
                    .and_then(|v| extract_url_id(v))
            });
        if let Some(nested_clip_id_str) = nested_clip_ref {
            if let Some(nested_clip_node_id) = doc.find_by_xml_id(&nested_clip_id_str) {
                // Avoid infinite recursion: don't apply self as nested clip.
                if nested_clip_node_id != clip_node_id {
                    Self::apply_clip_path(doc, nested_clip_node_id, transform, backend);
                }
            }
        }

        // Determine units.
        let units = clip_attrs
            .clip_path_units
            .unwrap_or(DomGradientUnits::UserSpaceOnUse);

        // Build a combined BezPath from the clipPath's children.
        let mut combined = BezPath::new();
        for &child_id in doc.children(clip_node_id) {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };
            if let Some(path) = element_to_bezpath(&child.element) {
                // Apply child's local transform if any.
                let child_transform = child
                    .presentation
                    .transform
                    .unwrap_or_else(Transform2D::identity);
                let transformed = transform_bezpath(&path, &child_transform);
                combined.extend(&transformed);
            }
        }

        if combined.elements().is_empty() {
            return;
        }

        let clip_transform = if units == DomGradientUnits::UserSpaceOnUse {
            *transform
        } else {
            // objectBoundingBox: not commonly used for clip paths, default to identity.
            *transform
        };

        let fill_rule = clip_node
            .presentation
            .clip_rule
            .map(|r| match r {
                sivgo_dom::FillRule::EvenOdd => FillRule::EvenOdd,
                sivgo_dom::FillRule::NonZero => FillRule::NonZero,
            })
            .unwrap_or(FillRule::NonZero);

        backend.set_clip_path(&combined, fill_rule, &clip_transform);
    }

    /// Apply a mask from a `<mask>` element by rendering its contents to a
    /// temporary pixmap and using luminance masking.
    fn apply_mask(
        doc: &SvgDocument,
        mask_node_id: NodeId,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let Some(_mask_node) = doc.get_node(mask_node_id) else {
            return;
        };

        let (w, h) = backend.size();
        if w == 0 || h == 0 {
            return;
        }

        // Render mask contents into a temporary pixmap.
        let mut temp_backend = TinySkiaBackend::new(w, h);
        temp_backend.clear(Color::BLACK);

        // Render each child of the mask element.
        let children: Vec<NodeId> = doc.children(mask_node_id).to_vec();
        for child_id in children {
            Self::render_node(doc, child_id, transform, &mut temp_backend, text_engine);
        }

        let mask_pixels = temp_backend.pixels();
        backend.set_mask_from_pixmap_luminance(mask_pixels);
    }

    /// Render a `<use>` element by resolving its href and rendering the
    /// referenced element.
    fn render_use_element(
        doc: &SvgDocument,
        attrs: &sivgo_dom::UseAttrs,
        _pres: &sivgo_dom::PresentationAttrs,
        parent_transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let href = match &attrs.href {
            Some(h) => h,
            None => return,
        };

        // Extract the id from href (strip leading '#').
        let ref_id = href.strip_prefix('#').unwrap_or(href);
        let Some(ref_node_id) = doc.find_by_xml_id(ref_id) else {
            return;
        };

        // Apply x/y offset as a translation.
        let dx = attrs.x.map(|l| l.to_px()).unwrap_or(0.0);
        let dy = attrs.y.map(|l| l.to_px()).unwrap_or(0.0);
        let use_transform = if dx != 0.0 || dy != 0.0 {
            parent_transform.then(&Transform2D::translate(dx, dy))
        } else {
            *parent_transform
        };

        let Some(ref_node) = doc.get_node(ref_node_id) else {
            return;
        };

        match &ref_node.element {
            SvgElement::Symbol(symbol_attrs) => {
                // When <use> references a <symbol>, we apply the symbol's
                // viewBox transform if present, using the use's width/height
                // as the viewport.
                let symbol_transform = if let Some(vb) = &symbol_attrs.view_box {
                    let vp_w = attrs.width.map(|l| l.to_px()).unwrap_or(vb.width);
                    let vp_h = attrs.height.map(|l| l.to_px()).unwrap_or(vb.height);
                    let vb_transform = compute_viewbox_to_viewport_transform(
                        vb,
                        vp_w,
                        vp_h,
                        symbol_attrs.preserve_aspect_ratio.as_ref(),
                    );
                    use_transform.then(&vb_transform)
                } else {
                    use_transform
                };

                // Render the symbol's children.
                let children: Vec<NodeId> = doc.children(ref_node_id).to_vec();
                for child_id in children {
                    Self::render_node(doc, child_id, &symbol_transform, backend, text_engine);
                }
            }
            _ => {
                // For non-symbol referenced elements, render them directly
                // (applying the use's presentation attributes is optional;
                // for now we render the referenced element as-is).
                Self::render_node(doc, ref_node_id, &use_transform, backend, text_engine);
            }
        }
    }

    /// Render an `<image>` element by decoding the embedded image data and
    /// drawing it onto the backend.
    fn render_image(
        attrs: &sivgo_dom::ImageAttrs,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
    ) {
        let href = match &attrs.href {
            Some(h) => h,
            None => return,
        };

        let x = attrs.x.map(|l| l.to_px()).unwrap_or(0.0);
        let y = attrs.y.map(|l| l.to_px()).unwrap_or(0.0);

        // Decode image data from data URI or load from file path.
        let (rgba_data, img_w, img_h) = if href.starts_with("data:") {
            match decode_image_data_uri(href) {
                Some(v) => v,
                None => return,
            }
        } else {
            match load_image_from_path(href) {
                Some(v) => v,
                None => return,
            }
        };

        // Target width/height: use attributes if given, otherwise native size.
        let vp_w = attrs.width.map(|l| l.to_px()).unwrap_or(img_w as f64);
        let vp_h = attrs.height.map(|l| l.to_px()).unwrap_or(img_h as f64);

        if vp_w <= 0.0 || vp_h <= 0.0 {
            return;
        }

        // Apply preserveAspectRatio to fit the image within the viewport.
        let par = attrs
            .preserve_aspect_ratio
            .as_ref()
            .copied()
            .unwrap_or_default();

        let (draw_x, draw_y, draw_w, draw_h) =
            compute_image_placement(img_w as f64, img_h as f64, vp_w, vp_h, &par);

        let img_transform = Transform2D::translate(x + draw_x, y + draw_y).then(transform);

        backend.draw_image(
            &rgba_data,
            img_w,
            img_h,
            0.0,
            0.0,
            draw_w,
            draw_h,
            &img_transform,
        );
    }

    /// Render a `<text>` element with its children (tspan etc.), given the text node's id.
    fn render_text_with_spans(
        doc: &SvgDocument,
        text_node_id: NodeId,
        attrs: &sivgo_dom::TextAttrs,
        pres: &sivgo_dom::PresentationAttrs,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let base_font_family = attrs
            .font_family
            .as_deref()
            .or(pres.font_family.as_deref())
            .unwrap_or("sans-serif");
        let parent_font_size = 16.0_f64;
        let base_font_size = attrs
            .font_size
            .or(pres.font_size)
            .map(|l| l.resolve_px(parent_font_size, parent_font_size))
            .unwrap_or(parent_font_size);
        let base_font_weight = match attrs
            .font_weight
            .or(pres.font_weight)
            .unwrap_or(DomFontWeight::Normal)
        {
            DomFontWeight::Normal => 400u16,
            DomFontWeight::Bold => 700,
            DomFontWeight::Number(n) => n as u16,
        };
        let base_font_style = match attrs
            .font_style
            .or(pres.font_style)
            .unwrap_or(DomFontStyle::Normal)
        {
            DomFontStyle::Normal => FontStyleKind::Normal,
            DomFontStyle::Italic => FontStyleKind::Italic,
            DomFontStyle::Oblique => FontStyleKind::Italic,
        };
        let base_anchor = match attrs
            .text_anchor
            .or(pres.text_anchor)
            .unwrap_or(DomTextAnchor::Start)
        {
            DomTextAnchor::Start => TextEngineAnchor::Start,
            DomTextAnchor::Middle => TextEngineAnchor::Middle,
            DomTextAnchor::End => TextEngineAnchor::End,
        };

        let base_letter_spacing = pres.letter_spacing.map(|l| l.to_px()).unwrap_or(0.0);
        let base_word_spacing = pres.word_spacing.map(|l| l.to_px()).unwrap_or(0.0);

        let base_decoration = pres.text_decoration.unwrap_or_default();

        let mut cursor_x = attrs.x.first().map(|l| l.to_px()).unwrap_or(0.0);
        let mut cursor_y = attrs.y.first().map(|l| l.to_px()).unwrap_or(0.0);
        cursor_x += attrs.dx.first().map(|l| l.to_px()).unwrap_or(0.0);
        cursor_y += attrs.dy.first().map(|l| l.to_px()).unwrap_or(0.0);

        // Helper: render a single text run and advance cursor.
        // Accepts per-character positioning arrays for SVG text x/y and tspan dx/dy
        // positioning (tasks 6.6 and 6.7).
        let mut render_run = |text: &str,
                              x: f64,
                              y: f64,
                              font_family: &str,
                              font_size: f64,
                              font_weight: u16,
                              font_style: FontStyleKind,
                              anchor: TextEngineAnchor,
                              fill_paint: &Option<RenderPaint>,
                              stroke: &Option<(RenderPaint, StrokeStyle)>,
                              fill_rule: FillRule,
                              letter_spacing: f64,
                              word_spacing: f64,
                              decoration: TextDecoration,
                              x_list: &[f64],
                              y_list: &[f64],
                              dx_list: &[f64],
                              dy_list: &[f64],
                              cursor_x: &mut f64| {
            if text.is_empty() {
                return;
            }

            // Use positioned variant when any array has multiple values or
            // dx/dy arrays are present.
            let has_positioned =
                x_list.len() > 1 || y_list.len() > 1 || !dx_list.is_empty() || !dy_list.is_empty();

            let path = if has_positioned {
                sivgo_text::text_to_path_positioned(
                    text_engine,
                    text,
                    x,
                    y,
                    font_family,
                    font_size,
                    font_weight,
                    font_style,
                    anchor,
                    letter_spacing,
                    word_spacing,
                    x_list,
                    y_list,
                    dx_list,
                    dy_list,
                )
            } else {
                sivgo_text::text_to_path(
                    text_engine,
                    text,
                    x,
                    y,
                    font_family,
                    font_size,
                    font_weight,
                    font_style,
                    anchor,
                    letter_spacing,
                    word_spacing,
                )
            };

            let path = match path {
                Some(p) => p,
                None => return,
            };

            // Advance cursor by approximate text width (use bounding box).
            let bbox = path.bounding_box();
            if bbox.width() > 0.0 {
                *cursor_x = bbox.x1;
            }

            if let Some(paint) = fill_paint {
                backend.fill_path(&path, paint, fill_rule, transform);
            }
            if let Some((paint, style)) = stroke {
                backend.stroke_path(&path, paint, style, transform);
            }

            // Render text decorations as horizontal lines.
            if !decoration.is_none() {
                let line_thickness = font_size * 0.07; // ~7% of font size
                let deco_style = StrokeStyle {
                    width: line_thickness,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    miter_limit: 4.0,
                    dash_array: None,
                    dash_offset: 0.0,
                };
                let deco_paint = fill_paint.as_ref().or(stroke.as_ref().map(|(p, _)| p));
                if let Some(paint) = deco_paint {
                    let text_left = bbox.x0;
                    let text_right = bbox.x1;

                    let mut draw_deco_line = |y_pos: f64| {
                        let mut line_path = BezPath::new();
                        line_path.move_to(Point::new(text_left, y_pos));
                        line_path.line_to(Point::new(text_right, y_pos));
                        backend.stroke_path(&line_path, paint, &deco_style, transform);
                    };

                    if decoration.underline {
                        // Below baseline by ~12.5% of font size
                        draw_deco_line(y + font_size * 0.125);
                    }
                    if decoration.overline {
                        // At the ascent line (above baseline)
                        let metrics = sivgo_text::get_font_metrics(
                            text_engine,
                            font_family,
                            font_size,
                            font_weight,
                            font_style,
                        );
                        draw_deco_line(y - metrics.ascent);
                    }
                    if decoration.line_through {
                        // At approximately x-height / 2 above baseline
                        let metrics = sivgo_text::get_font_metrics(
                            text_engine,
                            font_family,
                            font_size,
                            font_weight,
                            font_style,
                        );
                        draw_deco_line(y - metrics.x_height * 0.5);
                    }
                }
            }
        };

        let fill_rule = resolve_fill_rule(pres);
        let fill_paint = resolve_fill_paint(doc, pres);
        let stroke = resolve_stroke(doc, pres);

        // Convert text element x/y/dx/dy arrays to f64 slices for positioned layout.
        let text_x_list: Vec<f64> = attrs.x.iter().map(|l| l.to_px()).collect();
        let text_y_list: Vec<f64> = attrs.y.iter().map(|l| l.to_px()).collect();
        let text_dx_list: Vec<f64> = attrs.dx.iter().map(|l| l.to_px()).collect();
        let text_dy_list: Vec<f64> = attrs.dy.iter().map(|l| l.to_px()).collect();

        // Render the text element's own content (before any tspan children).
        if !attrs.text_content.is_empty() {
            render_run(
                &attrs.text_content,
                cursor_x,
                cursor_y,
                base_font_family,
                base_font_size,
                base_font_weight,
                base_font_style,
                base_anchor,
                &fill_paint,
                &stroke,
                fill_rule,
                base_letter_spacing,
                base_word_spacing,
                base_decoration,
                &text_x_list,
                &text_y_list,
                &text_dx_list,
                &text_dy_list,
                &mut cursor_x,
            );
        }

        // Process tspan children.
        let children: Vec<NodeId> = doc.children(text_node_id).to_vec();
        for child_id in children {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };

            if let SvgElement::TSpan(tspan_attrs) = &child.element {
                // Convert tspan positioning arrays to f64 for positioned layout.
                let tspan_x_list: Vec<f64> = tspan_attrs.x.iter().map(|l| l.to_px()).collect();
                let tspan_y_list: Vec<f64> = tspan_attrs.y.iter().map(|l| l.to_px()).collect();
                let tspan_dx_list: Vec<f64> = tspan_attrs.dx.iter().map(|l| l.to_px()).collect();
                let tspan_dy_list: Vec<f64> = tspan_attrs.dy.iter().map(|l| l.to_px()).collect();

                // Apply tspan positioning: absolute x/y override cursor.
                if let Some(x_val) = tspan_attrs.x.first() {
                    cursor_x = x_val.to_px();
                }
                if let Some(y_val) = tspan_attrs.y.first() {
                    cursor_y = y_val.to_px();
                }
                // For single-value dx/dy, apply to cursor (legacy behavior).
                // Multi-value arrays are handled by the positioned layout engine.
                if tspan_dx_list.len() <= 1 {
                    cursor_x += tspan_attrs.dx.first().map(|l| l.to_px()).unwrap_or(0.0);
                }
                if tspan_dy_list.len() <= 1 {
                    cursor_y += tspan_attrs.dy.first().map(|l| l.to_px()).unwrap_or(0.0);
                }

                // Resolve tspan font properties (inherit from parent text element).
                let font_family = tspan_attrs
                    .font_family
                    .as_deref()
                    .or(child.presentation.font_family.as_deref())
                    .unwrap_or(base_font_family);
                let font_size = tspan_attrs
                    .font_size
                    .or(child.presentation.font_size)
                    .map(|l| l.resolve_px(base_font_size, base_font_size))
                    .unwrap_or(base_font_size);
                let font_weight = match tspan_attrs
                    .font_weight
                    .or(child.presentation.font_weight)
                    .unwrap_or(DomFontWeight::Normal)
                {
                    DomFontWeight::Normal => base_font_weight,
                    DomFontWeight::Bold => 700,
                    DomFontWeight::Number(n) => n as u16,
                };
                let font_style = match tspan_attrs.font_style.or(child.presentation.font_style) {
                    Some(DomFontStyle::Italic) | Some(DomFontStyle::Oblique) => {
                        FontStyleKind::Italic
                    }
                    Some(DomFontStyle::Normal) => FontStyleKind::Normal,
                    None => base_font_style,
                };
                let anchor = match tspan_attrs.text_anchor.or(child.presentation.text_anchor) {
                    Some(DomTextAnchor::Start) => TextEngineAnchor::Start,
                    Some(DomTextAnchor::Middle) => TextEngineAnchor::Middle,
                    Some(DomTextAnchor::End) => TextEngineAnchor::End,
                    None => base_anchor,
                };

                // Use tspan's own fill/stroke if set, else inherit from parent.
                let tspan_fill =
                    resolve_fill_paint(doc, &child.presentation).or_else(|| fill_paint.clone());
                let tspan_stroke =
                    resolve_stroke(doc, &child.presentation).or_else(|| stroke.clone());
                let tspan_fill_rule = if child.presentation.fill_rule.is_some() {
                    resolve_fill_rule(&child.presentation)
                } else {
                    fill_rule
                };

                // Inherit or override letter/word spacing from tspan.
                let tspan_letter_spacing = child
                    .presentation
                    .letter_spacing
                    .map(|l| l.to_px())
                    .unwrap_or(base_letter_spacing);
                let tspan_word_spacing = child
                    .presentation
                    .word_spacing
                    .map(|l| l.to_px())
                    .unwrap_or(base_word_spacing);
                let tspan_decoration = child
                    .presentation
                    .text_decoration
                    .unwrap_or(base_decoration);

                render_run(
                    &tspan_attrs.text_content,
                    cursor_x,
                    cursor_y,
                    font_family,
                    font_size,
                    font_weight,
                    font_style,
                    anchor,
                    &tspan_fill,
                    &tspan_stroke,
                    tspan_fill_rule,
                    tspan_letter_spacing,
                    tspan_word_spacing,
                    tspan_decoration,
                    &tspan_x_list,
                    &tspan_y_list,
                    &tspan_dx_list,
                    &tspan_dy_list,
                    &mut cursor_x,
                );
            }
        }

        // Drop render_run closure to release borrows on backend/text_engine.
        drop(render_run);

        // Process <textPath> children (needs mutable backend + text_engine).
        let children2: Vec<NodeId> = doc.children(text_node_id).to_vec();
        for child_id in children2 {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };
            if let SvgElement::TextPath(tp_attrs) = &child.element {
                Self::render_text_on_path(
                    doc,
                    tp_attrs,
                    &child.presentation,
                    pres,
                    base_font_family,
                    base_font_size,
                    base_font_weight,
                    base_font_style,
                    &fill_paint,
                    &stroke,
                    fill_rule,
                    transform,
                    backend,
                    text_engine,
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // SVG 1.2 Flowed Text (flowRoot / flowRegion / flowPara / flowSpan)
    // -----------------------------------------------------------------------

    /// Render a `<flowRoot>` element (SVG 1.2 flowed text in regions).
    ///
    /// The flow region is determined by the first `<flowRegion>` child, which
    /// typically contains a `<rect>` defining the text area.  Each `<flowPara>`
    /// child is a paragraph, and `<flowSpan>` children are inline styled spans.
    /// Text is word-wrapped to fit within the flow region bounds.
    fn render_flow_root(
        doc: &SvgDocument,
        flow_root_id: NodeId,
        pres: &sivgo_dom::PresentationAttrs,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        // 1. Find the flow region rectangle.
        let region_rect = Self::find_flow_region_rect(doc, flow_root_id);
        let Some(region) = region_rect else {
            return;
        };

        // 2. Resolve base font properties from the flowRoot presentation attrs.
        let parent_font_size = 16.0_f64;
        let base_font_family = pres.font_family.as_deref().unwrap_or("sans-serif");
        let base_font_size = pres
            .font_size
            .map(|l| l.resolve_px(parent_font_size, parent_font_size))
            .unwrap_or(parent_font_size);
        let base_font_weight = match pres.font_weight.unwrap_or(DomFontWeight::Normal) {
            DomFontWeight::Normal => 400u16,
            DomFontWeight::Bold => 700,
            DomFontWeight::Number(n) => n as u16,
        };
        let base_font_style = match pres.font_style.unwrap_or(DomFontStyle::Normal) {
            DomFontStyle::Normal => FontStyleKind::Normal,
            DomFontStyle::Italic | DomFontStyle::Oblique => FontStyleKind::Italic,
        };
        let base_anchor = match pres.text_anchor.unwrap_or(DomTextAnchor::Start) {
            DomTextAnchor::Start => TextEngineAnchor::Start,
            DomTextAnchor::Middle => TextEngineAnchor::Middle,
            DomTextAnchor::End => TextEngineAnchor::End,
        };
        let base_letter_spacing = pres.letter_spacing.map(|l| l.to_px()).unwrap_or(0.0);
        let base_word_spacing = pres.word_spacing.map(|l| l.to_px()).unwrap_or(0.0);

        let fill_paint = resolve_fill_paint(doc, pres);
        let stroke = resolve_stroke(doc, pres);
        let fill_rule = resolve_fill_rule(pres);

        // 3. Collect paragraphs from flowPara children.
        let mut cursor_y = region.y0;
        let children: Vec<NodeId> = doc.children(flow_root_id).to_vec();

        for child_id in children {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };

            match &child.element {
                SvgElement::FlowPara(_para_attrs) => {
                    // Collect all text runs (inline spans and direct text) in this paragraph.
                    let runs = Self::collect_flow_para_runs(
                        doc,
                        child_id,
                        &child.presentation,
                        pres,
                        base_font_family,
                        base_font_size,
                        base_font_weight,
                        base_font_style,
                        base_letter_spacing,
                        base_word_spacing,
                    );

                    // Word-wrap and render the paragraph within the region.
                    cursor_y = Self::render_flow_paragraph(
                        &runs,
                        &region,
                        cursor_y,
                        base_anchor,
                        &fill_paint,
                        &stroke,
                        fill_rule,
                        transform,
                        backend,
                        text_engine,
                    );

                    // If we've exceeded the region, stop rendering.
                    if cursor_y > region.y1 {
                        break;
                    }
                }
                _ => {
                    // Skip non-flowPara children (e.g. flowRegion already processed).
                }
            }
        }
    }

    /// Find the bounding rectangle of the first `<flowRegion>` child.
    ///
    /// The flow region typically contains a `<rect>` child whose geometry
    /// defines the text area. Returns `None` if no valid region is found.
    fn find_flow_region_rect(doc: &SvgDocument, flow_root_id: NodeId) -> Option<Rect> {
        let children: Vec<NodeId> = doc.children(flow_root_id).to_vec();
        for child_id in children {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };
            if let SvgElement::FlowRegion(_) = &child.element {
                // Look for a rect (or other shape) inside the flowRegion.
                let region_children: Vec<NodeId> = doc.children(child_id).to_vec();
                for region_child_id in region_children {
                    let Some(region_child) = doc.get_node(region_child_id) else {
                        continue;
                    };
                    match &region_child.element {
                        SvgElement::Rect(rect_attrs) => {
                            let x = rect_attrs.x.to_px();
                            let y = rect_attrs.y.to_px();
                            let w = rect_attrs.width.to_px();
                            let h = rect_attrs.height.to_px();
                            if w > 0.0 && h > 0.0 {
                                return Some(Rect::new(x, y, x + w, y + h));
                            }
                        }
                        SvgElement::Circle(circle_attrs) => {
                            // Use bounding box of circle.
                            let cx = circle_attrs.cx.to_px();
                            let cy = circle_attrs.cy.to_px();
                            let r = circle_attrs.r.to_px();
                            if r > 0.0 {
                                return Some(Rect::new(cx - r, cy - r, cx + r, cy + r));
                            }
                        }
                        SvgElement::Ellipse(ellipse_attrs) => {
                            // Use bounding box of ellipse.
                            let cx = ellipse_attrs.cx.to_px();
                            let cy = ellipse_attrs.cy.to_px();
                            let rx = ellipse_attrs.rx.to_px();
                            let ry = ellipse_attrs.ry.to_px();
                            if rx > 0.0 && ry > 0.0 {
                                return Some(Rect::new(cx - rx, cy - ry, cx + rx, cy + ry));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Collect text runs from a `<flowPara>` element.
    ///
    /// This gathers the direct text content and any `<flowSpan>` children
    /// into a list of styled text runs.
    #[allow(clippy::too_many_arguments)]
    fn collect_flow_para_runs(
        doc: &SvgDocument,
        para_id: NodeId,
        para_pres: &sivgo_dom::PresentationAttrs,
        root_pres: &sivgo_dom::PresentationAttrs,
        base_font_family: &str,
        base_font_size: f64,
        base_font_weight: u16,
        base_font_style: FontStyleKind,
        base_letter_spacing: f64,
        base_word_spacing: f64,
    ) -> Vec<FlowTextRun> {
        let mut runs = Vec::new();

        // Resolve paragraph-level font properties (inherit from flowRoot).
        let para_font_family = para_pres.font_family.as_deref().unwrap_or(base_font_family);
        let para_font_size = para_pres
            .font_size
            .map(|l| l.resolve_px(base_font_size, base_font_size))
            .unwrap_or(base_font_size);
        let para_font_weight = match para_pres.font_weight {
            Some(DomFontWeight::Bold) => 700u16,
            Some(DomFontWeight::Number(n)) => n as u16,
            Some(DomFontWeight::Normal) | None => base_font_weight,
        };
        let para_font_style = match para_pres.font_style {
            Some(DomFontStyle::Italic) | Some(DomFontStyle::Oblique) => FontStyleKind::Italic,
            Some(DomFontStyle::Normal) => FontStyleKind::Normal,
            None => base_font_style,
        };
        let para_letter_spacing = para_pres
            .letter_spacing
            .map(|l| l.to_px())
            .unwrap_or(base_letter_spacing);
        let para_word_spacing = para_pres
            .word_spacing
            .map(|l| l.to_px())
            .unwrap_or(base_word_spacing);

        let para_fill =
            resolve_fill_paint(doc, para_pres).or_else(|| resolve_fill_paint(doc, root_pres));
        let para_stroke = resolve_stroke(doc, para_pres).or_else(|| resolve_stroke(doc, root_pres));
        let para_fill_rule = if para_pres.fill_rule.is_some() {
            resolve_fill_rule(para_pres)
        } else {
            resolve_fill_rule(root_pres)
        };

        // Get the paragraph's own text content.
        let para_node = doc.get_node(para_id);
        if let Some(node) = para_node {
            if let SvgElement::FlowPara(attrs) = &node.element {
                if !attrs.text_content.is_empty() {
                    runs.push(FlowTextRun {
                        text: attrs.text_content.clone(),
                        font_family: para_font_family.to_string(),
                        font_size: para_font_size,
                        font_weight: para_font_weight,
                        font_style: para_font_style,
                        letter_spacing: para_letter_spacing,
                        word_spacing: para_word_spacing,
                        fill_paint: para_fill.clone(),
                        stroke: para_stroke.clone(),
                        fill_rule: para_fill_rule,
                    });
                }
            }
        }

        // Process flowSpan children.
        let children: Vec<NodeId> = doc.children(para_id).to_vec();
        for child_id in children {
            let Some(child) = doc.get_node(child_id) else {
                continue;
            };
            if let SvgElement::FlowSpan(span_attrs) = &child.element {
                if span_attrs.text_content.is_empty() {
                    continue;
                }

                // Resolve span font properties (inherit from paragraph).
                let span_font_family = child
                    .presentation
                    .font_family
                    .as_deref()
                    .unwrap_or(para_font_family);
                let span_font_size = child
                    .presentation
                    .font_size
                    .map(|l| l.resolve_px(para_font_size, para_font_size))
                    .unwrap_or(para_font_size);
                let span_font_weight = match child.presentation.font_weight {
                    Some(DomFontWeight::Bold) => 700u16,
                    Some(DomFontWeight::Number(n)) => n as u16,
                    Some(DomFontWeight::Normal) => 400,
                    None => para_font_weight,
                };
                let span_font_style = match child.presentation.font_style {
                    Some(DomFontStyle::Italic) | Some(DomFontStyle::Oblique) => {
                        FontStyleKind::Italic
                    }
                    Some(DomFontStyle::Normal) => FontStyleKind::Normal,
                    None => para_font_style,
                };
                let span_letter_spacing = child
                    .presentation
                    .letter_spacing
                    .map(|l| l.to_px())
                    .unwrap_or(para_letter_spacing);
                let span_word_spacing = child
                    .presentation
                    .word_spacing
                    .map(|l| l.to_px())
                    .unwrap_or(para_word_spacing);

                let span_fill =
                    resolve_fill_paint(doc, &child.presentation).or_else(|| para_fill.clone());
                let span_stroke =
                    resolve_stroke(doc, &child.presentation).or_else(|| para_stroke.clone());
                let span_fill_rule = if child.presentation.fill_rule.is_some() {
                    resolve_fill_rule(&child.presentation)
                } else {
                    para_fill_rule
                };

                runs.push(FlowTextRun {
                    text: span_attrs.text_content.clone(),
                    font_family: span_font_family.to_string(),
                    font_size: span_font_size,
                    font_weight: span_font_weight,
                    font_style: span_font_style,
                    letter_spacing: span_letter_spacing,
                    word_spacing: span_word_spacing,
                    fill_paint: span_fill,
                    stroke: span_stroke,
                    fill_rule: span_fill_rule,
                });
            }
        }

        runs
    }

    /// Render a single flow paragraph with word wrapping within a region.
    ///
    /// Returns the updated `cursor_y` position after the paragraph.
    #[allow(clippy::too_many_arguments)]
    fn render_flow_paragraph(
        runs: &[FlowTextRun],
        region: &Rect,
        start_y: f64,
        anchor: TextEngineAnchor,
        default_fill: &Option<RenderPaint>,
        default_stroke: &Option<(RenderPaint, StrokeStyle)>,
        _default_fill_rule: FillRule,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) -> f64 {
        if runs.is_empty() {
            return start_y;
        }

        // Concatenate all runs into a single string for word wrapping.
        // Track run boundaries so we can apply correct styling per segment.
        let mut full_text = String::new();
        let mut run_boundaries: Vec<(usize, usize, usize)> = Vec::new(); // (start_byte, end_byte, run_index)

        for (run_idx, run) in runs.iter().enumerate() {
            let start = full_text.len();
            full_text.push_str(&run.text);
            let end = full_text.len();
            run_boundaries.push((start, end, run_idx));
        }

        if full_text.is_empty() {
            return start_y;
        }

        // Use the first run's font metrics for line height calculation.
        let primary_run = &runs[0];
        let metrics = sivgo_text::get_font_metrics(
            text_engine,
            &primary_run.font_family,
            primary_run.font_size,
            primary_run.font_weight,
            primary_run.font_style,
        );
        let line_height = metrics.line_height;
        let region_width = region.width();

        // Word-wrap the full text into lines.
        let words = split_into_words(&full_text);
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();

        for word in &words {
            if current_line.is_empty() {
                // First word on the line, always add it.
                current_line.push_str(word);
            } else {
                // Try adding the word with a space.
                let candidate = format!("{} {}", current_line, word);
                let candidate_width = sivgo_text::measure_text_width(
                    text_engine,
                    &candidate,
                    &primary_run.font_family,
                    primary_run.font_size,
                    primary_run.font_weight,
                    primary_run.font_style,
                    primary_run.letter_spacing,
                    primary_run.word_spacing,
                );

                if candidate_width <= region_width {
                    current_line = candidate;
                } else {
                    // Wrap to next line.
                    lines.push(current_line);
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Render each line.
        let mut cursor_y = start_y + metrics.ascent;

        for line in &lines {
            if cursor_y + metrics.descent > region.y1 {
                // Stop if we've exceeded the region vertically.
                return region.y1 + 1.0; // Signal overflow
            }

            // Determine x position based on text-anchor.
            let x = match anchor {
                TextEngineAnchor::Start => region.x0,
                TextEngineAnchor::Middle => region.x0 + region_width / 2.0,
                TextEngineAnchor::End => region.x1,
            };

            // Find which run(s) this line text belongs to.
            // For simplicity, if the paragraph has a single run (common case),
            // render with that run's styling. For multi-run paragraphs, find
            // the byte offset within the full text and map to the right run.
            if runs.len() == 1 {
                let run = &runs[0];
                let fill = run.fill_paint.as_ref().or(default_fill.as_ref());
                let stroke = run.stroke.as_ref().or(default_stroke.as_ref());

                if let Some(path) = sivgo_text::text_to_path(
                    text_engine,
                    line,
                    x,
                    cursor_y,
                    &run.font_family,
                    run.font_size,
                    run.font_weight,
                    run.font_style,
                    anchor,
                    run.letter_spacing,
                    run.word_spacing,
                ) {
                    if let Some(paint) = fill {
                        backend.fill_path(&path, paint, run.fill_rule, transform);
                    }
                    if let Some((paint, style)) = stroke {
                        backend.stroke_path(&path, paint, style, transform);
                    }
                }
            } else {
                // Multi-run paragraph: render with the primary run's styling.
                // In the common Inkscape case, each flowPara has either direct
                // text or flowSpan children, so this handles the mixed case
                // by finding which run the line's characters originally came from.
                // For a first implementation, we render each line using the
                // styling of whichever run contributes the most characters.
                let line_start_in_full = find_line_offset_in_text(&full_text, line);
                let dominant_run =
                    find_dominant_run(&run_boundaries, line_start_in_full, line.len(), runs);

                let fill = dominant_run.fill_paint.as_ref().or(default_fill.as_ref());
                let stroke = dominant_run.stroke.as_ref().or(default_stroke.as_ref());

                if let Some(path) = sivgo_text::text_to_path(
                    text_engine,
                    line,
                    x,
                    cursor_y,
                    &dominant_run.font_family,
                    dominant_run.font_size,
                    dominant_run.font_weight,
                    dominant_run.font_style,
                    anchor,
                    dominant_run.letter_spacing,
                    dominant_run.word_spacing,
                ) {
                    if let Some(paint) = fill {
                        backend.fill_path(&path, paint, dominant_run.fill_rule, transform);
                    }
                    if let Some((paint, style)) = stroke {
                        backend.stroke_path(&path, paint, style, transform);
                    }
                }
            }

            cursor_y += line_height;
        }

        cursor_y
    }

    /// Render text along a referenced path (`<textPath>`).
    fn render_text_on_path(
        doc: &SvgDocument,
        tp_attrs: &sivgo_dom::TextPathAttrs,
        tp_pres: &sivgo_dom::PresentationAttrs,
        _parent_pres: &sivgo_dom::PresentationAttrs,
        base_font_family: &str,
        base_font_size: f64,
        base_font_weight: u16,
        base_font_style: FontStyleKind,
        parent_fill: &Option<RenderPaint>,
        parent_stroke: &Option<(RenderPaint, StrokeStyle)>,
        fill_rule: FillRule,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let text = &tp_attrs.text_content;
        if text.is_empty() {
            return;
        }

        // Resolve the referenced path.
        let href = match &tp_attrs.href {
            Some(h) => h.strip_prefix('#').unwrap_or(h).to_string(),
            None => return,
        };
        let path_node_id = match doc.find_by_xml_id(&href) {
            Some(id) => id,
            None => return,
        };
        let path_node = match doc.get_node(path_node_id) {
            Some(n) => n,
            None => return,
        };
        let ref_path = match element_to_bezpath(&path_node.element) {
            Some(p) => p,
            None => return,
        };

        let start_offset = tp_attrs.start_offset.map(|l| l.to_px()).unwrap_or(0.0);

        // Resolve paint.
        let fill_paint = resolve_fill_paint(doc, tp_pres).or_else(|| parent_fill.clone());
        let stroke = resolve_stroke(doc, tp_pres).or_else(|| parent_stroke.clone());

        let font_family = tp_pres.font_family.as_deref().unwrap_or(base_font_family);
        let font_size = tp_pres
            .font_size
            .map(|l| l.resolve_px(base_font_size, base_font_size))
            .unwrap_or(base_font_size);

        // Place each character along the path.
        let total_len = ref_path.arc_length(0.5);
        if total_len <= 0.0 {
            return;
        }

        let mut offset = start_offset;
        for ch in text.chars() {
            if ch.is_whitespace() && ch != ' ' {
                continue;
            }
            let ch_str = ch.to_string();

            // Get the glyph outline at origin.
            let glyph_path = match sivgo_text::text_to_path(
                text_engine,
                &ch_str,
                0.0,
                0.0,
                font_family,
                font_size,
                base_font_weight,
                base_font_style,
                TextEngineAnchor::Start,
                0.0,
                0.0,
            ) {
                Some(p) => p,
                None => continue,
            };

            let bbox = glyph_path.bounding_box();
            let char_width = bbox.width().max(font_size * 0.3); // fallback width

            // Find position and tangent at the midpoint of this character.
            let mid_dist = offset + char_width * 0.5;
            if mid_dist < 0.0 || mid_dist > total_len {
                offset += char_width;
                continue;
            }

            let t_param = match ref_path.param_at_arclen(mid_dist, 0.5) {
                Some(t) => t,
                None => {
                    offset += char_width;
                    continue;
                }
            };
            let pos = match ref_path.eval(t_param) {
                Some(p) => p,
                None => {
                    offset += char_width;
                    continue;
                }
            };

            // Compute tangent angle by sampling two nearby points.
            let dt = 0.001;
            let t_before = (t_param - dt).max(0.0);
            let t_after_val = t_param + dt;
            let p_before = ref_path.eval(t_before).unwrap_or(pos);
            let p_after = ref_path.eval(t_after_val).unwrap_or(pos);
            let dx = p_after.x - p_before.x;
            let dy = p_after.y - p_before.y;
            let angle = dy.atan2(dx);

            // Build transform: translate to path point, rotate to tangent, offset by -char_width/2.
            let char_transform = Transform2D::translate(-char_width * 0.5, 0.0)
                .then(&Transform2D::rotate(angle))
                .then(&Transform2D::translate(pos.x, pos.y));
            let final_transform = char_transform.then(transform);

            if let Some(paint) = &fill_paint {
                backend.fill_path(&glyph_path, paint, fill_rule, &final_transform);
            }
            if let Some((paint, style)) = &stroke {
                backend.stroke_path(&glyph_path, paint, style, &final_transform);
            }

            offset += char_width;
        }
    }

    // -----------------------------------------------------------------------
    // Marker rendering
    // -----------------------------------------------------------------------

    /// Render markers (start, mid, end) for a shape element's path.
    fn render_markers(
        doc: &SvgDocument,
        path: &BezPath,
        pres: &sivgo_dom::PresentationAttrs,
        stroke_width: f64,
        transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let vertices = get_path_vertices(path);
        if vertices.is_empty() {
            return;
        }

        // Resolve marker element IDs from url(#id) references.
        let marker_start_id = pres
            .marker_start
            .as_deref()
            .and_then(extract_url_id)
            .and_then(|id| doc.find_by_xml_id(&id));
        let marker_mid_id = pres
            .marker_mid
            .as_deref()
            .and_then(extract_url_id)
            .and_then(|id| doc.find_by_xml_id(&id));
        let marker_end_id = pres
            .marker_end
            .as_deref()
            .and_then(extract_url_id)
            .and_then(|id| doc.find_by_xml_id(&id));

        let last_idx = vertices.len() - 1;

        for (i, vertex) in vertices.iter().enumerate() {
            let marker_node_id = if i == 0 {
                marker_start_id
            } else if i == last_idx {
                marker_end_id
            } else {
                marker_mid_id
            };

            let Some(marker_node_id) = marker_node_id else {
                continue;
            };

            let is_start = i == 0;

            Self::render_single_marker(
                doc,
                marker_node_id,
                vertex,
                stroke_width,
                is_start,
                transform,
                backend,
                text_engine,
            );
        }
    }

    /// Render a single marker instance at the given vertex position.
    fn render_single_marker(
        doc: &SvgDocument,
        marker_node_id: NodeId,
        vertex: &MarkerVertex,
        stroke_width: f64,
        is_start: bool,
        parent_transform: &Transform2D,
        backend: &mut dyn RenderBackend,
        text_engine: &mut TextEngine,
    ) {
        let Some(marker_node) = doc.get_node(marker_node_id) else {
            return;
        };
        let SvgElement::Marker(marker_attrs) = &marker_node.element else {
            return;
        };

        // Resolve marker dimensions (SVG defaults: markerWidth=3, markerHeight=3).
        let marker_width = marker_attrs.marker_width.map(|l| l.to_px()).unwrap_or(3.0);
        let marker_height = marker_attrs.marker_height.map(|l| l.to_px()).unwrap_or(3.0);

        if marker_width <= 0.0 || marker_height <= 0.0 {
            return;
        }

        // Resolve refX, refY (default 0).
        let ref_x = marker_attrs.ref_x.map(|l| l.to_px()).unwrap_or(0.0);
        let ref_y = marker_attrs.ref_y.map(|l| l.to_px()).unwrap_or(0.0);

        // Resolve orient.
        let orient = marker_attrs.orient.unwrap_or(Orient::Auto);
        let angle_rad = match orient {
            Orient::Auto => vertex.angle,
            Orient::AutoStartReverse => {
                if is_start {
                    vertex.angle + std::f64::consts::PI
                } else {
                    vertex.angle
                }
            }
            Orient::Angle(deg) => deg.to_radians(),
        };

        // Resolve markerUnits (default: strokeWidth).
        let marker_units = marker_attrs
            .marker_units
            .unwrap_or(MarkerUnits::StrokeWidth);

        // Build the marker transform:
        // 1. Start with parent transform
        // 2. Translate to vertex position
        // 3. Rotate by orient angle
        // 4. Scale by stroke width (if markerUnits=strokeWidth)
        // 5. Apply viewBox transform or scale to markerWidth/markerHeight
        // 6. Translate by -refX, -refY

        let mut marker_transform = *parent_transform;

        // Translate to vertex.
        marker_transform =
            marker_transform.then(&Transform2D::translate(vertex.point.x, vertex.point.y));

        // Rotate.
        if angle_rad.abs() > 1e-10 {
            marker_transform = marker_transform.then(&Transform2D::rotate(angle_rad));
        }

        // Scale by stroke width if markerUnits=strokeWidth.
        if marker_units == MarkerUnits::StrokeWidth {
            marker_transform = marker_transform.then(&Transform2D::scale(stroke_width));
        }

        // Apply viewBox transform if present, otherwise just use markerWidth x markerHeight.
        if let Some(vb) = &marker_attrs.view_box {
            if vb.width > 0.0 && vb.height > 0.0 {
                let vb_transform = compute_viewbox_to_viewport_transform(
                    vb,
                    marker_width,
                    marker_height,
                    marker_attrs.preserve_aspect_ratio.as_ref(),
                );
                marker_transform = marker_transform.then(&vb_transform);
            }
        }

        // Translate by -refX, -refY.
        marker_transform = marker_transform.then(&Transform2D::translate(-ref_x, -ref_y));

        // Render marker's children.
        let children: Vec<NodeId> = doc.children(marker_node_id).to_vec();
        for child_id in children {
            Self::render_node(doc, child_id, &marker_transform, backend, text_engine);
        }
    }
}

// ---------------------------------------------------------------------------
// Marker vertex extraction
// ---------------------------------------------------------------------------

/// A vertex on a path where a marker may be placed.
struct MarkerVertex {
    /// The position of the vertex.
    point: Point,
    /// The orientation angle in radians (for orient="auto").
    /// This is the bisector angle of the incoming and outgoing directions,
    /// or the tangent direction if only one direction is available.
    angle: f64,
}

/// Extract vertices from a BezPath for marker placement.
///
/// Returns a list of vertices with their positions and orientation angles.
/// The first vertex gets marker-start, the last gets marker-end, and all
/// intermediate vertices get marker-mid.
fn get_path_vertices(path: &BezPath) -> Vec<MarkerVertex> {
    let segments = path.elements();
    if segments.is_empty() {
        return Vec::new();
    }

    // Collect all segment endpoints and their tangent directions.
    // We need (point, incoming_tangent_angle, outgoing_tangent_angle) for each vertex.
    struct RawVertex {
        point: Point,
        /// Incoming tangent angle (None for the first point in a subpath).
        incoming: Option<f64>,
        /// Outgoing tangent angle (None for the last point in a subpath).
        outgoing: Option<f64>,
    }

    let mut raw_vertices: Vec<RawVertex> = Vec::new();
    let mut current_point = Point::new(0.0, 0.0);
    let mut subpath_start = Point::new(0.0, 0.0);
    let mut subpath_start_idx: Option<usize> = None;

    for seg in &segments {
        match seg {
            sivgo_math::PathSegment::MoveTo(p) => {
                current_point = *p;
                subpath_start = *p;
                raw_vertices.push(RawVertex {
                    point: *p,
                    incoming: None,
                    outgoing: None,
                });
                subpath_start_idx = Some(raw_vertices.len() - 1);
            }
            sivgo_math::PathSegment::LineTo(p) => {
                let angle = angle_between(current_point, *p);
                // Set outgoing angle on previous vertex.
                if let Some(last) = raw_vertices.last_mut() {
                    if last.outgoing.is_none() {
                        last.outgoing = Some(angle);
                    }
                }
                raw_vertices.push(RawVertex {
                    point: *p,
                    incoming: Some(angle),
                    outgoing: None,
                });
                current_point = *p;
            }
            sivgo_math::PathSegment::QuadTo(cp, p) => {
                // Outgoing tangent from current point toward cp.
                let out_angle = angle_between(current_point, *cp);
                if let Some(last) = raw_vertices.last_mut() {
                    if last.outgoing.is_none() {
                        last.outgoing = Some(out_angle);
                    }
                }
                // Incoming tangent at endpoint is from cp toward p.
                let in_angle = angle_between(*cp, *p);
                raw_vertices.push(RawVertex {
                    point: *p,
                    incoming: Some(in_angle),
                    outgoing: None,
                });
                current_point = *p;
            }
            sivgo_math::PathSegment::CurveTo(cp1, cp2, p) => {
                // Outgoing tangent from current point toward cp1.
                let out_angle = angle_between(current_point, *cp1);
                if let Some(last) = raw_vertices.last_mut() {
                    if last.outgoing.is_none() {
                        last.outgoing = Some(out_angle);
                    }
                }
                // Incoming tangent at endpoint is from cp2 toward p.
                let in_angle = angle_between(*cp2, *p);
                raw_vertices.push(RawVertex {
                    point: *p,
                    incoming: Some(in_angle),
                    outgoing: None,
                });
                current_point = *p;
            }
            sivgo_math::PathSegment::ClosePath => {
                // ClosePath draws a line back to the subpath start.
                if let Some(start_idx) = subpath_start_idx {
                    let close_angle = angle_between(current_point, subpath_start);
                    // Set outgoing on the last vertex before close.
                    if let Some(last) = raw_vertices.last_mut() {
                        if last.outgoing.is_none() {
                            last.outgoing = Some(close_angle);
                        }
                    }
                    // Set incoming on the subpath start vertex.
                    if start_idx < raw_vertices.len() {
                        if raw_vertices[start_idx].incoming.is_none() {
                            raw_vertices[start_idx].incoming = Some(close_angle);
                        }
                    }
                }
                current_point = subpath_start;
            }
        }
    }

    // Convert raw vertices to MarkerVertices with computed bisector angles.
    raw_vertices
        .into_iter()
        .map(|rv| {
            let angle = match (rv.incoming, rv.outgoing) {
                (Some(inc), Some(out)) => bisector_angle(inc, out),
                (Some(a), None) => a,
                (None, Some(a)) => a,
                (None, None) => 0.0,
            };
            MarkerVertex {
                point: rv.point,
                angle,
            }
        })
        .collect()
}

/// Compute the angle (in radians) of the direction from point `a` to point `b`.
fn angle_between(a: Point, b: Point) -> f64 {
    (b.y - a.y).atan2(b.x - a.x)
}

/// Compute the bisector angle between two direction angles.
///
/// This gives the average direction, handling the wraparound correctly.
fn bisector_angle(incoming: f64, outgoing: f64) -> f64 {
    // The bisector of two directions: average the angles, but handle
    // the case where they wrap around.
    let mut diff = outgoing - incoming;
    // Normalize diff to [-PI, PI].
    while diff > std::f64::consts::PI {
        diff -= 2.0 * std::f64::consts::PI;
    }
    while diff < -std::f64::consts::PI {
        diff += 2.0 * std::f64::consts::PI;
    }
    incoming + diff / 2.0
}

// ---------------------------------------------------------------------------
// Paint / stroke resolution helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// SVG 1.2 Flowed Text helpers
// ---------------------------------------------------------------------------

/// A single text run within a flow paragraph, with its own styling.
struct FlowTextRun {
    text: String,
    font_family: String,
    font_size: f64,
    font_weight: u16,
    font_style: FontStyleKind,
    letter_spacing: f64,
    word_spacing: f64,
    fill_paint: Option<RenderPaint>,
    stroke: Option<(RenderPaint, StrokeStyle)>,
    fill_rule: FillRule,
}

/// Split a string into words on whitespace boundaries.
///
/// Consecutive whitespace is collapsed. Empty words are not produced.
fn split_into_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(|s| s.to_string()).collect()
}

/// Find the byte offset of a line within the full text.
///
/// This is used to map wrapped line content back to run boundaries.
/// Returns the first occurrence of `line` as a substring in `full_text`,
/// or 0 if not found.
fn find_line_offset_in_text(full_text: &str, line: &str) -> usize {
    // Since word wrapping may have collapsed whitespace, we search
    // for the first word of the line in the original text.
    full_text.find(line).unwrap_or(0)
}

/// Find the run whose text contributes the most bytes to a line segment.
fn find_dominant_run<'a>(
    run_boundaries: &[(usize, usize, usize)],
    line_start: usize,
    line_len: usize,
    runs: &'a [FlowTextRun],
) -> &'a FlowTextRun {
    let line_end = line_start + line_len;
    let mut best_run_idx = 0;
    let mut best_overlap = 0;

    for &(start, end, run_idx) in run_boundaries {
        let overlap_start = start.max(line_start);
        let overlap_end = end.min(line_end);
        if overlap_end > overlap_start {
            let overlap = overlap_end - overlap_start;
            if overlap > best_overlap {
                best_overlap = overlap;
                best_run_idx = run_idx;
            }
        }
    }

    &runs[best_run_idx]
}

/// Resolve the fill paint for an element.
///
/// SVG default fill is black if not specified.
fn resolve_fill_paint(
    doc: &SvgDocument,
    pres: &sivgo_dom::PresentationAttrs,
) -> Option<RenderPaint> {
    match &pres.fill {
        Some(Paint::Color(c)) => Some(RenderPaint::solid(*c)),
        Some(Paint::None) => None,
        Some(Paint::Url(ref url)) => resolve_paint_server(doc, url),
        Some(Paint::CurrentColor) => {
            // currentColor resolves to the `color` property (default: black).
            let c = pres.color.unwrap_or(Color::BLACK);
            Some(RenderPaint::solid(c))
        }
        None => {
            // SVG default: fill is black.
            Some(RenderPaint::solid(Color::BLACK))
        }
    }
}

/// Resolve the stroke paint and style for an element.
///
/// Returns `None` if stroke is `none` or not specified.
fn resolve_stroke(
    doc: &SvgDocument,
    pres: &sivgo_dom::PresentationAttrs,
) -> Option<(RenderPaint, StrokeStyle)> {
    let paint = match &pres.stroke {
        Some(Paint::Color(c)) => RenderPaint::solid(*c),
        Some(Paint::Url(ref url)) => resolve_paint_server(doc, url)?,
        Some(Paint::CurrentColor) => {
            // currentColor resolves to the `color` property (default: black).
            let c = pres.color.unwrap_or(Color::BLACK);
            RenderPaint::solid(c)
        }
        Some(Paint::None) => return None,
        None => return None, // SVG default: no stroke
    };

    let width = pres.stroke_width.map(|l| l.to_px()).unwrap_or(1.0);

    let line_cap = match pres.stroke_linecap {
        Some(sivgo_dom::LineCap::Butt) => LineCap::Butt,
        Some(sivgo_dom::LineCap::Round) => LineCap::Round,
        Some(sivgo_dom::LineCap::Square) => LineCap::Square,
        None => LineCap::Butt,
    };

    let line_join = match pres.stroke_linejoin {
        Some(sivgo_dom::LineJoin::Miter) => LineJoin::Miter,
        Some(sivgo_dom::LineJoin::Round) => LineJoin::Round,
        Some(sivgo_dom::LineJoin::Bevel) => LineJoin::Bevel,
        None => LineJoin::Miter,
    };

    let miter_limit = pres.stroke_miterlimit.unwrap_or(4.0);
    let dash_array = pres.stroke_dasharray.clone();
    let dash_offset = pres.stroke_dashoffset.unwrap_or(0.0);

    Some((
        paint,
        StrokeStyle {
            width,
            line_cap,
            line_join,
            miter_limit,
            dash_array,
            dash_offset,
        },
    ))
}

/// Resolve the fill-rule from presentation attrs.
fn resolve_fill_rule(pres: &sivgo_dom::PresentationAttrs) -> FillRule {
    match pres.fill_rule {
        Some(sivgo_dom::FillRule::EvenOdd) => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    }
}

/// Build a `BezPath` from a list of (x, y) points.
///
/// If `close` is true, the path is closed (polygon); otherwise it is open (polyline).
fn points_to_bezpath(points: &[(f64, f64)], close: bool) -> BezPath {
    let mut path = BezPath::new();
    if let Some(&(x, y)) = points.first() {
        path.move_to(Point::new(x, y));
        for &(x, y) in &points[1..] {
            path.line_to(Point::new(x, y));
        }
        if close {
            path.close();
        }
    }
    path
}

// ---------------------------------------------------------------------------
// Paint server resolution
// ---------------------------------------------------------------------------

/// Resolve a `url(#id)` paint reference to a `RenderPaint`.
///
/// Supports `<linearGradient>` and `<radialGradient>` elements.
fn resolve_paint_server(doc: &SvgDocument, url: &str) -> Option<RenderPaint> {
    // Extract the id from the URL (strip leading '#').
    let ref_id = url.strip_prefix('#').unwrap_or(url);
    let node_id = doc.find_by_xml_id(ref_id)?;
    let node = doc.get_node(node_id)?;

    match &node.element {
        SvgElement::LinearGradient(attrs) => resolve_linear_gradient(doc, node_id, attrs),
        SvgElement::RadialGradient(attrs) => resolve_radial_gradient(doc, node_id, attrs),
        SvgElement::Pattern(attrs) => resolve_pattern(doc, node_id, attrs),
        SvgElement::MeshGradient(attrs) => resolve_mesh_gradient(doc, node_id, attrs),
        _ => None,
    }
}

/// Resolve a `<linearGradient>` element into a `RenderPaint`.
fn resolve_linear_gradient(
    doc: &SvgDocument,
    node_id: NodeId,
    attrs: &sivgo_dom::LinearGradientAttrs,
) -> Option<RenderPaint> {
    let units = attrs
        .gradient_units
        .unwrap_or(DomGradientUnits::ObjectBoundingBox);

    // SVG defaults for linear gradient coordinates.
    let (default_x1, default_y1, default_x2, default_y2) =
        if units == DomGradientUnits::ObjectBoundingBox {
            (0.0, 0.0, 1.0, 0.0)
        } else {
            (0.0, 0.0, 1.0, 0.0)
        };

    let x1 = attrs.x1.map(|l| l.to_px()).unwrap_or(default_x1);
    let y1 = attrs.y1.map(|l| l.to_px()).unwrap_or(default_y1);
    let x2 = attrs.x2.map(|l| l.to_px()).unwrap_or(default_x2);
    let y2 = attrs.y2.map(|l| l.to_px()).unwrap_or(default_y2);

    let spread = convert_dom_spread_method(attrs.spread_method.unwrap_or(DomSpreadMethod::Pad));
    let transform = attrs
        .gradient_transform
        .unwrap_or_else(Transform2D::identity);

    let render_units = match units {
        DomGradientUnits::ObjectBoundingBox => GradientUnits::ObjectBoundingBox,
        DomGradientUnits::UserSpaceOnUse => GradientUnits::UserSpaceOnUse,
    };

    // Collect stops from child <stop> elements, or from href if none.
    let stops = collect_gradient_stops(doc, node_id, attrs.href.as_deref());

    if stops.is_empty() {
        return None;
    }

    Some(RenderPaint::LinearGradient {
        x1,
        y1,
        x2,
        y2,
        stops,
        spread,
        transform,
        units: render_units,
    })
}

/// Resolve a `<radialGradient>` element into a `RenderPaint`.
fn resolve_radial_gradient(
    doc: &SvgDocument,
    node_id: NodeId,
    attrs: &sivgo_dom::RadialGradientAttrs,
) -> Option<RenderPaint> {
    let units = attrs
        .gradient_units
        .unwrap_or(DomGradientUnits::ObjectBoundingBox);

    let (default_cx, default_cy, default_r) = if units == DomGradientUnits::ObjectBoundingBox {
        (0.5, 0.5, 0.5)
    } else {
        (0.0, 0.0, 0.0)
    };

    let cx = attrs.cx.map(|l| l.to_px()).unwrap_or(default_cx);
    let cy = attrs.cy.map(|l| l.to_px()).unwrap_or(default_cy);
    let r = attrs.r.map(|l| l.to_px()).unwrap_or(default_r);
    let fx = attrs.fx.map(|l| l.to_px()).unwrap_or(cx);
    let fy = attrs.fy.map(|l| l.to_px()).unwrap_or(cy);

    let spread = convert_dom_spread_method(attrs.spread_method.unwrap_or(DomSpreadMethod::Pad));
    let transform = attrs
        .gradient_transform
        .unwrap_or_else(Transform2D::identity);

    let render_units = match units {
        DomGradientUnits::ObjectBoundingBox => GradientUnits::ObjectBoundingBox,
        DomGradientUnits::UserSpaceOnUse => GradientUnits::UserSpaceOnUse,
    };

    let stops = collect_gradient_stops(doc, node_id, attrs.href.as_deref());

    if stops.is_empty() {
        return None;
    }

    Some(RenderPaint::RadialGradient {
        cx,
        cy,
        r,
        fx,
        fy,
        stops,
        spread,
        transform,
        units: render_units,
    })
}

/// Resolve a `<pattern>` element into a `RenderPaint`.
///
/// Renders the pattern's content into a small offscreen pixmap (the tile),
/// then packages it as a `RenderPaint::Pattern` for tiled rendering.
fn resolve_pattern(
    doc: &SvgDocument,
    pattern_node_id: NodeId,
    attrs: &sivgo_dom::PatternAttrs,
) -> Option<RenderPaint> {
    // Resolve pattern attributes, following href chain for inherited values.
    let resolved = resolve_pattern_attrs(doc, pattern_node_id, attrs, 8);

    let units = resolved
        .pattern_units
        .unwrap_or(DomGradientUnits::ObjectBoundingBox);
    let _content_units = resolved
        .pattern_content_units
        .unwrap_or(DomGradientUnits::UserSpaceOnUse);

    // Determine pattern tile dimensions in user-space coordinates.
    // For objectBoundingBox units the values are fractions (0..1); we
    // cannot convert without a bounding box, so we treat them as if the
    // bounding box is 1x1 and use the transform to scale at render time.
    // For userSpaceOnUse the values are absolute pixel coordinates.
    let tile_w = resolved.width.map(|l| l.to_px()).unwrap_or(0.0);
    let tile_h = resolved.height.map(|l| l.to_px()).unwrap_or(0.0);

    if tile_w <= 0.0 || tile_h <= 0.0 {
        return None;
    }

    // For objectBoundingBox pattern_units the w/h are fractions; we pick a
    // reasonable pixel size to render the tile content. We'll use a fixed
    // size and let the pattern transform scale.
    let (render_w, render_h) = if units == DomGradientUnits::ObjectBoundingBox {
        // Render at a fixed resolution; the pattern transform will scale
        // from object-space fractions to this size.
        let px_w = (tile_w * 256.0).ceil().max(1.0).min(2048.0) as u32;
        let px_h = (tile_h * 256.0).ceil().max(1.0).min(2048.0) as u32;
        (px_w, px_h)
    } else {
        // userSpaceOnUse: the tile dimensions are in pixels.
        let px_w = tile_w.ceil().max(1.0).min(4096.0) as u32;
        let px_h = tile_h.ceil().max(1.0).min(4096.0) as u32;
        (px_w, px_h)
    };

    if render_w == 0 || render_h == 0 {
        return None;
    }

    // Determine which node holds the pattern content (children). If this
    // pattern has no children but has an href, follow the reference.
    let content_node_id =
        resolve_pattern_content_node(doc, pattern_node_id, attrs.href.as_deref(), 8);

    // Render the pattern content into an offscreen tile pixmap.
    let mut tile_backend = TinySkiaBackend::new(render_w, render_h);
    // Start with a fully transparent background so only the pattern
    // content contributes.
    tile_backend.clear(sivgo_dom::Color::new(0, 0, 0, 0));

    // If patternContentUnits is objectBoundingBox, we scale the content
    // so that (0..1, 0..1) maps to the tile size.
    let content_transform = if _content_units == DomGradientUnits::ObjectBoundingBox {
        Transform2D::scale_xy(render_w as f64, render_h as f64)
    } else if units == DomGradientUnits::UserSpaceOnUse {
        // Content is in user space; offset by pattern x/y.
        let px = resolved.x.map(|l| l.to_px()).unwrap_or(0.0);
        let py = resolved.y.map(|l| l.to_px()).unwrap_or(0.0);
        Transform2D::translate(-px, -py)
    } else {
        Transform2D::identity()
    };

    // If the pattern has a viewBox, apply a viewBox -> tile transform.
    let content_transform = if let Some(ref vb) = resolved.view_box {
        let vb_transform = compute_viewbox_to_viewport_transform(
            vb,
            render_w as f64,
            render_h as f64,
            resolved.preserve_aspect_ratio.as_ref(),
        );
        content_transform.then(&vb_transform)
    } else {
        content_transform
    };

    // Render each child of the pattern content node.
    let mut text_engine = sivgo_text::TextEngine::new();
    let children: Vec<NodeId> = doc.children(content_node_id).to_vec();
    for child_id in children {
        DocumentRenderer::render_node(
            doc,
            child_id,
            &content_transform,
            &mut tile_backend,
            &mut text_engine,
        );
    }

    let tile_pixels = tile_backend.pixels().to_vec();

    // Build the pattern transform.
    //
    // For userSpaceOnUse: the pattern tile is placed at (x, y) and tiled
    // at intervals of (width, height).
    //
    // For objectBoundingBox: the fractions will need to be resolved against
    // the shape's bbox at draw time. Since we don't have the bbox here,
    // we bake in a transform that maps the rendered tile size back to the
    // fractional objectBoundingBox space. The backend's make_paint will
    // further transform based on the shape_bbox.
    let pattern_transform = resolved
        .pattern_transform
        .unwrap_or_else(Transform2D::identity);

    let final_transform = if units == DomGradientUnits::UserSpaceOnUse {
        let px = resolved.x.map(|l| l.to_px()).unwrap_or(0.0);
        let py = resolved.y.map(|l| l.to_px()).unwrap_or(0.0);
        // Translate to pattern origin, then apply patternTransform.
        Transform2D::translate(px, py).then(&pattern_transform)
    } else {
        pattern_transform
    };

    Some(RenderPaint::Pattern {
        pixels: tile_pixels,
        tile_width: render_w,
        tile_height: render_h,
        transform: final_transform,
        opacity: 1.0,
    })
}

/// Resolve pattern attributes by following the href chain for inherited values.
///
/// Each attribute is taken from the first pattern in the chain that defines it.
fn resolve_pattern_attrs(
    doc: &SvgDocument,
    _node_id: NodeId,
    attrs: &sivgo_dom::PatternAttrs,
    max_depth: u32,
) -> sivgo_dom::PatternAttrs {
    let mut resolved = attrs.clone();

    // Follow the href chain for missing attributes.
    let mut href = attrs.href.clone();
    let mut depth = 0;
    while let Some(ref href_val) = href {
        if depth >= max_depth {
            break;
        }
        depth += 1;
        let ref_id = href_val.strip_prefix('#').unwrap_or(href_val);
        let Some(ref_node_id) = doc.find_by_xml_id(ref_id) else {
            break;
        };
        let Some(ref_node) = doc.get_node(ref_node_id) else {
            break;
        };
        let SvgElement::Pattern(ref_attrs) = &ref_node.element else {
            break;
        };

        // Inherit missing attributes.
        if resolved.x.is_none() {
            resolved.x = ref_attrs.x;
        }
        if resolved.y.is_none() {
            resolved.y = ref_attrs.y;
        }
        if resolved.width.is_none() {
            resolved.width = ref_attrs.width;
        }
        if resolved.height.is_none() {
            resolved.height = ref_attrs.height;
        }
        if resolved.pattern_units.is_none() {
            resolved.pattern_units = ref_attrs.pattern_units;
        }
        if resolved.pattern_content_units.is_none() {
            resolved.pattern_content_units = ref_attrs.pattern_content_units;
        }
        if resolved.pattern_transform.is_none() {
            resolved.pattern_transform = ref_attrs.pattern_transform;
        }
        if resolved.view_box.is_none() {
            resolved.view_box = ref_attrs.view_box.clone();
        }
        if resolved.preserve_aspect_ratio.is_none() {
            resolved.preserve_aspect_ratio = ref_attrs.preserve_aspect_ratio;
        }

        href = ref_attrs.href.clone();
    }

    resolved
}

/// Find the node that holds the pattern's content (children).
///
/// If the given pattern node has no children and has an href, follow the
/// reference chain up to `max_depth` to find a pattern with children.
fn resolve_pattern_content_node(
    doc: &SvgDocument,
    pattern_node_id: NodeId,
    href: Option<&str>,
    max_depth: u32,
) -> NodeId {
    // If the pattern itself has children, use it.
    if !doc.children(pattern_node_id).is_empty() {
        return pattern_node_id;
    }

    // Follow href chain.
    let mut current_href = href.map(|s| s.to_string());
    let mut depth = 0;
    while let Some(ref href_val) = current_href {
        if depth >= max_depth {
            break;
        }
        depth += 1;
        let ref_id = href_val.strip_prefix('#').unwrap_or(href_val);
        let Some(ref_node_id) = doc.find_by_xml_id(ref_id) else {
            break;
        };
        let Some(ref_node) = doc.get_node(ref_node_id) else {
            break;
        };

        if !doc.children(ref_node_id).is_empty() {
            return ref_node_id;
        }

        // Continue following the chain.
        if let SvgElement::Pattern(ref_attrs) = &ref_node.element {
            current_href = ref_attrs.href.clone();
        } else {
            break;
        }
    }

    // Fallback: return the original node even if it has no children.
    pattern_node_id
}

/// Collect gradient stops from a gradient element's children.
///
/// If the gradient has no child `<stop>` elements and an `href` is provided,
/// follow the reference chain (up to a depth limit).
fn collect_gradient_stops(
    doc: &SvgDocument,
    gradient_node_id: NodeId,
    href: Option<&str>,
) -> Vec<GradientStop> {
    let mut stops = Vec::new();

    for &child_id in doc.children(gradient_node_id) {
        let Some(child) = doc.get_node(child_id) else {
            continue;
        };
        if let SvgElement::Stop(stop_attrs) = &child.element {
            let color = stop_attrs.stop_color.unwrap_or(Color::BLACK);
            let opacity = stop_attrs.stop_opacity.unwrap_or(1.0);
            stops.push(GradientStop {
                offset: stop_attrs.offset.clamp(0.0, 1.0),
                color,
                opacity,
            });
        }
    }

    // If no stops found, try following the href reference.
    if stops.is_empty() {
        if let Some(href_val) = href {
            let ref_id = href_val.strip_prefix('#').unwrap_or(href_val);
            if let Some(ref_node_id) = doc.find_by_xml_id(ref_id) {
                if ref_node_id != gradient_node_id {
                    // Follow the chain (no href recursion to avoid infinite loops).
                    for &child_id in doc.children(ref_node_id) {
                        let Some(child) = doc.get_node(child_id) else {
                            continue;
                        };
                        if let SvgElement::Stop(stop_attrs) = &child.element {
                            let color = stop_attrs.stop_color.unwrap_or(Color::BLACK);
                            let opacity = stop_attrs.stop_opacity.unwrap_or(1.0);
                            stops.push(GradientStop {
                                offset: stop_attrs.offset.clamp(0.0, 1.0),
                                color,
                                opacity,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by offset.
    stops.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    stops
}

// ---------------------------------------------------------------------------
// Mesh gradient resolution
// ---------------------------------------------------------------------------

/// Resolve a `<meshGradient>` element into a `RenderPaint`.
///
/// Walks the meshGradient -> meshrow -> meshpatch -> stop hierarchy,
/// builds Coons patches, rasterizes them, and returns a `RenderPaint::MeshGradient`.
fn resolve_mesh_gradient(
    doc: &SvgDocument,
    mesh_node_id: NodeId,
    attrs: &sivgo_dom::MeshGradientAttrs,
) -> Option<RenderPaint> {
    use crate::mesh_gradient::{build_patches, rasterize_mesh, MeshPatchData};

    let origin_x = attrs.x.map(|l| l.to_px()).unwrap_or(0.0);
    let origin_y = attrs.y.map(|l| l.to_px()).unwrap_or(0.0);

    let units = attrs
        .gradient_units
        .unwrap_or(DomGradientUnits::UserSpaceOnUse);
    let grad_transform = attrs
        .gradient_transform
        .unwrap_or_else(Transform2D::identity);

    // Collect rows and patches from the DOM tree.
    let mut rows: Vec<Vec<MeshPatchData>> = Vec::new();

    for &row_id in doc.children(mesh_node_id) {
        let Some(row_node) = doc.get_node(row_id) else {
            continue;
        };
        if !matches!(row_node.element, SvgElement::MeshRow(_)) {
            continue;
        }

        let mut row_patches: Vec<MeshPatchData> = Vec::new();

        for &patch_id in doc.children(row_id) {
            let Some(patch_node) = doc.get_node(patch_id) else {
                continue;
            };
            if !matches!(patch_node.element, SvgElement::MeshPatch(_)) {
                continue;
            }

            let mut colors = Vec::new();
            let mut edge_paths = Vec::new();

            // Collect stop children of the meshpatch
            for &stop_id in doc.children(patch_id) {
                let Some(stop_node) = doc.get_node(stop_id) else {
                    continue;
                };
                if let SvgElement::Stop(stop_attrs) = &stop_node.element {
                    let color = stop_attrs.stop_color.unwrap_or(Color::BLACK);
                    let opacity = stop_attrs.stop_opacity.unwrap_or(1.0);
                    let c = Color::new(
                        color.r,
                        color.g,
                        color.b,
                        (color.a as f64 * opacity).round().clamp(0.0, 255.0) as u8,
                    );
                    colors.push(c);
                    edge_paths.push(stop_attrs.path.clone());
                }
            }

            if !colors.is_empty() {
                row_patches.push(MeshPatchData { colors, edge_paths });
            }
        }

        if !row_patches.is_empty() {
            rows.push(row_patches);
        }
    }

    if rows.is_empty() {
        return None;
    }

    let patches = build_patches((origin_x, origin_y), &rows);
    if patches.is_empty() {
        return None;
    }

    let (w, h, offset_x, offset_y, pixels) = rasterize_mesh(&patches, 1.0)?;

    let render_units = match units {
        DomGradientUnits::ObjectBoundingBox => GradientUnits::ObjectBoundingBox,
        DomGradientUnits::UserSpaceOnUse => GradientUnits::UserSpaceOnUse,
    };

    // Build the transform that positions the rasterized mesh in user space.
    // The rasterized pixels cover the region [offset_x, offset_y] to
    // [offset_x + w, offset_y + h] in gradient coordinate space.
    // We need a transform that maps from user-space to the pixel buffer.
    let mesh_transform = Transform2D::translate(offset_x, offset_y).then(&grad_transform);

    Some(RenderPaint::MeshGradient {
        pixels,
        width: w,
        height: h,
        transform: mesh_transform,
        units: render_units,
    })
}

/// Convert a DOM `SpreadMethod` to our render `SpreadMethod`.
fn convert_dom_spread_method(s: DomSpreadMethod) -> SpreadMethod {
    match s {
        DomSpreadMethod::Pad => SpreadMethod::Pad,
        DomSpreadMethod::Reflect => SpreadMethod::Reflect,
        DomSpreadMethod::Repeat => SpreadMethod::Repeat,
    }
}

// ---------------------------------------------------------------------------
// Helper: extract url(#id) -> id
// ---------------------------------------------------------------------------

/// Extract the id from a `url(#id)` string.
fn extract_url_id(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix("url(")?;
    let inner = inner.strip_suffix(')')?;
    let inner = inner.trim().trim_matches(|c| c == '\'' || c == '"');
    let id = inner.strip_prefix('#')?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

// ---------------------------------------------------------------------------
// Helper: compute viewBox -> viewport transform
// ---------------------------------------------------------------------------

/// Compute a `Transform2D` that maps a viewBox to a viewport of the given
/// dimensions, respecting the `preserveAspectRatio` setting.
fn compute_viewbox_to_viewport_transform(
    vb: &ViewBox,
    vp_width: f64,
    vp_height: f64,
    par: Option<&PreserveAspectRatio>,
) -> Transform2D {
    if vb.width <= 0.0 || vb.height <= 0.0 || vp_width <= 0.0 || vp_height <= 0.0 {
        return Transform2D::identity();
    }

    let par = par.copied().unwrap_or_default();

    let scale_x = vp_width / vb.width;
    let scale_y = vp_height / vb.height;

    if par.align == Align::None {
        // Non-uniform scaling: stretch to fill viewport.
        let tx = -vb.min_x * scale_x;
        let ty = -vb.min_y * scale_y;
        return Transform2D::new([scale_x, 0.0, 0.0, scale_y, tx, ty]);
    }

    let scale = match par.meet_or_slice {
        MeetOrSlice::Meet => scale_x.min(scale_y),
        MeetOrSlice::Slice => scale_x.max(scale_y),
    };

    // Compute translation based on alignment.
    let vb_scaled_w = vb.width * scale;
    let vb_scaled_h = vb.height * scale;

    let tx_align = match par.align {
        Align::XMinYMin | Align::XMinYMid | Align::XMinYMax => 0.0,
        Align::XMidYMin | Align::XMidYMid | Align::XMidYMax => (vp_width - vb_scaled_w) / 2.0,
        Align::XMaxYMin | Align::XMaxYMid | Align::XMaxYMax => vp_width - vb_scaled_w,
        Align::None => unreachable!(),
    };

    let ty_align = match par.align {
        Align::XMinYMin | Align::XMidYMin | Align::XMaxYMin => 0.0,
        Align::XMinYMid | Align::XMidYMid | Align::XMaxYMid => (vp_height - vb_scaled_h) / 2.0,
        Align::XMinYMax | Align::XMidYMax | Align::XMaxYMax => vp_height - vb_scaled_h,
        Align::None => unreachable!(),
    };

    let tx = tx_align - vb.min_x * scale;
    let ty = ty_align - vb.min_y * scale;

    Transform2D::new([scale, 0.0, 0.0, scale, tx, ty])
}

// ---------------------------------------------------------------------------
// Helper: convert elements to BezPath (for clip paths)
// ---------------------------------------------------------------------------

/// Try to convert an SVG element to a `BezPath` for clip-path building.
fn element_to_bezpath(element: &SvgElement) -> Option<BezPath> {
    match element {
        SvgElement::Rect(attrs) => {
            let x = attrs.x.to_px();
            let y = attrs.y.to_px();
            let w = attrs.width.to_px();
            let h = attrs.height.to_px();
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let mut path = BezPath::new();
            path.move_to(Point::new(x, y));
            path.line_to(Point::new(x + w, y));
            path.line_to(Point::new(x + w, y + h));
            path.line_to(Point::new(x, y + h));
            path.close();
            Some(path)
        }
        SvgElement::Circle(attrs) => {
            let cx = attrs.cx.to_px();
            let cy = attrs.cy.to_px();
            let r = attrs.r.to_px();
            if r <= 0.0 {
                return None;
            }
            Some(ellipse_to_bezpath(cx, cy, r, r))
        }
        SvgElement::Ellipse(attrs) => {
            let cx = attrs.cx.to_px();
            let cy = attrs.cy.to_px();
            let rx = attrs.rx.to_px();
            let ry = attrs.ry.to_px();
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            Some(ellipse_to_bezpath(cx, cy, rx, ry))
        }
        SvgElement::Path(attrs) => {
            if attrs.d.elements().is_empty() {
                None
            } else {
                Some(attrs.d.clone())
            }
        }
        SvgElement::Polygon(attrs) => {
            if attrs.points.len() >= 3 {
                Some(points_to_bezpath(&attrs.points, true))
            } else {
                None
            }
        }
        SvgElement::Polyline(attrs) => {
            if attrs.points.len() >= 2 {
                Some(points_to_bezpath(&attrs.points, false))
            } else {
                None
            }
        }
        SvgElement::Line(attrs) => {
            let mut path = BezPath::new();
            path.move_to(Point::new(attrs.x1.to_px(), attrs.y1.to_px()));
            path.line_to(Point::new(attrs.x2.to_px(), attrs.y2.to_px()));
            Some(path)
        }
        _ => None,
    }
}

/// Build a BezPath ellipse approximation using 4 cubic Bezier curves.
fn ellipse_to_bezpath(cx: f64, cy: f64, rx: f64, ry: f64) -> BezPath {
    const K: f64 = 0.5522847498;
    let kx = rx * K;
    let ky = ry * K;
    let mut path = BezPath::new();
    path.move_to(Point::new(cx + rx, cy));
    path.curve_to(
        Point::new(cx + rx, cy - ky),
        Point::new(cx + kx, cy - ry),
        Point::new(cx, cy - ry),
    );
    path.curve_to(
        Point::new(cx - kx, cy - ry),
        Point::new(cx - rx, cy - ky),
        Point::new(cx - rx, cy),
    );
    path.curve_to(
        Point::new(cx - rx, cy + ky),
        Point::new(cx - kx, cy + ry),
        Point::new(cx, cy + ry),
    );
    path.curve_to(
        Point::new(cx + kx, cy + ry),
        Point::new(cx + rx, cy + ky),
        Point::new(cx + rx, cy),
    );
    path.close();
    path
}

/// Transform all points in a BezPath by the given transform.
fn transform_bezpath(path: &BezPath, transform: &Transform2D) -> BezPath {
    if transform.is_identity() {
        return path.clone();
    }
    let mut result = BezPath::new();
    for seg in &path.elements() {
        match seg {
            sivgo_math::PathSegment::MoveTo(p) => {
                result.move_to(transform.transform_point(*p));
            }
            sivgo_math::PathSegment::LineTo(p) => {
                result.line_to(transform.transform_point(*p));
            }
            sivgo_math::PathSegment::QuadTo(c, p) => {
                result.quad_to(transform.transform_point(*c), transform.transform_point(*p));
            }
            sivgo_math::PathSegment::CurveTo(c1, c2, p) => {
                result.curve_to(
                    transform.transform_point(*c1),
                    transform.transform_point(*c2),
                    transform.transform_point(*p),
                );
            }
            sivgo_math::PathSegment::ClosePath => {
                result.close();
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Image data URI decoding
// ---------------------------------------------------------------------------

/// Decode a data URI containing a PNG image into RGBA pixel data.
fn decode_image_data_uri(href: &str) -> Option<(Vec<u8>, u32, u32)> {
    let href = href.trim();
    if !href.starts_with("data:") {
        return None;
    }
    let rest = &href[5..];
    let comma_pos = rest.find(',')?;
    let meta = &rest[..comma_pos];
    let encoded = &rest[comma_pos + 1..];

    if !meta.contains("base64") {
        return None;
    }

    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()?;

    if meta.contains("image/png") {
        decode_png_rgba(&decoded)
    } else if meta.contains("image/jpeg") || meta.contains("image/jpg") {
        decode_jpeg_rgba(&decoded)
    } else {
        None
    }
}

/// Decode a PNG byte slice into RGBA pixel data.
fn decode_png_rgba(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let width = info.width;
    let height = info.height;

    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in buf[..info.buffer_size()].chunks(3) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &v in &buf[..info.buffer_size()] {
                rgba.push(v);
                rgba.push(v);
                rgba.push(v);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in buf[..info.buffer_size()].chunks(2) {
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
            }
            rgba
        }
        _ => return None,
    };

    // Premultiply alpha (tiny-skia expects premultiplied RGBA).
    let mut premul = rgba;
    for pixel in premul.chunks_exact_mut(4) {
        let a = pixel[3] as u16;
        pixel[0] = ((pixel[0] as u16 * a + 127) / 255) as u8;
        pixel[1] = ((pixel[1] as u16 * a + 127) / 255) as u8;
        pixel[2] = ((pixel[2] as u16 * a + 127) / 255) as u8;
    }

    Some((premul, width, height))
}

/// Load an image from a file path. Supports PNG and JPEG.
fn load_image_from_path(path: &str) -> Option<(Vec<u8>, u32, u32)> {
    let data = std::fs::read(path).ok()?;
    // Try PNG first.
    if let Some(result) = decode_png_rgba(&data) {
        return Some(result);
    }
    // Try JPEG.
    decode_jpeg_rgba(&data)
}

/// Decode JPEG data to RGBA pixel data.
fn decode_jpeg_rgba(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let width = info.width as u32;
    let height = info.height as u32;
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks(3) {
                out.push(chunk[0]);
                out.push(chunk[1]);
                out.push(chunk[2]);
                out.push(255);
            }
            out
        }
        jpeg_decoder::PixelFormat::L8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for &v in &pixels {
                out.push(v);
                out.push(v);
                out.push(v);
                out.push(255);
            }
            out
        }
        _ => return None,
    };
    // Premultiply (all pixels are fully opaque from JPEG, so this is a no-op).
    Some((rgba, width, height))
}

/// Compute image placement within a viewport using preserveAspectRatio.
/// Returns `(x, y, width, height)` for the drawn image within the viewport.
fn compute_image_placement(
    img_w: f64,
    img_h: f64,
    vp_w: f64,
    vp_h: f64,
    par: &PreserveAspectRatio,
) -> (f64, f64, f64, f64) {
    if par.align == Align::None {
        return (0.0, 0.0, vp_w, vp_h);
    }

    let scale_x = vp_w / img_w;
    let scale_y = vp_h / img_h;

    let scale = match par.meet_or_slice {
        MeetOrSlice::Meet => scale_x.min(scale_y),
        MeetOrSlice::Slice => scale_x.max(scale_y),
    };

    let draw_w = img_w * scale;
    let draw_h = img_h * scale;

    let x = match par.align {
        Align::XMinYMin | Align::XMinYMid | Align::XMinYMax => 0.0,
        Align::XMidYMin | Align::XMidYMid | Align::XMidYMax => (vp_w - draw_w) / 2.0,
        Align::XMaxYMin | Align::XMaxYMid | Align::XMaxYMax => vp_w - draw_w,
        Align::None => unreachable!(),
    };

    let y = match par.align {
        Align::XMinYMin | Align::XMidYMin | Align::XMaxYMin => 0.0,
        Align::XMinYMid | Align::XMidYMid | Align::XMaxYMid => (vp_h - draw_h) / 2.0,
        Align::XMinYMax | Align::XMidYMax | Align::XMaxYMax => vp_h - draw_h,
        Align::None => unreachable!(),
    };

    (x, y, draw_w, draw_h)
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

/// Render an SVG document to an RGBA pixel buffer using the CPU backend.
///
/// Returns a `Vec<u8>` of length `width * height * 4` containing premultiplied
/// RGBA pixel data.
pub fn render_document_to_pixmap(doc: &SvgDocument, width: u32, height: u32) -> Vec<u8> {
    let mut backend = TinySkiaBackend::new(width, height);
    DocumentRenderer::render(doc, &mut backend);
    backend.pixels().to_vec()
}

// ---------------------------------------------------------------------------
// Filter pipeline building & region computation
// ---------------------------------------------------------------------------

/// Execute a filter pipeline with proper filter region computation.
///
/// This computes the filter region from the `<filter>` element's attributes and
/// the element's bounding box, then executes the pipeline with sub-region clipping.
fn execute_pipeline_with_region(
    doc: &SvgDocument,
    filter_node_id: NodeId,
    element_node_id: NodeId,
    transform: &Transform2D,
    pipeline: &FilterPipeline,
    source: &FilterBuffer,
) -> FilterBuffer {
    // Look up the <filter> element to get its attributes.
    let filter_node = doc.get_node(filter_node_id);
    let filter_attrs = filter_node.and_then(|n| {
        if let SvgElement::Filter(attrs) = &n.element {
            Some(attrs)
        } else {
            None
        }
    });

    let Some(filter_attrs) = filter_attrs else {
        // Fallback: no filter attributes found, run without region clipping.
        return pipeline.execute(source);
    };

    // Build region parameters from the <filter> element.
    let region_params = build_filter_region_params(filter_attrs);

    // Compute the element's bounding box in user-space coordinates.
    let element_bbox = compute_element_bbox_for_filter(doc, element_node_id, transform);

    // Compute the filter region.
    let computed_region = sivgo_filters::compute_filter_region(&region_params, element_bbox);

    // Execute with region clipping.
    pipeline.execute_with_region(source, &computed_region)
}

/// Build a `FilterPipeline` from a `<filter>` element's children in the DOM.
fn build_filter_pipeline(doc: &SvgDocument, filter_node_id: NodeId) -> FilterPipeline {
    let mut pipeline = FilterPipeline::new();
    let mut auto_idx = 0usize;

    let children: Vec<NodeId> = doc.children(filter_node_id).to_vec();
    for child_id in children {
        let Some(child) = doc.get_node(child_id) else {
            continue;
        };
        if child
            .attributes
            .get("data-sivgo-enabled")
            .or_else(|| child.attributes.get("sivgo:enabled"))
            .is_some_and(|value| value == "false" || value == "0")
        {
            continue;
        }

        match &child.element {
            SvgElement::FeGaussianBlur(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                pipeline.add(FilterPrimitive::GaussianBlur {
                    input: filter_input_to_string(&attrs.input),
                    std_dev_x: attrs.std_deviation_x,
                    std_dev_y: attrs.std_deviation_y,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeOffset(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                pipeline.add(FilterPrimitive::Offset {
                    input: filter_input_to_string(&attrs.input),
                    dx: attrs.dx,
                    dy: attrs.dy,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeFlood(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let color = flood_color_to_f32(attrs.flood_color.as_ref(), attrs.flood_opacity);
                pipeline.add(FilterPrimitive::Flood {
                    color,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeBlend(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                pipeline.add(FilterPrimitive::Blend {
                    input1: filter_input_to_string(&attrs.input),
                    input2: filter_input_to_string(&attrs.in2),
                    mode: convert_blend_mode(attrs.mode.as_ref()),
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeComposite(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let op = convert_composite_op(attrs);
                pipeline.add(FilterPrimitive::Composite {
                    input1: filter_input_to_string(&attrs.input),
                    input2: filter_input_to_string(&attrs.in2),
                    op,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeColorMatrix(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                if let Some(mt) = convert_color_matrix_type(attrs) {
                    pipeline.add(FilterPrimitive::ColorMatrix {
                        input: filter_input_to_string(&attrs.input),
                        matrix_type: mt,
                        result: result_name,
                        sub_region: convert_primitive_region(&attrs.common),
                    });
                }
            }
            SvgElement::FeMerge(_attrs) => {
                // Collect feMergeNode children.
                let merge_children: Vec<NodeId> = doc.children(child_id).to_vec();
                let mut inputs = Vec::new();
                for mc_id in merge_children {
                    let Some(mc) = doc.get_node(mc_id) else {
                        continue;
                    };
                    if let SvgElement::FeMergeNode(mn_attrs) = &mc.element {
                        inputs.push(filter_input_to_string(&mn_attrs.input));
                    }
                }
                let result_name = result_name(&_attrs.common.result, &mut auto_idx);
                pipeline.add(FilterPrimitive::Merge {
                    inputs,
                    result: result_name,
                    sub_region: convert_primitive_region(&_attrs.common),
                });
            }
            SvgElement::FeMorphology(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let op = match attrs.operator {
                    Some(DomMorphologyOp::Dilate) => sivgo_filters::MorphologyOp::Dilate,
                    _ => sivgo_filters::MorphologyOp::Erode,
                };
                pipeline.add(FilterPrimitive::Morphology {
                    input: filter_input_to_string(&attrs.input),
                    op,
                    radius_x: attrs.radius_x,
                    radius_y: attrs.radius_y,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeTurbulence(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let turb_type = match attrs.turbulence_type {
                    Some(DomTurbulenceType::FractalNoise) => {
                        sivgo_filters::turbulence::TurbulenceType::FractalNoise
                    }
                    _ => sivgo_filters::turbulence::TurbulenceType::Turbulence,
                };
                pipeline.add(FilterPrimitive::Turbulence {
                    turb_type,
                    base_freq_x: attrs.base_frequency_x,
                    base_freq_y: attrs.base_frequency_y,
                    num_octaves: attrs.num_octaves.unwrap_or(1).max(0) as u32,
                    seed: attrs.seed.unwrap_or(0.0) as i32,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeDropShadow(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let color = flood_color_to_f32(attrs.flood_color.as_ref(), attrs.flood_opacity);
                // DropShadow uses a single std_dev (average of x and y).
                let std_dev = (attrs.std_deviation_x + attrs.std_deviation_y) / 2.0;
                pipeline.add(FilterPrimitive::DropShadow {
                    input: filter_input_to_string(&attrs.input),
                    dx: attrs.dx,
                    dy: attrs.dy,
                    std_dev,
                    color,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeDisplacementMap(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                pipeline.add(FilterPrimitive::DisplacementMap {
                    input1: filter_input_to_string(&attrs.input),
                    input2: filter_input_to_string(&attrs.in2),
                    scale: attrs.scale.unwrap_or(0.0),
                    x_channel: convert_channel_selector(attrs.x_channel_selector),
                    y_channel: convert_channel_selector(attrs.y_channel_selector),
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            SvgElement::FeConvolveMatrix(attrs) => {
                let result_name = result_name(&attrs.common.result, &mut auto_idx);
                let order_x = attrs.order_x.max(1) as u32;
                let order_y = attrs.order_y.max(1) as u32;
                let target_x = attrs.target_x.unwrap_or((order_x / 2) as i32).max(0) as u32;
                let target_y = attrs.target_y.unwrap_or((order_y / 2) as i32).max(0) as u32;
                let divisor = attrs.divisor.unwrap_or(1.0) as f32;
                let kernel = attrs
                    .kernel_matrix
                    .iter()
                    .map(|value| *value as f32)
                    .collect();
                pipeline.add(FilterPrimitive::ConvolveMatrix {
                    input: filter_input_to_string(&attrs.input),
                    kernel,
                    order_x,
                    order_y,
                    divisor,
                    bias: attrs.bias as f32,
                    target_x,
                    target_y,
                    edge_mode: sivgo_filters::EdgeMode::Duplicate,
                    preserve_alpha: attrs.preserve_alpha,
                    result: result_name,
                    sub_region: convert_primitive_region(&attrs.common),
                });
            }
            _ => {
                // Unknown or unimplemented filter primitive — skip.
                log::warn!(
                    "Unimplemented filter primitive: {}",
                    child.element.tag_name()
                );
            }
        }
    }

    pipeline
}

/// Build a `FilterRegionParams` from a `<filter>` element's attributes.
fn build_filter_region_params(
    filter_attrs: &sivgo_dom::FilterAttrs,
) -> sivgo_filters::FilterRegionParams {
    use sivgo_filters::{FilterRegionParams, FilterUnits};

    let filter_units = match filter_attrs.filter_units {
        Some(DomGradientUnits::UserSpaceOnUse) => FilterUnits::UserSpaceOnUse,
        _ => FilterUnits::ObjectBoundingBox, // default
    };

    let primitive_units = match filter_attrs.primitive_units {
        Some(DomGradientUnits::ObjectBoundingBox) => FilterUnits::ObjectBoundingBox,
        _ => FilterUnits::UserSpaceOnUse, // default
    };

    FilterRegionParams {
        x: filter_attrs
            .x
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
        y: filter_attrs
            .y
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
        width: filter_attrs
            .width
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
        height: filter_attrs
            .height
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
        filter_units,
        primitive_units,
    }
}

/// Convert an `SvgLength` to a `FilterLength`.
fn svg_length_to_filter_length(l: &sivgo_dom::SvgLength) -> sivgo_filters::FilterLength {
    use sivgo_dom::LengthUnit;
    match l.unit {
        LengthUnit::Percent => sivgo_filters::FilterLength::Percent(l.value),
        _ => sivgo_filters::FilterLength::Absolute(l.to_px()),
    }
}

/// Convert DOM `FilterPrimitiveAttrs` (common sub-region attributes) to
/// `PrimitiveRegionParams`. Returns `None` if no sub-region attributes are set.
fn convert_primitive_region(
    common: &sivgo_dom::FilterPrimitiveAttrs,
) -> Option<sivgo_filters::PrimitiveRegionParams> {
    let has_any = common.x.is_some()
        || common.y.is_some()
        || common.width.is_some()
        || common.height.is_some();
    if !has_any {
        return None;
    }
    Some(sivgo_filters::PrimitiveRegionParams {
        x: common.x.as_ref().map(|l| svg_length_to_filter_length(l)),
        y: common.y.as_ref().map(|l| svg_length_to_filter_length(l)),
        width: common
            .width
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
        height: common
            .height
            .as_ref()
            .map(|l| svg_length_to_filter_length(l)),
    })
}

/// Compute the object bounding box for a given element in the DOM (post-transform).
///
/// Returns an `AbsoluteRect` representing the element's bounding box in user-space.
/// Falls back to a 100x100 rect at (0,0) if the bbox cannot be determined.
fn compute_element_bbox_for_filter(
    doc: &SvgDocument,
    node_id: NodeId,
    transform: &Transform2D,
) -> sivgo_filters::AbsoluteRect {
    let Some(node) = doc.get_node(node_id) else {
        return sivgo_filters::AbsoluteRect::new(0.0, 0.0, 100.0, 100.0);
    };

    // Try to compute bbox from element geometry.
    match &node.element {
        SvgElement::Rect(attrs) => {
            let x = attrs.x.to_px();
            let y = attrs.y.to_px();
            let w = attrs.width.to_px();
            let h = attrs.height.to_px();
            let p0 = transform.transform_point(Point::new(x, y));
            let p1 = transform.transform_point(Point::new(x + w, y + h));
            sivgo_filters::AbsoluteRect::new(
                p0.x.min(p1.x),
                p0.y.min(p1.y),
                (p1.x - p0.x).abs(),
                (p1.y - p0.y).abs(),
            )
        }
        SvgElement::Circle(attrs) => {
            let cx = attrs.cx.to_px();
            let cy = attrs.cy.to_px();
            let r = attrs.r.to_px();
            let p0 = transform.transform_point(Point::new(cx - r, cy - r));
            let p1 = transform.transform_point(Point::new(cx + r, cy + r));
            sivgo_filters::AbsoluteRect::new(
                p0.x.min(p1.x),
                p0.y.min(p1.y),
                (p1.x - p0.x).abs(),
                (p1.y - p0.y).abs(),
            )
        }
        SvgElement::Ellipse(attrs) => {
            let cx = attrs.cx.to_px();
            let cy = attrs.cy.to_px();
            let rx = attrs.rx.to_px();
            let ry = attrs.ry.to_px();
            let p0 = transform.transform_point(Point::new(cx - rx, cy - ry));
            let p1 = transform.transform_point(Point::new(cx + rx, cy + ry));
            sivgo_filters::AbsoluteRect::new(
                p0.x.min(p1.x),
                p0.y.min(p1.y),
                (p1.x - p0.x).abs(),
                (p1.y - p0.y).abs(),
            )
        }
        SvgElement::Line(attrs) => {
            let x1 = attrs.x1.to_px();
            let y1 = attrs.y1.to_px();
            let x2 = attrs.x2.to_px();
            let y2 = attrs.y2.to_px();
            let p0 = transform.transform_point(Point::new(x1.min(x2), y1.min(y2)));
            let p1 = transform.transform_point(Point::new(x1.max(x2), y1.max(y2)));
            sivgo_filters::AbsoluteRect::new(
                p0.x.min(p1.x),
                p0.y.min(p1.y),
                (p1.x - p0.x).abs().max(1.0),
                (p1.y - p0.y).abs().max(1.0),
            )
        }
        SvgElement::Path(attrs) => {
            let bbox = attrs.d.bounding_box();
            let p0 = transform.transform_point(Point::new(bbox.x0, bbox.y0));
            let p1 = transform.transform_point(Point::new(bbox.x1, bbox.y1));
            sivgo_filters::AbsoluteRect::new(
                p0.x.min(p1.x),
                p0.y.min(p1.y),
                (p1.x - p0.x).abs().max(1.0),
                (p1.y - p0.y).abs().max(1.0),
            )
        }
        SvgElement::Group(_) | SvgElement::Svg(_) | SvgElement::Anchor(_) => {
            // For groups, compute the union bbox of all children.
            let children = doc.children(node_id).to_vec();
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;
            for child_id in children {
                let child_bbox = compute_element_bbox_for_filter(doc, child_id, transform);
                min_x = min_x.min(child_bbox.x);
                min_y = min_y.min(child_bbox.y);
                max_x = max_x.max(child_bbox.x + child_bbox.width);
                max_y = max_y.max(child_bbox.y + child_bbox.height);
            }
            if min_x <= max_x && min_y <= max_y {
                sivgo_filters::AbsoluteRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
            } else {
                // No visible children — use a default.
                sivgo_filters::AbsoluteRect::new(0.0, 0.0, 100.0, 100.0)
            }
        }
        _ => {
            // Fallback for other element types.
            sivgo_filters::AbsoluteRect::new(0.0, 0.0, 100.0, 100.0)
        }
    }
}

/// Convert a DOM `FilterInput` option to a pipeline input string.
fn filter_input_to_string(input: &Option<FilterInput>) -> String {
    match input {
        Some(FilterInput::SourceGraphic) => "SourceGraphic".to_string(),
        Some(FilterInput::SourceAlpha) => "SourceAlpha".to_string(),
        Some(FilterInput::BackgroundImage) => "BackgroundImage".to_string(),
        Some(FilterInput::BackgroundAlpha) => "BackgroundAlpha".to_string(),
        Some(FilterInput::FillPaint) => "FillPaint".to_string(),
        Some(FilterInput::StrokePaint) => "StrokePaint".to_string(),
        Some(FilterInput::Reference(name)) => name.clone(),
        None => String::new(), // Empty = use implicit (last result)
    }
}

/// Generate or reuse a result name for a filter primitive.
fn result_name(explicit: &Option<String>, auto_idx: &mut usize) -> String {
    match explicit {
        Some(name) if !name.is_empty() => name.clone(),
        _ => {
            let name = format!("__auto_{}", *auto_idx);
            *auto_idx += 1;
            name
        }
    }
}

/// Convert a flood color + opacity to an [r, g, b, a] f32 array.
fn flood_color_to_f32(color: Option<&Color>, opacity: Option<f64>) -> [f32; 4] {
    let c = color.copied().unwrap_or(Color::BLACK);
    let a = opacity.unwrap_or(1.0) as f32;
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        a,
    ]
}

/// Convert a DOM `BlendMode` to a filters `BlendMode`.
fn convert_blend_mode(mode: Option<&DomBlendMode>) -> sivgo_filters::BlendMode {
    match mode {
        Some(DomBlendMode::Normal) | None => sivgo_filters::BlendMode::Normal,
        Some(DomBlendMode::Multiply) => sivgo_filters::BlendMode::Multiply,
        Some(DomBlendMode::Screen) => sivgo_filters::BlendMode::Screen,
        Some(DomBlendMode::Overlay) => sivgo_filters::BlendMode::Overlay,
        Some(DomBlendMode::Darken) => sivgo_filters::BlendMode::Darken,
        Some(DomBlendMode::Lighten) => sivgo_filters::BlendMode::Lighten,
        Some(DomBlendMode::ColorDodge) => sivgo_filters::BlendMode::ColorDodge,
        Some(DomBlendMode::ColorBurn) => sivgo_filters::BlendMode::ColorBurn,
        Some(DomBlendMode::HardLight) => sivgo_filters::BlendMode::HardLight,
        Some(DomBlendMode::SoftLight) => sivgo_filters::BlendMode::SoftLight,
        Some(DomBlendMode::Difference) => sivgo_filters::BlendMode::Difference,
        Some(DomBlendMode::Exclusion) => sivgo_filters::BlendMode::Exclusion,
        // Hue, Saturation, Color, Luminosity not supported in filter crate yet.
        Some(_) => sivgo_filters::BlendMode::Normal,
    }
}

/// Convert DOM `FeCompositeAttrs` to a filters `CompositeOp`.
fn convert_composite_op(attrs: &sivgo_dom::FeCompositeAttrs) -> sivgo_filters::CompositeOp {
    match attrs.operator {
        Some(DomCompositeOp::In) => sivgo_filters::CompositeOp::In,
        Some(DomCompositeOp::Out) => sivgo_filters::CompositeOp::Out,
        Some(DomCompositeOp::Atop) => sivgo_filters::CompositeOp::Atop,
        Some(DomCompositeOp::Xor) => sivgo_filters::CompositeOp::Xor,
        Some(DomCompositeOp::Arithmetic) => sivgo_filters::CompositeOp::Arithmetic {
            k1: attrs.k1 as f32,
            k2: attrs.k2 as f32,
            k3: attrs.k3 as f32,
            k4: attrs.k4 as f32,
        },
        Some(DomCompositeOp::Over) | None => sivgo_filters::CompositeOp::Over,
    }
}

fn convert_channel_selector(
    selector: Option<DomChannelSelector>,
) -> sivgo_filters::ChannelSelector {
    match selector {
        Some(DomChannelSelector::G) => sivgo_filters::ChannelSelector::G,
        Some(DomChannelSelector::B) => sivgo_filters::ChannelSelector::B,
        Some(DomChannelSelector::A) => sivgo_filters::ChannelSelector::A,
        Some(DomChannelSelector::R) | None => sivgo_filters::ChannelSelector::R,
    }
}

/// Convert DOM `FeColorMatrixAttrs` to a filters `ColorMatrixType`.
fn convert_color_matrix_type(
    attrs: &sivgo_dom::FeColorMatrixAttrs,
) -> Option<sivgo_filters::ColorMatrixType> {
    match attrs
        .matrix_type
        .as_ref()
        .unwrap_or(&DomColorMatrixType::Matrix)
    {
        DomColorMatrixType::Matrix => {
            let values_str = attrs.values.as_deref().unwrap_or("");
            let vals: Vec<f32> = values_str
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<f32>().ok())
                .collect();
            if vals.len() == 20 {
                let mut m = [0.0f32; 20];
                m.copy_from_slice(&vals);
                Some(sivgo_filters::ColorMatrixType::Matrix(m))
            } else {
                // Default: identity matrix.
                let m = [
                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                ];
                Some(sivgo_filters::ColorMatrixType::Matrix(m))
            }
        }
        DomColorMatrixType::Saturate => {
            let val: f32 = attrs
                .values
                .as_deref()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1.0);
            Some(sivgo_filters::ColorMatrixType::Saturate(val))
        }
        DomColorMatrixType::HueRotate => {
            let val: f32 = attrs
                .values
                .as_deref()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0.0);
            Some(sivgo_filters::ColorMatrixType::HueRotate(val))
        }
        DomColorMatrixType::LuminanceToAlpha => {
            Some(sivgo_filters::ColorMatrixType::LuminanceToAlpha)
        }
    }
}

// ---------------------------------------------------------------------------
// Pixel format conversion: premultiplied u8 <-> straight f32
// ---------------------------------------------------------------------------

/// Convert premultiplied u8 RGBA pixels to a straight (non-premultiplied) f32 `FilterBuffer`.
fn premultiplied_u8_to_straight_f32(data: &[u8], width: u32, height: u32) -> FilterBuffer {
    let pixel_count = (width as usize) * (height as usize);
    let mut buf = FilterBuffer::new(width, height);
    for i in 0..pixel_count {
        let idx = i * 4;
        let r_pre = data[idx] as f32 / 255.0;
        let g_pre = data[idx + 1] as f32 / 255.0;
        let b_pre = data[idx + 2] as f32 / 255.0;
        let a = data[idx + 3] as f32 / 255.0;
        if a > 0.0 {
            buf.data[idx] = (r_pre / a).min(1.0);
            buf.data[idx + 1] = (g_pre / a).min(1.0);
            buf.data[idx + 2] = (b_pre / a).min(1.0);
        }
        // else: RGB stays 0.0 (transparent pixel)
        buf.data[idx + 3] = a;
    }
    buf
}

/// Convert straight (non-premultiplied) f32 `FilterBuffer` to premultiplied u8 RGBA pixels.
fn straight_f32_to_premultiplied_u8(buf: &FilterBuffer) -> Vec<u8> {
    let pixel_count = (buf.width as usize) * (buf.height as usize);
    let mut data = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = i * 4;
        let r = buf.data[idx].clamp(0.0, 1.0);
        let g = buf.data[idx + 1].clamp(0.0, 1.0);
        let b = buf.data[idx + 2].clamp(0.0, 1.0);
        let a = buf.data[idx + 3].clamp(0.0, 1.0);
        // Premultiply
        data[idx] = (r * a * 255.0).round() as u8;
        data[idx + 1] = (g * a * 255.0).round() as u8;
        data[idx + 2] = (b * a * 255.0).round() as u8;
        data[idx + 3] = (a * 255.0).round() as u8;
    }
    data
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::*;

    /// Helper: build a minimal document with a single colored rect.
    fn doc_with_red_rect() -> SvgDocument {
        let mut doc = SvgDocument::new();

        // Root SVG element.
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // A red rect.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(80.0),
            ..RectAttrs::default()
        }));

        // Set fill to red.
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }

        doc.append_child(svg_id, rect_id);
        doc
    }

    #[test]
    fn render_red_rect_to_pixmap() {
        let doc = doc_with_red_rect();
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        assert_eq!(pixels.len(), 100 * 100 * 4);

        // The center pixel (50, 50) should be red.
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 255, "red channel");
        assert_eq!(pixels[idx + 1], 0, "green channel");
        assert_eq!(pixels[idx + 2], 0, "blue channel");
        assert_eq!(pixels[idx + 3], 255, "alpha channel");

        // A corner pixel (0, 0) should be white (background).
        let idx0 = 0;
        assert_eq!(pixels[idx0], 255);
        assert_eq!(pixels[idx0 + 1], 255);
        assert_eq!(pixels[idx0 + 2], 255);
        assert_eq!(pixels[idx0 + 3], 255);
    }

    #[test]
    fn render_empty_document() {
        let doc = SvgDocument::new();
        let pixels = render_document_to_pixmap(&doc, 50, 50);
        // Should be all white.
        assert_eq!(pixels.len(), 50 * 50 * 4);
        for chunk in pixels.chunks(4) {
            assert_eq!(chunk, &[255, 255, 255, 255]);
        }
    }

    #[test]
    fn render_circle_document() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let circle_id = doc.add_node(SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(50.0),
            cy: SvgLength::px(50.0),
            r: SvgLength::px(30.0),
        }));

        if let Some(node) = doc.get_node_mut(circle_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLUE));
        }

        doc.append_child(svg_id, circle_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Center should be blue.
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 0);
        assert_eq!(pixels[idx + 1], 0);
        assert_eq!(pixels[idx + 2], 255);
        assert_eq!(pixels[idx + 3], 255);
    }

    #[test]
    fn render_group_with_transform() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Group with translate(50, 50)
        let group_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        if let Some(node) = doc.get_node_mut(group_id) {
            node.presentation.transform = Some(Transform2D::translate(50.0, 50.0));
        }

        // Rect at (0,0)-(30,30) in group-local coords.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(30.0),
            height: SvgLength::px(30.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }

        doc.append_child(svg_id, group_id);
        doc.append_child(group_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 200, 200);
        // Rect should appear at (50,50)-(80,80).
        // Center of rect: (65, 65)
        let idx = (65 * 200 + 65) * 4;
        assert_eq!(pixels[idx], 255, "translated rect should be red");
        assert_eq!(pixels[idx + 1], 0);
        assert_eq!(pixels[idx + 2], 0);
        assert_eq!(pixels[idx + 3], 255);

        // Origin (0, 0) should be white.
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[1], 255);
        assert_eq!(pixels[2], 255);
    }

    #[test]
    fn render_line_document() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(10.0),
            y1: SvgLength::px(50.0),
            x2: SvgLength::px(90.0),
            y2: SvgLength::px(50.0),
        }));

        if let Some(node) = doc.get_node_mut(line_id) {
            node.presentation.stroke = Some(Paint::Color(Color::BLACK));
            node.presentation.stroke_width = Some(SvgLength::px(2.0));
        }

        doc.append_child(svg_id, line_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // The horizontal line at y=50 should have black pixels
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 0, "line pixel should be black");
        assert_eq!(pixels[idx + 1], 0);
        assert_eq!(pixels[idx + 2], 0);
        assert_eq!(pixels[idx + 3], 255);
    }

    #[test]
    fn render_defs_not_visible() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));

        // Put a red rect inside defs -- it should NOT be rendered.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }

        doc.append_child(svg_id, defs_id);
        doc.append_child(defs_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Should be all white.
        for chunk in pixels.chunks(4) {
            assert_eq!(
                chunk,
                &[255, 255, 255, 255],
                "defs content should not render"
            );
        }
    }

    #[test]
    fn render_path_document() {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // A triangle path.
        let d = BezPath::from_svg_path("M 10 90 L 50 10 L 90 90 Z").unwrap();
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d,
            ..Default::default()
        }));

        if let Some(node) = doc.get_node_mut(path_id) {
            node.presentation.fill = Some(Paint::Color(Color::new(0, 200, 0, 255)));
        }

        doc.append_child(svg_id, path_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Center of triangle (~50, 60) should be filled.
        let idx = (60 * 100 + 50) * 4;
        assert_ne!(
            &pixels[idx..idx + 4],
            &[255, 255, 255, 255],
            "triangle center should not be white"
        );
        // It should be the greenish color.
        assert_eq!(pixels[idx + 1], 200, "green channel should be 200");
    }

    #[test]
    fn render_non_empty_output() {
        // Smoke test: any non-trivial document produces non-all-white output.
        let doc = doc_with_red_rect();
        let pixels = render_document_to_pixmap(&doc, 100, 100);
        let all_white = pixels.chunks(4).all(|c| c == [255, 255, 255, 255]);
        assert!(!all_white, "rendered output should not be entirely white");
    }

    // -----------------------------------------------------------------------
    // New tests: gradient, viewBox, use element
    // -----------------------------------------------------------------------

    #[test]
    fn gradient_paint_resolves_from_document() {
        // Build a document with a <linearGradient> in <defs> and a rect
        // that references it.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // <linearGradient id="grad1" x1="0" y1="0" x2="100" y2="0"
        //   gradientUnits="userSpaceOnUse">
        let grad_id = doc.add_node(SvgElement::LinearGradient(LinearGradientAttrs {
            x1: Some(SvgLength::px(0.0)),
            y1: Some(SvgLength::px(0.0)),
            x2: Some(SvgLength::px(100.0)),
            y2: Some(SvgLength::px(0.0)),
            gradient_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            ..LinearGradientAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(grad_id) {
            node.xml_id = Some("grad1".to_string());
        }
        doc.append_child(defs_id, grad_id);

        // <stop offset="0" stop-color="red"/>
        let stop1_id = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::RED),
            stop_opacity: Some(1.0),
            ..StopAttrs::default()
        }));
        doc.append_child(grad_id, stop1_id);

        // <stop offset="1" stop-color="blue"/>
        let stop2_id = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 1.0,
            stop_color: Some(Color::BLUE),
            stop_opacity: Some(1.0),
            ..StopAttrs::default()
        }));
        doc.append_child(grad_id, stop2_id);

        // <rect x="0" y="0" width="100" height="100" fill="url(#grad1)"/>
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Url("#grad1".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Left edge (x=5) should be mostly red.
        let left_idx = (50 * 100 + 5) * 4;
        assert!(
            pixels[left_idx] > 200,
            "left red channel: {} should be > 200",
            pixels[left_idx]
        );
        assert!(
            pixels[left_idx + 2] < 50,
            "left blue channel: {} should be < 50",
            pixels[left_idx + 2]
        );

        // Right edge (x=95) should be mostly blue.
        let right_idx = (50 * 100 + 95) * 4;
        assert!(
            pixels[right_idx] < 50,
            "right red channel: {} should be < 50",
            pixels[right_idx]
        );
        assert!(
            pixels[right_idx + 2] > 200,
            "right blue channel: {} should be > 200",
            pixels[right_idx + 2]
        );
    }

    #[test]
    fn viewbox_scales_rendering() {
        // Create a document with viewBox="0 0 200 200" but render at 100x100.
        // A rect at (0,0)-(200,200) in viewBox coords should fill the whole
        // 100x100 render target when scaled down by 0.5x.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: Some(ViewBox::new(0.0, 0.0, 200.0, 200.0)),
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // A rect that spans the entire viewBox.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(200.0),
            height: SvgLength::px(200.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The center pixel should be red (not white).
        let center_idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[center_idx], 255, "center should be red R");
        assert_eq!(pixels[center_idx + 1], 0, "center should be red G=0");
        assert_eq!(pixels[center_idx + 2], 0, "center should be red B=0");
        assert_eq!(pixels[center_idx + 3], 255, "center should be opaque");

        // Corner pixels should also be red since the viewBox scales down
        // to fill exactly.
        let corner_idx = (1 * 100 + 1) * 4;
        assert_eq!(pixels[corner_idx], 255, "near-corner should be red R");
    }

    #[test]
    fn use_element_renders_referenced() {
        // <svg>
        //   <defs>
        //     <rect id="myRect" width="20" height="20" fill="green"/>
        //   </defs>
        //   <use href="#myRect" x="40" y="40"/>
        // </svg>
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(20.0),
            height: SvgLength::px(20.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.xml_id = Some("myRect".to_string());
            node.presentation.fill = Some(Paint::Color(Color::GREEN));
        }
        doc.append_child(defs_id, rect_id);

        let use_id = doc.add_node(SvgElement::Use(UseAttrs {
            href: Some("#myRect".to_string()),
            x: Some(SvgLength::px(40.0)),
            y: Some(SvgLength::px(40.0)),
            width: None,
            height: None,
        }));
        doc.append_child(svg_id, use_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The rect should appear at (40,40)-(60,60). Center is (50, 50).
        let center_idx = (50 * 100 + 50) * 4;
        // sivgo_dom GREEN = (0, 128, 0)
        assert_eq!(pixels[center_idx], 0, "use center R should be 0");
        assert_eq!(pixels[center_idx + 1], 128, "use center G should be 128");
        assert_eq!(pixels[center_idx + 2], 0, "use center B should be 0");
        assert_eq!(pixels[center_idx + 3], 255, "use center A should be 255");

        // Top-left corner (5, 5) should be white (untouched).
        let corner_idx = (5 * 100 + 5) * 4;
        assert_eq!(pixels[corner_idx], 255, "corner should be white R");
        assert_eq!(pixels[corner_idx + 1], 255, "corner should be white G");
        assert_eq!(pixels[corner_idx + 2], 255, "corner should be white B");
    }

    #[test]
    fn extract_url_id_works() {
        assert_eq!(extract_url_id("url(#foo)"), Some("foo".to_string()));
        assert_eq!(extract_url_id("url('#bar')"), Some("bar".to_string()));
        assert_eq!(extract_url_id("url(\"#baz\")"), Some("baz".to_string()));
        assert_eq!(extract_url_id("none"), None);
        assert_eq!(extract_url_id("url(#)"), None);
    }

    #[test]
    fn viewbox_transform_computation() {
        let vb = ViewBox::new(0.0, 0.0, 200.0, 200.0);
        let t = compute_viewbox_to_viewport_transform(&vb, 100.0, 100.0, None);
        let c = t.as_coeffs();
        // Scale should be 0.5 (100/200).
        assert!((c[0] - 0.5).abs() < 1e-9, "sx should be 0.5, got {}", c[0]);
        assert!((c[3] - 0.5).abs() < 1e-9, "sy should be 0.5, got {}", c[3]);
        // Translation should be 0 for centered viewBox starting at origin.
        assert!((c[4]).abs() < 1e-9, "tx should be 0, got {}", c[4]);
        assert!((c[5]).abs() < 1e-9, "ty should be 0, got {}", c[5]);
    }

    #[test]
    fn viewbox_preserve_aspect_ratio_none() {
        // preserveAspectRatio="none" → non-uniform stretch
        let vb = ViewBox::new(0.0, 0.0, 200.0, 100.0);
        let par = PreserveAspectRatio {
            align: Align::None,
            meet_or_slice: MeetOrSlice::Meet,
        };
        let t = compute_viewbox_to_viewport_transform(&vb, 400.0, 400.0, Some(&par));
        let c = t.as_coeffs();
        // sx = 400/200 = 2.0, sy = 400/100 = 4.0
        assert!((c[0] - 2.0).abs() < 1e-9, "sx should be 2.0, got {}", c[0]);
        assert!((c[3] - 4.0).abs() < 1e-9, "sy should be 4.0, got {}", c[3]);
    }

    #[test]
    fn viewbox_xminymin_meet() {
        // 200x100 viewBox into 400x400 viewport: meet → scale = min(2,4) = 2
        // xMinYMin → top-left alignment
        let vb = ViewBox::new(0.0, 0.0, 200.0, 100.0);
        let par = PreserveAspectRatio {
            align: Align::XMinYMin,
            meet_or_slice: MeetOrSlice::Meet,
        };
        let t = compute_viewbox_to_viewport_transform(&vb, 400.0, 400.0, Some(&par));
        let c = t.as_coeffs();
        assert!((c[0] - 2.0).abs() < 1e-9, "scale should be 2.0");
        assert!((c[3] - 2.0).abs() < 1e-9, "scale should be 2.0");
        // tx=0, ty=0 for xMin, yMin
        assert!((c[4]).abs() < 1e-9, "tx should be 0");
        assert!((c[5]).abs() < 1e-9, "ty should be 0");
    }

    #[test]
    fn viewbox_xmaxymax_meet() {
        // 200x100 viewBox into 400x400 viewport: meet → scale=2
        // xMaxYMax → bottom-right alignment
        let vb = ViewBox::new(0.0, 0.0, 200.0, 100.0);
        let par = PreserveAspectRatio {
            align: Align::XMaxYMax,
            meet_or_slice: MeetOrSlice::Meet,
        };
        let t = compute_viewbox_to_viewport_transform(&vb, 400.0, 400.0, Some(&par));
        let c = t.as_coeffs();
        assert!((c[0] - 2.0).abs() < 1e-9, "scale should be 2.0");
        // vb_scaled_w = 200*2 = 400, tx = 400-400 = 0
        assert!((c[4]).abs() < 1e-9, "tx should be 0 (vb fills width)");
        // vb_scaled_h = 100*2 = 200, ty = 400-200 = 200
        assert!(
            (c[5] - 200.0).abs() < 1e-9,
            "ty should be 200, got {}",
            c[5]
        );
    }

    #[test]
    fn viewbox_xmidymid_slice() {
        // 200x100 viewBox into 400x400 viewport: slice → scale = max(2,4) = 4
        // xMidYMid → centered
        let vb = ViewBox::new(0.0, 0.0, 200.0, 100.0);
        let par = PreserveAspectRatio {
            align: Align::XMidYMid,
            meet_or_slice: MeetOrSlice::Slice,
        };
        let t = compute_viewbox_to_viewport_transform(&vb, 400.0, 400.0, Some(&par));
        let c = t.as_coeffs();
        assert!((c[0] - 4.0).abs() < 1e-9, "scale should be 4.0");
        // vb_scaled_w = 200*4 = 800, tx = (400-800)/2 = -200
        assert!(
            (c[4] - (-200.0)).abs() < 1e-9,
            "tx should be -200, got {}",
            c[4]
        );
        // vb_scaled_h = 100*4 = 400, ty = (400-400)/2 = 0
        assert!((c[5]).abs() < 1e-9, "ty should be 0, got {}", c[5]);
    }

    #[test]
    fn gradient_elements_not_rendered_directly() {
        // Gradient elements in the tree should not be rendered, even outside <defs>.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // LinearGradient with a stop child -- should not render.
        let grad_id = doc.add_node(SvgElement::LinearGradient(LinearGradientAttrs::default()));
        doc.append_child(svg_id, grad_id);

        let stop_id = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::RED),
            stop_opacity: Some(1.0),
            ..StopAttrs::default()
        }));
        doc.append_child(grad_id, stop_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Should be all white.
        for chunk in pixels.chunks(4) {
            assert_eq!(
                chunk,
                &[255, 255, 255, 255],
                "gradient should not render directly"
            );
        }
    }

    #[test]
    fn display_none_skips_element_and_children() {
        // A group with display:none containing a red rect should produce
        // an all-white image because both the group and its children are skipped.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Group with display:none
        let group_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        {
            let group_node = doc.get_node_mut(group_id).unwrap();
            group_node.presentation.display = Some(Display::None);
        }
        doc.append_child(svg_id, group_id);

        // Red rect child of the hidden group
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        {
            let rect_node = doc.get_node_mut(rect_id).unwrap();
            rect_node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(group_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Should be all white (cleared), no red at all.
        for chunk in pixels.chunks(4) {
            assert_eq!(
                chunk,
                &[255, 255, 255, 255],
                "display:none group's children should not render"
            );
        }
    }

    #[test]
    fn visibility_hidden_skips_element_but_renders_children() {
        // A group with visibility:hidden containing a child rect with
        // visibility:visible should render the child rect.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Group with visibility:hidden
        let group_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        {
            let group_node = doc.get_node_mut(group_id).unwrap();
            group_node.presentation.visibility = Some(Visibility::Hidden);
        }
        doc.append_child(svg_id, group_id);

        // Red rect child that overrides visibility to visible
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        {
            let rect_node = doc.get_node_mut(rect_id).unwrap();
            rect_node.presentation.fill = Some(Paint::Color(Color::RED));
            rect_node.presentation.visibility = Some(Visibility::Visible);
        }
        doc.append_child(group_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // The child rect is visible, so there should be red pixels.
        let has_red = pixels
            .chunks(4)
            .any(|c| c[0] == 255 && c[1] == 0 && c[2] == 0);
        assert!(
            has_red,
            "visibility:hidden group should still render visible children"
        );
    }

    #[test]
    fn visibility_hidden_skips_own_drawing() {
        // A rect with visibility:hidden should not render.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        {
            let rect_node = doc.get_node_mut(rect_id).unwrap();
            rect_node.presentation.fill = Some(Paint::Color(Color::RED));
            rect_node.presentation.visibility = Some(Visibility::Hidden);
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Should be all white -- the hidden rect should not render.
        for chunk in pixels.chunks(4) {
            assert_eq!(
                chunk,
                &[255, 255, 255, 255],
                "visibility:hidden rect should not render"
            );
        }
    }

    /// Helper: create a minimal 2x2 red PNG and return it as a data URI string.
    fn make_red_2x2_png_data_uri() -> String {
        use base64::Engine;

        // Encode a 2x2 red PNG in-memory.
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(
                std::io::Cursor::new(&mut png_bytes),
                2, // width
                2, // height
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            // 2x2 RGBA: all red, fully opaque
            let data: [u8; 16] = [
                255, 0, 0, 255, // pixel (0,0)
                255, 0, 0, 255, // pixel (1,0)
                255, 0, 0, 255, // pixel (0,1)
                255, 0, 0, 255, // pixel (1,1)
            ];
            writer.write_image_data(&data).unwrap();
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        format!("data:image/png;base64,{}", b64)
    }

    #[test]
    fn render_image_element_from_data_uri() {
        let data_uri = make_red_2x2_png_data_uri();

        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Create an <image> element referencing the data URI, placed at (10, 10)
        // with size 80x80.
        let img_id = doc.add_node(SvgElement::Image(ImageAttrs {
            href: Some(data_uri),
            x: Some(SvgLength::px(10.0)),
            y: Some(SvgLength::px(10.0)),
            width: Some(SvgLength::px(80.0)),
            height: Some(SvgLength::px(80.0)),
            preserve_aspect_ratio: None,
        }));
        doc.append_child(svg_id, img_id);

        // Render -- should not panic.
        let pixels = render_document_to_pixmap(&doc, 100, 100);
        assert_eq!(pixels.len(), 100 * 100 * 4);

        // The center pixel (50,50) should be red (the image is scaled-up red).
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 255, "red channel at center");
        assert_eq!(pixels[idx + 1], 0, "green channel at center");
        assert_eq!(pixels[idx + 2], 0, "blue channel at center");
        assert_eq!(pixels[idx + 3], 255, "alpha channel at center");

        // A corner pixel (0, 0) outside the image should be white (background).
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[1], 255);
        assert_eq!(pixels[2], 255);
        assert_eq!(pixels[3], 255);
    }

    #[test]
    fn decode_image_data_uri_returns_none_for_invalid() {
        // Not a data URI.
        assert!(decode_image_data_uri("https://example.com/img.png").is_none());
        // Empty.
        assert!(decode_image_data_uri("").is_none());
        // No base64.
        assert!(decode_image_data_uri("data:image/png,rawdata").is_none());
        // Unsupported MIME type.
        assert!(decode_image_data_uri("data:image/jpeg;base64,/9j/4AAQ").is_none());
    }

    #[test]
    fn decode_image_data_uri_decodes_png() {
        let uri = make_red_2x2_png_data_uri();
        let result = decode_image_data_uri(&uri);
        assert!(result.is_some(), "should decode a valid PNG data URI");
        let (rgba, w, h) = result.unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(rgba.len(), 2 * 2 * 4);
        // First pixel should be premultiplied red (255, 0, 0, 255).
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn group_opacity_uses_offscreen_compositing() {
        // Build an SVG document with a <g opacity="0.5"> containing two
        // overlapping red rects. With correct group opacity (offscreen
        // rendering), the overlap region should look identical to the
        // non-overlap region, because the group is composited as a unit.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Group with opacity 0.5.
        let group_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        if let Some(node) = doc.get_node_mut(group_id) {
            node.presentation.opacity = Some(0.5);
        }
        doc.append_child(svg_id, group_id);

        // First red rect: (10,10)-(60,60).
        let rect1_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(50.0),
            height: SvgLength::px(50.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect1_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(group_id, rect1_id);

        // Second red rect: (40,40)-(90,90), overlapping the first.
        let rect2_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(40.0),
            y: SvgLength::px(40.0),
            width: SvgLength::px(50.0),
            height: SvgLength::px(50.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect2_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(group_id, rect2_id);

        // Render.
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Sample the overlap region (50, 50) and non-overlap region (30, 30).
        let overlap_idx = (50 * 100 + 50) * 4;
        let non_overlap_idx = (30 * 100 + 30) * 4;

        // With offscreen compositing, both should be the same color
        // (solid red in buffer -> composited at 50% onto white).
        assert_eq!(
            pixels[overlap_idx], pixels[non_overlap_idx],
            "R channel should match between overlap and non-overlap"
        );
        assert_eq!(
            pixels[overlap_idx + 1],
            pixels[non_overlap_idx + 1],
            "G channel should match between overlap and non-overlap"
        );
        assert_eq!(
            pixels[overlap_idx + 2],
            pixels[non_overlap_idx + 2],
            "B channel should match between overlap and non-overlap"
        );

        // The area should show 50% red blended onto white (not fully red).
        // G and B channels should be around 127 (white showing through).
        assert!(
            pixels[non_overlap_idx + 1] > 100 && pixels[non_overlap_idx + 1] < 160,
            "G should be ~127, got {}",
            pixels[non_overlap_idx + 1]
        );
    }

    // -----------------------------------------------------------------------
    // Pattern fill tests
    // -----------------------------------------------------------------------

    #[test]
    fn pattern_fill_user_space_on_use() {
        // Build an SVG document with a pattern defined in userSpaceOnUse
        // that contains a red 20x20 rect, and apply it as fill on a 100x100 rect.
        //
        //   <defs>
        //     <pattern id="pat1" x="0" y="0" width="20" height="20"
        //              patternUnits="userSpaceOnUse">
        //       <rect width="20" height="20" fill="red"/>
        //     </pattern>
        //   </defs>
        //   <rect width="100" height="100" fill="url(#pat1)"/>
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // Pattern element.
        let pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            width: Some(SvgLength::px(20.0)),
            height: Some(SvgLength::px(20.0)),
            pattern_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            pattern_content_units: None,
            pattern_transform: None,
            href: None,
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(pat_id) {
            node.xml_id = Some("pat1".to_string());
        }
        doc.append_child(defs_id, pat_id);

        // A red rect as pattern content.
        let pat_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(20.0),
            height: SvgLength::px(20.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(pat_rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(pat_id, pat_rect_id);

        // Main rect with pattern fill.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Url("#pat1".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The entire 100x100 area should be covered with red (the pattern
        // tile fills the whole tile, so tiling produces a solid red fill).
        let center_idx = (50 * 100 + 50) * 4;
        assert_eq!(
            pixels[center_idx], 255,
            "center R should be 255 (red), got {}",
            pixels[center_idx]
        );
        assert_eq!(
            pixels[center_idx + 1],
            0,
            "center G should be 0, got {}",
            pixels[center_idx + 1]
        );
        assert_eq!(
            pixels[center_idx + 2],
            0,
            "center B should be 0, got {}",
            pixels[center_idx + 2]
        );
        assert_eq!(
            pixels[center_idx + 3],
            255,
            "center A should be 255, got {}",
            pixels[center_idx + 3]
        );
    }

    #[test]
    fn pattern_fill_partial_tile() {
        // Build a pattern where the tile only covers half the area. The pattern
        // has a 10x10 red rect in a 20x20 tile, so after tiling, roughly half
        // of the area should be red and half should be transparent (showing
        // through to the white background).
        //
        //   <pattern width="20" height="20" patternUnits="userSpaceOnUse">
        //     <rect width="10" height="10" fill="red"/>
        //   </pattern>
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            width: Some(SvgLength::px(20.0)),
            height: Some(SvgLength::px(20.0)),
            pattern_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            pattern_content_units: None,
            pattern_transform: None,
            href: None,
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(pat_id) {
            node.xml_id = Some("pat2".to_string());
        }
        doc.append_child(defs_id, pat_id);

        // Red rect that only fills the top-left quarter of the tile.
        let pat_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(10.0),
            height: SvgLength::px(10.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(pat_rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(pat_id, pat_rect_id);

        // Main rect with pattern fill.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Url("#pat2".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Inside the red portion of the first tile (e.g., pixel (5, 5)):
        // should be red.
        let red_idx = (5 * 100 + 5) * 4;
        assert_eq!(
            pixels[red_idx], 255,
            "inside red portion R should be 255, got {}",
            pixels[red_idx]
        );
        assert_eq!(pixels[red_idx + 1], 0);
        assert_eq!(pixels[red_idx + 2], 0);

        // Inside the empty portion of the first tile (e.g., pixel (15, 15)):
        // should be white (background, since pattern is transparent there).
        let empty_idx = (15 * 100 + 15) * 4;
        assert_eq!(
            pixels[empty_idx], 255,
            "empty portion R should be 255 (white bg), got {}",
            pixels[empty_idx]
        );
        assert_eq!(
            pixels[empty_idx + 1],
            255,
            "empty portion G should be 255, got {}",
            pixels[empty_idx + 1]
        );
        assert_eq!(
            pixels[empty_idx + 2],
            255,
            "empty portion B should be 255, got {}",
            pixels[empty_idx + 2]
        );

        // Check that tiling works: pixel (25, 5) should also be red
        // (second tile repetition).
        let tiled_idx = (5 * 100 + 25) * 4;
        assert_eq!(
            pixels[tiled_idx], 255,
            "tiled red portion R should be 255, got {}",
            pixels[tiled_idx]
        );
        assert_eq!(pixels[tiled_idx + 1], 0);
        assert_eq!(pixels[tiled_idx + 2], 0);
    }

    #[test]
    fn pattern_fill_with_href_inheritance() {
        // Test that a pattern can inherit content from another pattern via href.
        //
        //   <defs>
        //     <pattern id="base_pat" width="20" height="20"
        //              patternUnits="userSpaceOnUse">
        //       <rect width="20" height="20" fill="blue"/>
        //     </pattern>
        //     <pattern id="derived_pat" href="#base_pat"/>
        //   </defs>
        //   <rect fill="url(#derived_pat)" width="100" height="100"/>
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // Base pattern with blue content.
        let base_pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            width: Some(SvgLength::px(20.0)),
            height: Some(SvgLength::px(20.0)),
            pattern_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            pattern_content_units: None,
            pattern_transform: None,
            href: None,
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(base_pat_id) {
            node.xml_id = Some("base_pat".to_string());
        }
        doc.append_child(defs_id, base_pat_id);

        let blue_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(20.0),
            height: SvgLength::px(20.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(blue_rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLUE));
        }
        doc.append_child(base_pat_id, blue_rect_id);

        // Derived pattern that inherits everything from base via href.
        let derived_pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            href: Some("#base_pat".to_string()),
            ..PatternAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(derived_pat_id) {
            node.xml_id = Some("derived_pat".to_string());
        }
        doc.append_child(defs_id, derived_pat_id);

        // Main rect with derived pattern fill.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Url("#derived_pat".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Should be solid blue.
        let center_idx = (50 * 100 + 50) * 4;
        assert_eq!(
            pixels[center_idx], 0,
            "center R should be 0 (blue), got {}",
            pixels[center_idx]
        );
        assert_eq!(
            pixels[center_idx + 1],
            0,
            "center G should be 0, got {}",
            pixels[center_idx + 1]
        );
        assert_eq!(
            pixels[center_idx + 2],
            255,
            "center B should be 255, got {}",
            pixels[center_idx + 2]
        );
    }

    #[test]
    fn pattern_resolve_returns_none_for_zero_size() {
        // A pattern with zero width or height should return None.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            width: Some(SvgLength::px(0.0)),
            height: Some(SvgLength::px(20.0)),
            pattern_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            ..PatternAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(pat_id) {
            node.xml_id = Some("zeroPat".to_string());
        }
        doc.append_child(defs_id, pat_id);

        // A rect referencing the zero-size pattern: should fall back to default
        // fill (black) or no fill, depending on whether resolve_paint_server
        // returns None. resolve_fill_paint falls back to black if the URL
        // resolves to None.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Url("#zeroPat".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        // Should not crash. The rect will not be filled with a pattern
        // (since the pattern has zero width).
        let pixels = render_document_to_pixmap(&doc, 100, 100);
        assert_eq!(pixels.len(), 100 * 100 * 4);
    }

    #[test]
    fn pattern_fill_stroke() {
        // Test that a pattern can be used for stroke as well.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // Red pattern.
        let pat_id = doc.add_node(SvgElement::Pattern(PatternAttrs {
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            width: Some(SvgLength::px(10.0)),
            height: Some(SvgLength::px(10.0)),
            pattern_units: Some(sivgo_dom::GradientUnits::UserSpaceOnUse),
            ..PatternAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(pat_id) {
            node.xml_id = Some("strokePat".to_string());
        }
        doc.append_child(defs_id, pat_id);

        let pat_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(10.0),
            height: SvgLength::px(10.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(pat_rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(pat_id, pat_rect_id);

        // A rect with no fill and a pattern stroke.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(80.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::None);
            node.presentation.stroke = Some(Paint::Url("#strokePat".to_string()));
            node.presentation.stroke_width = Some(SvgLength::px(4.0));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The stroke at the top edge (y=10, x=50) should be red from the pattern.
        let stroke_idx = (10 * 100 + 50) * 4;
        assert_eq!(
            pixels[stroke_idx], 255,
            "stroke R should be 255 (red pattern), got {}",
            pixels[stroke_idx]
        );
        assert_eq!(pixels[stroke_idx + 1], 0);
        assert_eq!(pixels[stroke_idx + 2], 0);

        // The center (50, 50) should be white (no fill).
        let center_idx = (50 * 100 + 50) * 4;
        assert_eq!(
            pixels[center_idx], 255,
            "center R should be 255 (white bg), got {}",
            pixels[center_idx]
        );
        assert_eq!(pixels[center_idx + 1], 255);
        assert_eq!(pixels[center_idx + 2], 255);
    }

    // -----------------------------------------------------------------------
    // Marker rendering tests
    // -----------------------------------------------------------------------

    #[test]
    fn marker_on_line_does_not_crash() {
        // A line element with marker-start and marker-end should render
        // without crashing, even if the markers are minimal.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Defs with a marker containing a small red circle.
        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let marker_id = doc.add_node(SvgElement::Marker(MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: Some(ViewBox::new(0.0, 0.0, 10.0, 10.0)),
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(marker_id) {
            node.xml_id = Some("arrow".to_string());
        }
        doc.append_child(defs_id, marker_id);

        // Child of marker: a red circle.
        let circle_id = doc.add_node(SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(5.0),
            cy: SvgLength::px(5.0),
            r: SvgLength::px(4.0),
        }));
        if let Some(node) = doc.get_node_mut(circle_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(marker_id, circle_id);

        // A line with marker-start and marker-end.
        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(20.0),
            y1: SvgLength::px(100.0),
            x2: SvgLength::px(180.0),
            y2: SvgLength::px(100.0),
        }));
        if let Some(node) = doc.get_node_mut(line_id) {
            node.presentation.stroke = Some(Paint::Color(Color::BLACK));
            node.presentation.stroke_width = Some(SvgLength::px(2.0));
            node.presentation.marker_start = Some("url(#arrow)".to_string());
            node.presentation.marker_end = Some("url(#arrow)".to_string());
        }
        doc.append_child(svg_id, line_id);

        // Should not crash.
        let pixels = render_document_to_pixmap(&doc, 200, 200);
        assert_eq!(pixels.len(), 200 * 200 * 4);
    }

    #[test]
    fn marker_renders_content_at_path_endpoints() {
        // Verify that marker content actually appears at the path endpoints.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // Marker with a large red rect, using userSpaceOnUse so the
        // marker content renders at a predictable size.
        let marker_id = doc.add_node(SvgElement::Marker(MarkerAttrs {
            ref_x: Some(SvgLength::px(0.0)),
            ref_y: Some(SvgLength::px(0.0)),
            marker_width: Some(SvgLength::px(20.0)),
            marker_height: Some(SvgLength::px(20.0)),
            orient: Some(Orient::Angle(0.0)),
            marker_units: Some(MarkerUnits::UserSpaceOnUse),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(marker_id) {
            node.xml_id = Some("dot".to_string());
        }
        doc.append_child(defs_id, marker_id);

        // A red rect inside the marker.
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(10.0),
            height: SvgLength::px(10.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(marker_id, rect_id);

        // A horizontal line from (10, 50) to (90, 50) with marker-start.
        let line_id = doc.add_node(SvgElement::Line(LineAttrs {
            x1: SvgLength::px(10.0),
            y1: SvgLength::px(50.0),
            x2: SvgLength::px(90.0),
            y2: SvgLength::px(50.0),
        }));
        if let Some(node) = doc.get_node_mut(line_id) {
            node.presentation.stroke = Some(Paint::Color(Color::BLACK));
            node.presentation.stroke_width = Some(SvgLength::px(1.0));
            node.presentation.marker_start = Some("url(#dot)".to_string());
        }
        doc.append_child(svg_id, line_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The marker start is at (10, 50). The marker renders a red 10x10 rect
        // at (0,0) relative to the marker origin, which after translating to
        // vertex position (10, 50) becomes roughly (10..20, 50..60).
        // Check pixel at (15, 55) -- should be red.
        let idx = (55 * 100 + 15) * 4;
        assert_eq!(pixels[idx], 255, "marker content R at start vertex");
        assert_eq!(pixels[idx + 1], 0, "marker content G at start vertex");
        assert_eq!(pixels[idx + 2], 0, "marker content B at start vertex");
    }

    #[test]
    fn marker_mid_renders_at_intermediate_vertices() {
        // A polyline with 3 points should place marker-mid at the middle point.
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let marker_id = doc.add_node(SvgElement::Marker(MarkerAttrs {
            ref_x: Some(SvgLength::px(0.0)),
            ref_y: Some(SvgLength::px(0.0)),
            marker_width: Some(SvgLength::px(20.0)),
            marker_height: Some(SvgLength::px(20.0)),
            orient: Some(Orient::Angle(0.0)),
            marker_units: Some(MarkerUnits::UserSpaceOnUse),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        if let Some(node) = doc.get_node_mut(marker_id) {
            node.xml_id = Some("mid-dot".to_string());
        }
        doc.append_child(defs_id, marker_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(8.0),
            height: SvgLength::px(8.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLUE));
        }
        doc.append_child(marker_id, rect_id);

        // Polyline: (10,50) -> (50,50) -> (90,50)
        let polyline_id = doc.add_node(SvgElement::Polyline(PolylineAttrs {
            points: vec![(10.0, 50.0), (50.0, 50.0), (90.0, 50.0)],
        }));
        if let Some(node) = doc.get_node_mut(polyline_id) {
            node.presentation.stroke = Some(Paint::Color(Color::BLACK));
            node.presentation.stroke_width = Some(SvgLength::px(1.0));
            node.presentation.marker_mid = Some("url(#mid-dot)".to_string());
        }
        doc.append_child(svg_id, polyline_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // The mid marker is at (50, 50). The marker renders an 8x8 blue rect
        // offset from (50,50) -> check pixel at (54, 54).
        let idx = (54 * 100 + 54) * 4;
        assert_eq!(pixels[idx], 0, "marker-mid R at intermediate vertex");
        assert_eq!(pixels[idx + 1], 0, "marker-mid G at intermediate vertex");
        assert_eq!(pixels[idx + 2], 255, "marker-mid B at intermediate vertex");
    }

    #[test]
    fn get_path_vertices_extracts_correct_count() {
        // A simple triangle path M 0 0 L 100 0 L 50 100 Z should have 3 vertices.
        let path = BezPath::from_svg_path("M 0 0 L 100 0 L 50 100 Z").unwrap();
        let vertices = get_path_vertices(&path);
        assert_eq!(vertices.len(), 3, "triangle should have 3 vertices");
    }

    #[test]
    fn get_path_vertices_empty_path() {
        let path = BezPath::new();
        let vertices = get_path_vertices(&path);
        assert!(vertices.is_empty(), "empty path should have no vertices");
    }

    #[test]
    fn get_path_vertices_line_segment() {
        // M 0 0 L 100 0 -- should have 2 vertices.
        let path = BezPath::from_svg_path("M 0 0 L 100 0").unwrap();
        let vertices = get_path_vertices(&path);
        assert_eq!(vertices.len(), 2, "line should have 2 vertices");

        // First vertex: outgoing angle is 0 (pointing right).
        assert!(
            (vertices[0].angle - 0.0).abs() < 0.01,
            "start angle should be 0, got {}",
            vertices[0].angle
        );
        // Last vertex: incoming angle is 0 (came from left).
        assert!(
            (vertices[1].angle - 0.0).abs() < 0.01,
            "end angle should be 0, got {}",
            vertices[1].angle
        );
    }

    #[test]
    fn auto_start_reverse_flips_first_marker() {
        // Test that orient=auto-start-reverse adds PI to the first marker angle.
        let path = BezPath::from_svg_path("M 0 0 L 100 0").unwrap();
        let vertices = get_path_vertices(&path);
        assert_eq!(vertices.len(), 2);
        // angle at start: 0 (pointing right)
        // auto-start-reverse would add PI to get pointing left.
        let reversed = vertices[0].angle + std::f64::consts::PI;
        assert!(
            (reversed - std::f64::consts::PI).abs() < 0.01,
            "reversed angle should be PI, got {}",
            reversed
        );
    }

    #[test]
    fn marker_parsed_from_svg_string() {
        // Test rendering markers from a parsed SVG string.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs>
                <marker id="m1" refX="5" refY="5" markerWidth="10" markerHeight="10"
                        orient="auto" markerUnits="userSpaceOnUse" viewBox="0 0 10 10">
                    <circle cx="5" cy="5" r="4" fill="red"/>
                </marker>
            </defs>
            <line x1="10" y1="50" x2="90" y2="50"
                  stroke="black" stroke-width="2"
                  marker-start="url(#m1)" marker-end="url(#m1)"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Should produce valid output without crashing.
        assert_eq!(pixels.len(), 100 * 100 * 4);

        // The marker content (red circles) should appear near the line endpoints.
        // Check near the start at (10, 50).
        let start_idx = (50 * 100 + 10) * 4;
        // The pixel at the start vertex should be non-white (either from
        // the stroke or from the marker).
        let is_not_white = pixels[start_idx] != 255
            || pixels[start_idx + 1] != 255
            || pixels[start_idx + 2] != 255;
        assert!(is_not_white, "marker start area should not be pure white");
    }

    // -----------------------------------------------------------------------
    // Filter pipeline integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn filter_gaussian_blur_applied_to_rect() {
        // A red rect with feGaussianBlur filter should produce blurred edges.
        // Use a wider filter region to ensure blur spread is visible.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs>
                <filter id="blur1" x="-25%" y="-25%" width="150%" height="150%">
                    <feGaussianBlur in="SourceGraphic" stdDeviation="3"/>
                </filter>
            </defs>
            <rect x="30" y="30" width="40" height="40" fill="red" filter="url(#blur1)"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Center (50,50) should still be red.
        let center = (50 * 100 + 50) * 4;
        assert!(pixels[center] > 200, "center red channel should be bright");

        // Just outside the rect edge but inside the filter region (27, 50) should
        // have some red from blur. The rect starts at x=30 and filter region starts
        // at x=20 (30 - 0.25*40 = 20), so x=27 is well within the filter region.
        let edge = (50 * 100 + 27) * 4;
        assert!(
            pixels[edge] > 0 && pixels[edge] < 255,
            "edge area should show blur spread (R={})",
            pixels[edge]
        );
    }

    #[test]
    fn filter_flood_fills_with_color() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <defs>
                <filter id="f1">
                    <feFlood flood-color="blue" flood-opacity="1"/>
                </filter>
            </defs>
            <rect x="0" y="0" width="50" height="50" fill="red" filter="url(#f1)"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 50, 50);

        // The flood should replace everything with blue.
        let center = (25 * 50 + 25) * 4;
        assert_eq!(pixels[center], 0, "R should be 0 (blue flood)");
        assert_eq!(pixels[center + 1], 0, "G should be 0 (blue flood)");
        assert!(pixels[center + 2] > 200, "B should be bright (blue flood)");
    }

    #[test]
    fn disabled_filter_primitive_is_skipped() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <defs>
                <filter id="f1">
                    <feFlood data-sivgo-enabled="false" flood-color="blue" flood-opacity="1"/>
                </filter>
            </defs>
            <rect x="0" y="0" width="50" height="50" fill="red" filter="url(#f1)"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 50, 50);
        let center = (25 * 50 + 25) * 4;

        assert!(pixels[center] > 200, "R should remain red");
        assert!(pixels[center + 2] < 20, "B should not be flood blue");
    }

    #[test]
    fn filter_offset_shifts_content() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs>
                <filter id="f1">
                    <feOffset in="SourceGraphic" dx="20" dy="0"/>
                </filter>
            </defs>
            <rect x="10" y="40" width="20" height="20" fill="red" filter="url(#f1)"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        // Original rect was at x=10..30. After dx=20 offset, it should be at x=30..50.
        // Check that (40, 50) is red.
        let shifted = (50 * 100 + 40) * 4;
        assert!(
            pixels[shifted] > 200,
            "shifted area should be red (R={})",
            pixels[shifted]
        );

        // Original location (15, 50) should now be white (background).
        let orig = (50 * 100 + 15) * 4;
        assert!(
            pixels[orig] > 240,
            "original location should be white-ish (R={})",
            pixels[orig]
        );
    }

    #[test]
    fn filter_element_not_found_composites_unfiltered() {
        // Filter references a non-existent element — should render unfiltered.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <rect x="0" y="0" width="50" height="50" fill="red" filter="url(#nonexistent)"/>
        </svg>"##;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let pixels = render_document_to_pixmap(&doc, 50, 50);

        // Should still render the red rect.
        let center = (25 * 50 + 25) * 4;
        assert!(
            pixels[center] > 200,
            "should render unfiltered red rect (R={})",
            pixels[center]
        );
    }

    #[test]
    fn pixel_format_roundtrip() {
        // Test that premultiplied u8 -> straight f32 -> premultiplied u8 is lossless-ish.
        let premul = vec![
            128u8, 0, 0, 128, // red at 50% alpha (premultiplied)
            0, 255, 0, 255, // green opaque
            0, 0, 0, 0, // transparent
            255, 255, 255, 255, // white opaque
        ];
        let buf = premultiplied_u8_to_straight_f32(&premul, 2, 2);
        let back = straight_f32_to_premultiplied_u8(&buf);

        // Pixel 0: premul (128, 0, 0, 128) → straight (1.0, 0, 0, 0.502)
        // → premul back ≈ (128, 0, 0, 128)
        assert!(
            (back[0] as i16 - 128).abs() <= 1,
            "R roundtrip: {}",
            back[0]
        );
        assert_eq!(back[1], 0);
        assert!(
            (back[3] as i16 - 128).abs() <= 1,
            "A roundtrip: {}",
            back[3]
        );

        // Pixel 1: opaque green
        assert_eq!(back[4], 0);
        assert_eq!(back[5], 255);
        assert_eq!(back[7], 255);

        // Pixel 2: transparent
        assert_eq!(back[8], 0);
        assert_eq!(back[11], 0);

        // Pixel 3: white
        assert_eq!(back[12], 255);
        assert_eq!(back[13], 255);
        assert_eq!(back[14], 255);
        assert_eq!(back[15], 255);
    }

    #[test]
    fn mix_blend_mode_multiply_differs_from_normal() {
        // Render two overlapping rects where the top one has mix-blend-mode: multiply.
        // Compare against the same SVG without blend mode to verify different output.

        // Helper: build a document with a green background rect and a red rect
        // on top, optionally with a blend mode on the red rect.
        fn build_doc(blend: Option<BlendMode>) -> SvgDocument {
            let mut doc = SvgDocument::new();
            let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
                width: Some(SvgLength::px(100.0)),
                height: Some(SvgLength::px(100.0)),
                view_box: None,
                preserve_aspect_ratio: None,
            }));
            doc.set_root(svg_id);

            // Green background rect.
            let bg_id = doc.add_node(SvgElement::Rect(RectAttrs {
                x: SvgLength::px(0.0),
                y: SvgLength::px(0.0),
                width: SvgLength::px(100.0),
                height: SvgLength::px(100.0),
                ..RectAttrs::default()
            }));
            if let Some(node) = doc.get_node_mut(bg_id) {
                node.presentation.fill = Some(Paint::Color(Color {
                    r: 0,
                    g: 200,
                    b: 0,
                    a: 255,
                }));
            }
            doc.append_child(svg_id, bg_id);

            // Red foreground rect with optional blend mode.
            let fg_id = doc.add_node(SvgElement::Rect(RectAttrs {
                x: SvgLength::px(20.0),
                y: SvgLength::px(20.0),
                width: SvgLength::px(60.0),
                height: SvgLength::px(60.0),
                ..RectAttrs::default()
            }));
            if let Some(node) = doc.get_node_mut(fg_id) {
                node.presentation.fill = Some(Paint::Color(Color::RED));
                node.presentation.mix_blend_mode = blend;
            }
            doc.append_child(svg_id, fg_id);

            doc
        }

        // Render without blend mode (Normal / source-over).
        let doc_normal = build_doc(None);
        let mut backend_normal = TinySkiaBackend::new(100, 100);
        DocumentRenderer::render(&doc_normal, &mut backend_normal);

        // Render with Multiply blend mode.
        let doc_multiply = build_doc(Some(BlendMode::Multiply));
        let mut backend_multiply = TinySkiaBackend::new(100, 100);
        DocumentRenderer::render(&doc_multiply, &mut backend_multiply);

        let normal_px = backend_normal.pixels();
        let mult_px = backend_multiply.pixels();

        // In the overlap area (50, 50), results should differ.
        let idx = (50 * 100 + 50) * 4;

        // Normal: red fully covers green → R=255, G=0, B=0
        assert_eq!(normal_px[idx], 255, "normal: R=255");
        assert_eq!(normal_px[idx + 1], 0, "normal: G=0");

        // Multiply: red * green → R=255*0/255=0, G=0*200/255=0 → near black
        assert!(
            mult_px[idx] < 10,
            "multiply should have low R, got {}",
            mult_px[idx]
        );
        assert!(
            mult_px[idx + 1] < 10,
            "multiply should have low G, got {}",
            mult_px[idx + 1]
        );

        // Outside the foreground rect (5, 5) both should be the same green.
        let out_idx = (5 * 100 + 5) * 4;
        assert_eq!(
            normal_px[out_idx], mult_px[out_idx],
            "outside: R should match"
        );
        assert_eq!(
            normal_px[out_idx + 1],
            mult_px[out_idx + 1],
            "outside: G should match"
        );
    }

    #[test]
    fn mix_blend_mode_applies_to_group_children() {
        // A group with mix-blend-mode should apply the blend to all its children
        // as a single composited unit.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // White background rect (the clear is white, but let's be explicit).
        let bg_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(bg_id) {
            node.presentation.fill = Some(Paint::Color(Color {
                r: 0,
                g: 0,
                b: 200,
                a: 255,
            }));
        }
        doc.append_child(svg_id, bg_id);

        // Group with Multiply blend mode containing a red rect.
        let group_id = doc.add_node(SvgElement::Group(GroupAttrs::default()));
        if let Some(node) = doc.get_node_mut(group_id) {
            node.presentation.mix_blend_mode = Some(BlendMode::Multiply);
        }
        doc.append_child(svg_id, group_id);

        let child_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(80.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(child_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(group_id, child_id);

        let mut backend = TinySkiaBackend::new(100, 100);
        DocumentRenderer::render(&doc, &mut backend);

        let px = backend.pixels();
        let idx = (50 * 100 + 50) * 4;

        // Multiply of red (255,0,0) onto blue (0,0,200):
        // R = 255*0/255 = 0, G = 0*0/255 = 0, B = 0*200/255 = 0
        // Result should be near black.
        assert!(
            px[idx] < 10,
            "group multiply R should be near 0, got {}",
            px[idx]
        );
        assert!(
            px[idx + 2] < 10,
            "group multiply B should be near 0, got {}",
            px[idx + 2]
        );

        // Outside the group's rect should be the blue background.
        let out_idx = (5 * 100 + 5) * 4;
        assert_eq!(px[out_idx + 2], 200, "outside should be blue");
    }

    // -----------------------------------------------------------------------
    // SVG 1.2 Flowed Text tests
    // -----------------------------------------------------------------------

    /// Build a minimal flowRoot document for testing.
    ///
    /// Creates a 200x200 SVG with a flowRoot containing a flowRegion (rect)
    /// and one flowPara with the given text. The flow region is placed at
    /// (10, 10) with size 180x180.
    fn doc_with_flow_text(text: &str) -> SvgDocument {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // flowRoot
        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        if let Some(node) = doc.get_node_mut(flow_root_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
            node.presentation.font_size = Some(SvgLength::px(16.0));
        }
        doc.append_child(svg_id, flow_root_id);

        // flowRegion containing a rect
        let flow_region_id = doc.add_node(SvgElement::FlowRegion(FlowRegionAttrs::default()));
        doc.append_child(flow_root_id, flow_region_id);

        let region_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(180.0),
            height: SvgLength::px(180.0),
            ..RectAttrs::default()
        }));
        doc.append_child(flow_region_id, region_rect_id);

        // flowPara with text
        let flow_para_id = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: text.to_string(),
        }));
        doc.append_child(flow_root_id, flow_para_id);

        doc
    }

    #[test]
    fn render_flow_text_basic() {
        let doc = doc_with_flow_text("Hello");
        let pixels = render_document_to_pixmap(&doc, 200, 200);
        assert_eq!(pixels.len(), 200 * 200 * 4);

        // Some pixel within the flow region text area should be non-white
        // (text was rendered). Check a band of pixels around where text
        // should appear (y ~= 10 + ascent, x ~= 10..190).
        let mut found_non_white = false;
        for y in 10..60 {
            for x in 10..190 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_non_white = true;
                    break;
                }
            }
            if found_non_white {
                break;
            }
        }
        assert!(
            found_non_white,
            "Flow text should render non-white pixels in the region"
        );
    }

    #[test]
    fn render_flow_text_empty_para() {
        let doc = doc_with_flow_text("");
        let pixels = render_document_to_pixmap(&doc, 200, 200);
        // All pixels should be white (no text to render).
        for chunk in pixels.chunks(4) {
            assert_eq!(
                chunk,
                &[255, 255, 255, 255],
                "Empty flow text should be white"
            );
        }
    }

    #[test]
    fn render_flow_text_no_region() {
        // flowRoot without a flowRegion -- should not crash.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        doc.append_child(svg_id, flow_root_id);

        let flow_para_id = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: "Should not crash".to_string(),
        }));
        doc.append_child(flow_root_id, flow_para_id);

        let pixels = render_document_to_pixmap(&doc, 100, 100);
        // Should be all white (no region means no rendering).
        for chunk in pixels.chunks(4) {
            assert_eq!(chunk, &[255, 255, 255, 255]);
        }
    }

    #[test]
    fn render_flow_text_wraps_long_text() {
        // Create a narrow region so text must wrap.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        if let Some(node) = doc.get_node_mut(flow_root_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
            node.presentation.font_size = Some(SvgLength::px(14.0));
        }
        doc.append_child(svg_id, flow_root_id);

        // Narrow region: 60px wide, 180px tall.
        let flow_region_id = doc.add_node(SvgElement::FlowRegion(FlowRegionAttrs::default()));
        doc.append_child(flow_root_id, flow_region_id);

        let region_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(60.0),
            height: SvgLength::px(180.0),
            ..RectAttrs::default()
        }));
        doc.append_child(flow_region_id, region_rect_id);

        let flow_para_id = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: "Hello World Test".to_string(),
        }));
        doc.append_child(flow_root_id, flow_para_id);

        let pixels = render_document_to_pixmap(&doc, 200, 200);

        // Text should render on multiple lines. Check for non-white pixels
        // in two vertical bands (line 1 and line 2).
        let mut found_line1 = false;
        let mut found_line2 = false;

        // Line 1: approximately y = 10..30 (first line of text)
        for y in 10..35 {
            for x in 10..70 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_line1 = true;
                    break;
                }
            }
            if found_line1 {
                break;
            }
        }

        // Line 2: approximately y = 30..55 (second line of text, after wrapping)
        for y in 35..60 {
            for x in 10..70 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_line2 = true;
                    break;
                }
            }
            if found_line2 {
                break;
            }
        }

        assert!(found_line1, "First line of wrapped text should be rendered");
        assert!(
            found_line2,
            "Second line of wrapped text should be rendered"
        );
    }

    #[test]
    fn render_flow_text_with_span() {
        // Test a flowPara with a flowSpan child.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        if let Some(node) = doc.get_node_mut(flow_root_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
            node.presentation.font_size = Some(SvgLength::px(16.0));
        }
        doc.append_child(svg_id, flow_root_id);

        let flow_region_id = doc.add_node(SvgElement::FlowRegion(FlowRegionAttrs::default()));
        doc.append_child(flow_root_id, flow_region_id);

        let region_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(180.0),
            height: SvgLength::px(180.0),
            ..RectAttrs::default()
        }));
        doc.append_child(flow_region_id, region_rect_id);

        // flowPara with a flowSpan child (bold text).
        let flow_para_id = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: String::new(),
        }));
        doc.append_child(flow_root_id, flow_para_id);

        let flow_span_id = doc.add_node(SvgElement::FlowSpan(FlowSpanAttrs {
            text_content: "Bold text".to_string(),
        }));
        if let Some(node) = doc.get_node_mut(flow_span_id) {
            node.presentation.font_weight = Some(FontWeight::Bold);
        }
        doc.append_child(flow_para_id, flow_span_id);

        let pixels = render_document_to_pixmap(&doc, 200, 200);

        // Should have non-white pixels (text rendered).
        let mut found_non_white = false;
        for y in 10..60 {
            for x in 10..190 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_non_white = true;
                    break;
                }
            }
            if found_non_white {
                break;
            }
        }
        assert!(
            found_non_white,
            "Flow text with span should render non-white pixels"
        );
    }

    #[test]
    fn render_flow_text_multiple_paras() {
        // Test multiple flowPara children.
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        if let Some(node) = doc.get_node_mut(flow_root_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
            node.presentation.font_size = Some(SvgLength::px(14.0));
        }
        doc.append_child(svg_id, flow_root_id);

        let flow_region_id = doc.add_node(SvgElement::FlowRegion(FlowRegionAttrs::default()));
        doc.append_child(flow_root_id, flow_region_id);

        let region_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(180.0),
            height: SvgLength::px(180.0),
            ..RectAttrs::default()
        }));
        doc.append_child(flow_region_id, region_rect_id);

        // Two paragraphs.
        let para1 = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: "First paragraph".to_string(),
        }));
        doc.append_child(flow_root_id, para1);

        let para2 = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: "Second paragraph".to_string(),
        }));
        doc.append_child(flow_root_id, para2);

        let pixels = render_document_to_pixmap(&doc, 200, 200);

        // Both paragraphs should produce non-white pixels.
        // Para 1: roughly y in 10..30
        let mut found_para1 = false;
        for y in 10..35 {
            for x in 10..190 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_para1 = true;
                    break;
                }
            }
            if found_para1 {
                break;
            }
        }

        // Para 2: roughly y in 30..55
        let mut found_para2 = false;
        for y in 35..60 {
            for x in 10..190 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_para2 = true;
                    break;
                }
            }
            if found_para2 {
                break;
            }
        }

        assert!(found_para1, "First paragraph should be rendered");
        assert!(found_para2, "Second paragraph should be rendered");
    }

    #[test]
    fn render_flow_text_region_overflow_clipped() {
        // Text that exceeds the region height should be clipped (not rendered).
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let flow_root_id = doc.add_node(SvgElement::FlowRoot(FlowRootAttrs::default()));
        if let Some(node) = doc.get_node_mut(flow_root_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
            node.presentation.font_size = Some(SvgLength::px(14.0));
        }
        doc.append_child(svg_id, flow_root_id);

        // Very small region: only 25px tall, should only fit ~1 line.
        let flow_region_id = doc.add_node(SvgElement::FlowRegion(FlowRegionAttrs::default()));
        doc.append_child(flow_root_id, flow_region_id);

        let region_rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(40.0),
            height: SvgLength::px(25.0),
            ..RectAttrs::default()
        }));
        doc.append_child(flow_region_id, region_rect_id);

        let flow_para_id = doc.add_node(SvgElement::FlowPara(FlowParaAttrs {
            text_content: "One Two Three Four Five Six Seven Eight Nine Ten".to_string(),
        }));
        doc.append_child(flow_root_id, flow_para_id);

        // Should not panic; should render what fits.
        let pixels = render_document_to_pixmap(&doc, 200, 200);
        assert_eq!(pixels.len(), 200 * 200 * 4);

        // Below the region (y > 35), the area should be entirely white.
        let mut found_below_region = false;
        for y in 40..200 {
            for x in 10..50 {
                let idx = (y * 200 + x) * 4;
                if pixels[idx] != 255 || pixels[idx + 1] != 255 || pixels[idx + 2] != 255 {
                    found_below_region = true;
                    break;
                }
            }
            if found_below_region {
                break;
            }
        }
        assert!(
            !found_below_region,
            "Text below the region should not be rendered"
        );
    }

    #[test]
    fn flow_text_is_renderable() {
        assert!(SvgElement::FlowRoot(FlowRootAttrs::default()).is_renderable());
        assert!(SvgElement::FlowRoot(FlowRootAttrs::default()).is_flow_text());
        assert!(SvgElement::FlowRegion(FlowRegionAttrs::default()).is_flow_text());
        assert!(SvgElement::FlowPara(FlowParaAttrs::default()).is_flow_text());
        assert!(SvgElement::FlowSpan(FlowSpanAttrs::default()).is_flow_text());
    }

    #[test]
    fn split_into_words_helper() {
        let words = super::split_into_words("Hello  World\tFoo");
        assert_eq!(words, vec!["Hello", "World", "Foo"]);

        let empty = super::split_into_words("");
        assert!(empty.is_empty());

        let single = super::split_into_words("Word");
        assert_eq!(single, vec!["Word"]);
    }

    // -----------------------------------------------------------------------
    // Mesh gradient integration tests
    // -----------------------------------------------------------------------

    /// Helper: build a document with a mesh gradient and a rect using it.
    fn doc_with_mesh_gradient() -> SvgDocument {
        let mut doc = SvgDocument::new();

        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        // Defs
        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // MeshGradient at origin (0, 0)
        let mesh_id = doc.add_node(SvgElement::MeshGradient(MeshGradientAttrs {
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            gradient_units: None,
            gradient_transform: None,
            href: None,
        }));
        if let Some(n) = doc.get_node_mut(mesh_id) {
            n.xml_id = Some("meshGrad".to_string());
        }
        doc.append_child(defs_id, mesh_id);

        // MeshRow
        let row_id = doc.add_node(SvgElement::MeshRow(MeshRowAttrs {}));
        doc.append_child(mesh_id, row_id);

        // MeshPatch
        let patch_id = doc.add_node(SvgElement::MeshPatch(MeshPatchAttrs {
            stop_color: None,
            path: None,
        }));
        doc.append_child(row_id, patch_id);

        // 4 stops for a square patch 0,0 -> 100,0 -> 100,100 -> 0,100
        let stop0 = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::RED),
            stop_opacity: None,
            path: Some("L 100,0".to_string()),
        }));
        doc.append_child(patch_id, stop0);

        let stop1 = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::new(0, 255, 0, 255)),
            stop_opacity: None,
            path: Some("L 100,100".to_string()),
        }));
        doc.append_child(patch_id, stop1);

        let stop2 = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::BLUE),
            stop_opacity: None,
            path: Some("L 0,100".to_string()),
        }));
        doc.append_child(patch_id, stop2);

        let stop3 = doc.add_node(SvgElement::Stop(StopAttrs {
            offset: 0.0,
            stop_color: Some(Color::new(255, 255, 0, 255)),
            stop_opacity: None,
            path: Some("L 0,0".to_string()),
        }));
        doc.append_child(patch_id, stop3);

        // Rect that uses the mesh gradient
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(100.0),
            height: SvgLength::px(100.0),
            ..RectAttrs::default()
        }));
        if let Some(n) = doc.get_node_mut(rect_id) {
            n.presentation.fill = Some(Paint::Url("#meshGrad".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        doc
    }

    #[test]
    fn mesh_gradient_resolves_and_renders() {
        let doc = doc_with_mesh_gradient();
        let pixels = render_document_to_pixmap(&doc, 100, 100);

        assert_eq!(pixels.len(), 100 * 100 * 4);

        // The top-left corner should be reddish (from the Red corner).
        let tl = &pixels[0..4];
        assert!(
            tl[0] > 150,
            "top-left red channel should be high (was {})",
            tl[0]
        );

        // The top-right corner should be greenish.
        let tr_idx = (99) * 4;
        let tr = &pixels[tr_idx..tr_idx + 4];
        assert!(
            tr[1] > 150,
            "top-right green channel should be high (was {})",
            tr[1]
        );

        // The bottom-right corner should be bluish.
        let br_idx = (99 * 100 + 99) * 4;
        let br = &pixels[br_idx..br_idx + 4];
        assert!(
            br[2] > 150,
            "bottom-right blue channel should be high (was {})",
            br[2]
        );

        // The center should be a blend of all 4 colors.
        let center_idx = (50 * 100 + 50) * 4;
        let center = &pixels[center_idx..center_idx + 4];
        // Not pure white or pure black -- it should be some blended color.
        let total: u16 = center[0] as u16 + center[1] as u16 + center[2] as u16;
        assert!(
            total > 50 && total < 600,
            "center should be a color blend, got ({}, {}, {}, {})",
            center[0],
            center[1],
            center[2],
            center[3]
        );
    }

    #[test]
    fn mesh_gradient_paint_server_resolves() {
        let doc = doc_with_mesh_gradient();
        let paint = resolve_paint_server(&doc, "#meshGrad");
        assert!(
            paint.is_some(),
            "mesh gradient should resolve to a RenderPaint"
        );
        if let Some(RenderPaint::MeshGradient { width, height, .. }) = &paint {
            assert!(*width > 0, "mesh width should be > 0");
            assert!(*height > 0, "mesh height should be > 0");
        } else {
            panic!("expected RenderPaint::MeshGradient, got {:?}", paint);
        }
    }

    #[test]
    fn mesh_gradient_empty_rows_returns_none() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        // Empty mesh gradient (no rows)
        let mesh_id = doc.add_node(SvgElement::MeshGradient(MeshGradientAttrs::default()));
        if let Some(n) = doc.get_node_mut(mesh_id) {
            n.xml_id = Some("emptyMesh".to_string());
        }
        doc.append_child(defs_id, mesh_id);

        let paint = resolve_paint_server(&doc, "#emptyMesh");
        assert!(
            paint.is_none(),
            "empty mesh gradient should resolve to None"
        );
    }

    #[test]
    fn mesh_gradient_with_cubic_edges() {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(200.0)),
            height: Some(SvgLength::px(200.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.append_child(svg_id, defs_id);

        let mesh_id = doc.add_node(SvgElement::MeshGradient(MeshGradientAttrs {
            x: Some(SvgLength::px(10.0)),
            y: Some(SvgLength::px(10.0)),
            ..Default::default()
        }));
        if let Some(n) = doc.get_node_mut(mesh_id) {
            n.xml_id = Some("cubicMesh".to_string());
        }
        doc.append_child(defs_id, mesh_id);

        let row_id = doc.add_node(SvgElement::MeshRow(MeshRowAttrs {}));
        doc.append_child(mesh_id, row_id);

        let patch_id = doc.add_node(SvgElement::MeshPatch(MeshPatchAttrs::default()));
        doc.append_child(row_id, patch_id);

        // Cubic edges
        let stops = [
            ("C 60,0 110,0 160,10", Color::RED),
            ("C 170,60 170,110 160,160", Color::new(0, 255, 0, 255)),
            ("C 110,170 60,170 10,160", Color::BLUE),
            ("C 0,110 0,60 10,10", Color::new(255, 255, 0, 255)),
        ];
        for (path, color) in &stops {
            let stop_id = doc.add_node(SvgElement::Stop(StopAttrs {
                offset: 0.0,
                stop_color: Some(*color),
                stop_opacity: None,
                path: Some(path.to_string()),
            }));
            doc.append_child(patch_id, stop_id);
        }

        // Should resolve successfully
        let paint = resolve_paint_server(&doc, "#cubicMesh");
        assert!(paint.is_some(), "cubic mesh gradient should resolve");

        // Render it
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(0.0),
            y: SvgLength::px(0.0),
            width: SvgLength::px(200.0),
            height: SvgLength::px(200.0),
            ..RectAttrs::default()
        }));
        if let Some(n) = doc.get_node_mut(rect_id) {
            n.presentation.fill = Some(Paint::Url("#cubicMesh".to_string()));
        }
        doc.append_child(svg_id, rect_id);

        let pixels = render_document_to_pixmap(&doc, 200, 200);
        assert_eq!(pixels.len(), 200 * 200 * 4);

        // At least some non-white pixels should exist
        let has_color = pixels
            .chunks(4)
            .any(|px| px[0] != 255 || px[1] != 255 || px[2] != 255);
        assert!(has_color, "mesh gradient should produce colored pixels");
    }
}
