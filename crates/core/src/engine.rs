//! Editor engine facade.
//!
//! [`EditorEngine`] ties together the document, command history, selection,
//! tools, viewport, snapping, and event bus into one coherent API.

use std::collections::{HashMap, HashSet};

use sivgo_dom::{
    BlendMode as SvgBlendMode, ChannelSelector as SvgChannelSelector, Color,
    ColorMatrixType as SvgColorMatrixType, CompositeOperator as SvgCompositeOperator, DefsAttrs,
    DescAttrs, FeBlendAttrs, FeColorMatrixAttrs, FeCompositeAttrs, FeConvolveMatrixAttrs,
    FeDisplacementMapAttrs, FeFloodAttrs, FeGaussianBlurAttrs, FeMergeAttrs, FeMergeNodeAttrs,
    FeMorphologyAttrs, FeOffsetAttrs, FeTurbulenceAttrs, FilterAttrs, FilterInput,
    FilterPrimitiveAttrs, GroupAttrs, InkscapePageAttrs, LineCap as SvgLineCap,
    LineJoin as SvgLineJoin, MetadataAttrs, MorphologyOperator as SvgMorphologyOperator, NodeId,
    Paint, PathAttrs, PresentationAttrs, SodipodiGuideAttrs, SvgDocument, SvgElement, SvgNode,
    SvgSvgAttrs, TextAnchor as SvgTextAnchor, TitleAttrs, TurbulenceType as SvgTurbulenceType,
    UseAttrs,
};
use sivgo_math::{BezPath, Point, Rect, Transform2D, Vector};
use sivgo_path_ops::{
    boolean_op, break_apart, combine_paths, cut, element_to_path, offset_path, reverse_path,
    simplify_path, stroke_to_path, BooleanOp, DynamicOffset, LineCap as PathLineCap,
    LineJoin as PathLineJoin, LinkedOffset, StrokeParams,
};
use sivgo_text::{FontStyleKind, TextAnchor as TextEngineAnchor, TextEngine};

use crate::selection::element_bounding_box;

use crate::command::Command;
use crate::command::CommandHistory;
use crate::commands::{
    AddGuideCommand, AddNodeCommand, AddPageCommand, ClipMaskMode, CompoundCommand, GroupCommand,
    MergeLayerCommand, ModifyAttributeCommand, ModifyElementCommand, ModifyObjectLabelCommand,
    ModifyPageCommand, ModifyPathCommand, ModifyXmlIdCommand, ReleaseClipMaskCommand,
    RemoveNodeCommand, ReorderCommand, ReorderDirection, ReparentNodeCommand,
    RouteToActiveLayerCommand, SetClipMaskCommand, SetLockCommand, SetVisibilityCommand,
    UngroupCommand, UnlinkCloneCommand,
};
use crate::event::{EditorEvent, EventBus};
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::lpe::{
    BSplineLpe, BendPathLpe, CopyRotateLpe, DashedStrokeLpe, EnvelopeLpe, ExtrudeLpe,
    FilletChamferLpe, FilletMode, GearsLpe, HatchingLpe, InterpolateLpe, KnotLpe, LatticeLpe,
    LivePathEffect, MeasureSegmentsLpe, MirrorSymmetryLpe, OffsetJoin, OffsetLpe,
    PatternAlongPathLpe, PerspectiveEnvelopeLpe, PowerStrokeLpe, RoughenLpe, SimplifyLpe,
    SketchLpe, SliceLpe, SpiroLpe, TaperStrokeLpe, TilingLpe, VonKochLpe,
};
use crate::selection::SelectionSet;
use crate::snap::{SnapContext, SnapGuide, SnapIndicator, SnapSourceKind, SnappingEngine};
use crate::tool::{ToolId, ToolManager, ToolResponse};
use crate::tools::{
    Box3dTool, BucketFillTool, CalligraphyTool, ConnectorTool, DropperTool, EllipseTool,
    EraserTool, GradientTool, LpeTool, MarkerTool, MeasureTool, MeshTool, NodeTool, PagesTool,
    PenTool, PencilTool, RectTool, SelectTool, SpiralTool, SprayTool, StarTool, TextTool,
    TweakTool, ZoomTool,
};
use crate::viewport::{DisplayMode, Viewport};

use sivgo_dom::types::{LengthUnit, SvgLength};

/// A single entry on the internal clipboard, holding the serialized SVG XML
/// fragment of a copied element.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    /// The serialized SVG XML of the element (wrapped in an `<svg>` for
    /// parse-ability).
    pub svg_xml: String,
}

#[derive(Debug, Clone)]
struct PathSelectionItem {
    id: NodeId,
    element: SvgElement,
    presentation: PresentationAttrs,
    baked_presentation: PresentationAttrs,
    path: BezPath,
}

/// Decomposed affine transform values for inspector display and absolute edits.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TransformDecomposition {
    pub translate_x: f64,
    pub translate_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_degrees: f64,
    pub skew_x_degrees: f64,
}

/// Editable numeric parameter persisted for a selected-path LPE entry.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineLpeParam {
    pub name: String,
    pub value: f64,
}

/// One non-destructive Live Path Effect entry on a selected path.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineLpeEntry {
    pub id: String,
    pub kind: String,
    pub enabled: bool,
    pub params: Vec<EngineLpeParam>,
}

/// Editable string parameter persisted for an SVG filter primitive.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineFilterParam {
    pub name: String,
    pub value: String,
}

/// One primitive in the selected object's SVG filter chain.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineFilterPrimitiveEntry {
    pub id: String,
    pub kind: String,
    pub enabled: bool,
    pub params: Vec<EngineFilterParam>,
}

/// One reusable document resource discovered in the SVG tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineResourceEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub usage_count: usize,
}

/// Editable state for the primary selected text-like node.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineTextState {
    pub content: String,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: String,
    pub font_style: String,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub word_spacing: f64,
    pub text_anchor: String,
}

#[derive(Debug, Clone)]
struct DocumentSnapshotCommand {
    description: String,
    before: SvgDocument,
    after: SvgDocument,
}

impl DocumentSnapshotCommand {
    fn new(description: impl Into<String>, before: SvgDocument, after: SvgDocument) -> Self {
        Self {
            description: description.into(),
            before,
            after,
        }
    }
}

impl Command for DocumentSnapshotCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        *doc = self.after.clone();
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        *doc = self.before.clone();
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// The top-level editor engine.
pub struct EditorEngine {
    pub document: SvgDocument,
    pub history: CommandHistory,
    pub selection: SelectionSet,
    pub tools: ToolManager,
    pub viewport: Viewport,
    pub snapping: SnappingEngine,
    /// Last snap feedback generated by tool mouse input.
    pub last_snap_indicator: Option<SnapIndicator>,
    pub events: EventBus,
    /// Internal clipboard holding serialized copies of elements.
    pub clipboard: Vec<ClipboardEntry>,
    /// The currently active layer (new objects are added here).
    /// When `None`, objects are added to the root `<svg>` element.
    pub active_layer: Option<NodeId>,
}

impl EditorEngine {
    /// Create a new engine with an empty document and default tools.
    pub fn new(width: f64, height: f64) -> Self {
        let mut tools = ToolManager::new();
        tools.register(ToolId::Select, Box::new(SelectTool::new()));
        tools.register(ToolId::Node, Box::new(NodeTool::new()));
        tools.register(ToolId::Rect, Box::new(RectTool::new()));
        tools.register(ToolId::Ellipse, Box::new(EllipseTool::new()));
        tools.register(ToolId::Star, Box::new(StarTool::new()));
        tools.register(ToolId::Spiral, Box::new(SpiralTool::new()));
        tools.register(ToolId::Pen, Box::new(PenTool::new()));
        tools.register(ToolId::Pencil, Box::new(PencilTool::new()));
        tools.register(ToolId::Calligraphy, Box::new(CalligraphyTool::new()));
        tools.register(ToolId::Text, Box::new(TextTool::new()));
        tools.register(ToolId::Gradient, Box::new(GradientTool::new()));
        tools.register(ToolId::Dropper, Box::new(DropperTool::new()));
        tools.register(ToolId::BucketFill, Box::new(BucketFillTool::new()));
        tools.register(ToolId::Connector, Box::new(ConnectorTool::new()));
        tools.register(ToolId::Eraser, Box::new(EraserTool::new()));
        tools.register(ToolId::Spray, Box::new(SprayTool::new()));
        tools.register(ToolId::Tweak, Box::new(TweakTool::new()));
        tools.register(ToolId::Measure, Box::new(MeasureTool::new()));
        tools.register(ToolId::Zoom, Box::new(ZoomTool::new()));
        tools.register(ToolId::Mesh, Box::new(MeshTool::new()));
        tools.register(ToolId::Box3d, Box::new(Box3dTool::new()));
        tools.register(ToolId::Marker, Box::new(MarkerTool::new()));
        tools.register(ToolId::Pages, Box::new(PagesTool::new()));
        tools.register(ToolId::Lpe, Box::new(LpeTool::new()));

        let mut engine = Self {
            document: SvgDocument::new(),
            history: CommandHistory::new(200),
            selection: SelectionSet::new(),
            tools,
            viewport: Viewport::new(width, height),
            snapping: SnappingEngine::new(),
            last_snap_indicator: None,
            events: EventBus::new(),
            clipboard: Vec::new(),
            active_layer: None,
        };

        engine.new_document(width, height);
        engine
    }

    /// Load an existing document, replacing the current one.
    pub fn load_document(&mut self, doc: SvgDocument) {
        self.document = doc;
        self.history.clear();
        self.selection.clear();
        self.active_layer = None;
        self.last_snap_indicator = None;
        self.events.emit(EditorEvent::DocumentModified);
        self.events.emit(EditorEvent::SelectionChanged);
        self.events.emit(EditorEvent::UndoRedoChanged);
    }

    /// Create a new empty document with the given dimensions.
    pub fn new_document(&mut self, width: f64, height: f64) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::new(width, LengthUnit::None)),
            height: Some(SvgLength::new(height, LengthUnit::None)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);
        self.load_document(doc);
    }

    // -----------------------------------------------------------------------
    // Command execution
    // -----------------------------------------------------------------------

    /// Execute a command and record it in the history.
    pub fn execute(&mut self, cmd: Box<dyn Command>) {
        self.history.execute(cmd, &mut self.document);
        self.events.emit(EditorEvent::DocumentModified);
        self.events.emit(EditorEvent::UndoRedoChanged);
    }

    /// Undo the last command.
    pub fn undo(&mut self) {
        if self.history.undo(&mut self.document) {
            self.validate_active_layer();
            self.remove_missing_selection();
            self.events.emit(EditorEvent::DocumentModified);
            self.events.emit(EditorEvent::UndoRedoChanged);
        }
    }

    /// Redo the last undone command.
    pub fn redo(&mut self) {
        if self.history.redo(&mut self.document) {
            self.validate_active_layer();
            self.remove_missing_selection();
            self.select_created_nodes();
            self.events.emit(EditorEvent::DocumentModified);
            self.events.emit(EditorEvent::UndoRedoChanged);
        }
    }

    fn validate_active_layer(&mut self) {
        if self
            .active_layer
            .map(|id| !self.is_layer(id))
            .unwrap_or(false)
        {
            self.active_layer = None;
        }
    }

    fn remove_missing_selection(&mut self) {
        let missing: Vec<_> = self
            .selection
            .selected()
            .iter()
            .copied()
            .filter(|id| self.document.get_node(*id).is_none())
            .collect();
        if !missing.is_empty() {
            for id in missing {
                self.selection.remove(id);
            }
            self.events.emit(EditorEvent::SelectionChanged);
        }
    }

    fn select_created_nodes(&mut self) {
        let created: Vec<_> = self
            .history
            .created_selection()
            .into_iter()
            .filter(|id| self.is_selectable_object(*id))
            .collect();
        if !created.is_empty() {
            self.selection.clear();
            for id in created {
                self.selection.add(id);
            }
            self.events.emit(EditorEvent::SelectionChanged);
        }
    }

    // -----------------------------------------------------------------------
    // Input dispatch
    // -----------------------------------------------------------------------

    /// Cancel the active tool's unfinished gesture without changing tools,
    /// selection, settings, or already committed document edits.
    pub fn cancel_tool_interaction(&mut self) {
        let tool = self.tools.active_tool_mut();
        tool.on_deactivate();
        tool.on_activate();
        self.last_snap_indicator = None;
    }

    /// Dispatch a mouse-down event to the active tool.
    pub fn on_mouse_down(&mut self, event: MouseEvent) {
        let event = self.snapped_tool_mouse_event(event);
        let response = self.tools.active_tool_mut().on_mouse_down(
            event,
            &self.document,
            &self.selection,
            &self.viewport,
        );
        self.handle_tool_response(response);

        // If the tool is Select and it was a click, update selection.
        if self.tools.active_id() == ToolId::Select && event.button == MouseButton::Left {
            let doc_pt = self
                .viewport
                .screen_to_document(Point::new(event.x, event.y));
            if let Some(hit_id) = crate::tools::select::hit_test_at(&self.document, doc_pt) {
                if event.modifiers.shift {
                    self.selection.toggle(hit_id);
                } else if event.modifiers.ctrl {
                    self.selection.add(hit_id);
                } else if !self.selection.is_selected(hit_id) {
                    self.selection.select(hit_id);
                }
            } else if !event.modifiers.shift && !event.modifiers.ctrl {
                self.selection.clear();
            }
            self.events.emit(EditorEvent::SelectionChanged);
        }

        // The LPE tool selects the clicked path so that LPE operations
        // (panel buttons or the tool's own `apply_lpe`) target it.
        if self.tools.active_id() == ToolId::Lpe && event.button == MouseButton::Left {
            let doc_pt = self
                .viewport
                .screen_to_document(Point::new(event.x, event.y));
            match crate::tools::lpe_tool::hit_test_path_at(&self.document, doc_pt) {
                Some(hit_id) => self.selection.select(hit_id),
                None => self.selection.clear(),
            }
            self.events.emit(EditorEvent::SelectionChanged);
        }
    }

    /// Dispatch a mouse-move event to the active tool.
    pub fn on_mouse_move(&mut self, event: MouseEvent) {
        self.last_snap_indicator = None;
        let response = self.tools.active_tool_mut().on_mouse_move(
            event,
            &self.document,
            &self.selection,
            &self.viewport,
        );
        self.handle_tool_response(response);
    }

    /// Dispatch a mouse-up event to the active tool.
    pub fn on_mouse_up(&mut self, event: MouseEvent) {
        let event = self.snapped_tool_mouse_event(event);
        let response = self.tools.active_tool_mut().on_mouse_up(
            event,
            &self.document,
            &self.selection,
            &self.viewport,
        );
        self.handle_tool_response(response);
    }

    /// Dispatch a key-down event to the active tool.
    pub fn on_key_down(&mut self, event: KeyEvent) {
        let response = self.tools.active_tool_mut().on_key_down(
            event,
            &self.document,
            &self.selection,
            &self.viewport,
        );
        self.handle_tool_response(response);
    }

    /// Dispatch a key-up event to the active tool.
    pub fn on_key_up(&mut self, event: KeyEvent) {
        let response = self.tools.active_tool_mut().on_key_up(
            event,
            &self.document,
            &self.selection,
            &self.viewport,
        );
        self.handle_tool_response(response);
    }

    fn handle_tool_response(&mut self, response: ToolResponse) {
        match response {
            ToolResponse::ExecuteCommand(cmd) => {
                let command: Box<dyn Command> = if let Some(layer_id) = self.active_layer {
                    if self.is_layer(layer_id) {
                        Box::new(RouteToActiveLayerCommand::new(cmd, layer_id))
                    } else {
                        cmd
                    }
                } else {
                    cmd
                };
                self.execute(command);
                // Spray reads its source objects from the current selection
                // on every move, so keep those sources throughout the gesture.
                if self.tools.active_id() != ToolId::Spray {
                    self.select_created_nodes();
                }
            }
            ToolResponse::Handled | ToolResponse::None => {}
        }
    }

    fn snapped_tool_mouse_event(&mut self, event: MouseEvent) -> MouseEvent {
        self.last_snap_indicator = None;
        if !self.should_snap_active_tool()
            || !self.snapping.prefs.source_enabled(SnapSourceKind::Cursor)
            || event.modifiers.alt
        {
            return event;
        }

        let doc_point = self
            .viewport
            .screen_to_document(Point::new(event.x, event.y));
        let context = self.snap_context();
        let snapped = self.snapping.snap_all(doc_point, &context, &self.viewport);
        if snapped.target.is_none() {
            return event;
        }
        self.last_snap_indicator = self.snapping.build_indicator(&snapped);
        let screen_point = self.viewport.document_to_screen(snapped.point);
        MouseEvent {
            x: screen_point.x,
            y: screen_point.y,
            ..event
        }
    }

    fn should_snap_active_tool(&self) -> bool {
        matches!(
            self.tools.active_id(),
            ToolId::Rect
                | ToolId::Ellipse
                | ToolId::Star
                | ToolId::Spiral
                | ToolId::Pen
                | ToolId::Pencil
                | ToolId::Calligraphy
                | ToolId::Text
                | ToolId::Connector
                | ToolId::Box3d
                | ToolId::Marker
                | ToolId::Mesh
        )
    }

    fn snap_context(&self) -> SnapContext {
        let mut context = SnapContext::default();
        let Some(root) = self.document.root() else {
            return context;
        };

        for id in self.document.traverse_pre_order(root) {
            if id == root {
                continue;
            }
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            if node.hidden {
                continue;
            }

            if let SvgElement::SodipodiGuide(attrs) = &node.element {
                if let Some(guide) = Self::guide_line_from_attrs(attrs) {
                    context.guides.push(guide);
                }
                continue;
            }

            if let Some(bbox) = element_bounding_box(&node.element) {
                context.bboxes.push(bbox);
                let center = bbox.center();
                context.neighbor_centers_x.push(center.x);
                context.neighbor_centers_y.push(center.y);
            }

            if let Some(path) = element_to_path(&node.element) {
                if !path.is_empty() {
                    context.paths.push(path);
                }
            }
        }

        context
    }

    /// Return all document guide lines in snap-ready geometry.
    pub fn guide_lines(&self) -> Vec<SnapGuide> {
        let Some(root) = self.document.root() else {
            return Vec::new();
        };
        self.document
            .traverse_pre_order(root)
            .into_iter()
            .filter_map(|id| {
                let node = self.document.get_node(id)?;
                if let SvgElement::SodipodiGuide(attrs) = &node.element {
                    Self::guide_line_from_attrs(attrs)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Add a horizontal visual guide at document y.
    pub fn add_horizontal_guide(&mut self, y: f64) -> Option<NodeId> {
        if !y.is_finite() {
            return None;
        }
        let cmd = AddGuideCommand::new(
            &self.document,
            (0.0, y),
            (0.0, 1.0),
            Some(format!("H {:.1}", y)),
        )?;
        let guide_id = cmd.guide_id();
        self.execute(Box::new(cmd));
        Some(guide_id)
    }

    /// Add a vertical visual guide at document x.
    pub fn add_vertical_guide(&mut self, x: f64) -> Option<NodeId> {
        if !x.is_finite() {
            return None;
        }
        let cmd = AddGuideCommand::new(
            &self.document,
            (x, 0.0),
            (1.0, 0.0),
            Some(format!("V {:.1}", x)),
        )?;
        let guide_id = cmd.guide_id();
        self.execute(Box::new(cmd));
        Some(guide_id)
    }

    fn guide_line_from_attrs(attrs: &SodipodiGuideAttrs) -> Option<SnapGuide> {
        let normal = Self::guide_normal(attrs);
        let origin = attrs
            .position
            .as_deref()
            .and_then(|value| {
                Self::parse_number_pair(value).map(Point::from).or_else(|| {
                    Self::parse_number(value).map(|position| {
                        if matches!(attrs.orientation.as_deref(), Some("horizontal")) {
                            Point::new(0.0, position)
                        } else {
                            Point::new(position, 0.0)
                        }
                    })
                })
            })
            .unwrap_or(Point::ZERO);
        SnapGuide::new(origin, normal)
    }

    fn guide_normal(attrs: &SodipodiGuideAttrs) -> Vector {
        match attrs.orientation.as_deref() {
            Some("horizontal") => Vector::new(0.0, 1.0),
            Some("vertical") => Vector::new(1.0, 0.0),
            Some(value) => Self::parse_number_pair(value)
                .map(Vector::from)
                .unwrap_or_else(|| Vector::new(1.0, 0.0)),
            None => Vector::new(1.0, 0.0),
        }
    }

    fn parse_number_pair(value: &str) -> Option<(f64, f64)> {
        let mut parts = value
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|part| !part.is_empty());
        let x = parts.next()?.parse::<f64>().ok()?;
        let y = parts.next()?.parse::<f64>().ok()?;
        Some((x, y))
    }

    fn parse_number(value: &str) -> Option<f64> {
        value.trim().parse::<f64>().ok()
    }

    // -----------------------------------------------------------------------
    // Viewport: scroll-wheel zoom & middle-mouse pan (9.5.3, 9.5.4)
    // -----------------------------------------------------------------------

    /// Handle a scroll-wheel event by zooming centered on the cursor position.
    ///
    /// `delta` is the scroll amount (positive = zoom in, negative = zoom out).
    /// `mouse_x` and `mouse_y` are the cursor position in screen pixels.
    pub fn on_scroll(&mut self, delta: f64, mouse_x: f64, mouse_y: f64) {
        // Convert scroll delta into a zoom factor.
        // A typical scroll notch (delta = 1.0) maps to a ~10% zoom change.
        let factor = 1.1_f64.powf(delta);
        let center = Point::new(mouse_x, mouse_y);
        self.viewport.zoom_at(center, factor);
        self.events.emit(EditorEvent::ViewportChanged);
    }

    /// Handle a middle-mouse drag by panning the viewport.
    ///
    /// `dx` and `dy` are the drag delta in screen pixels since the last event.
    pub fn on_middle_mouse_drag(&mut self, dx: f64, dy: f64) {
        self.viewport.pan_by(sivgo_math::Vector::new(dx, dy));
        self.events.emit(EditorEvent::ViewportChanged);
    }

    /// Set the viewport display mode explicitly.
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        if self.viewport.display_mode != mode {
            self.viewport.display_mode = mode;
            self.events.emit(EditorEvent::ViewportChanged);
        }
    }

    /// Cycle the viewport display mode to the next variant.
    pub fn cycle_display_mode(&mut self) {
        self.viewport.cycle_display_mode();
        self.events.emit(EditorEvent::ViewportChanged);
    }

    // -----------------------------------------------------------------------
    // High-level operations
    // -----------------------------------------------------------------------

    /// Delete all selected elements.
    pub fn delete_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        self.selection.clear();
        self.events.emit(EditorEvent::SelectionChanged);

        if ids.len() == 1 {
            self.execute(Box::new(RemoveNodeCommand::new(ids[0])));
        } else {
            let commands: Vec<Box<dyn Command>> = ids
                .into_iter()
                .map(|id| Box::new(RemoveNodeCommand::new(id)) as Box<dyn Command>)
                .collect();
            self.execute(Box::new(CompoundCommand::new("Delete elements", commands)));
        }
    }

    /// Select all objects in the active layer, falling back to all document
    /// objects when no active layer exists.
    pub fn select_all(&mut self) {
        self.select_all_current_layer();
    }

    /// Select all selectable objects in the active layer.
    pub fn select_all_current_layer(&mut self) {
        let scope = self
            .active_layer
            .filter(|id| self.is_layer(*id))
            .or_else(|| self.document.root());
        self.select_all_in_scope(scope);
    }

    /// Select all selectable objects across the entire document.
    pub fn select_all_in_all_layers(&mut self) {
        self.select_all_in_scope(self.document.root());
    }

    fn select_all_in_scope(&mut self, scope: Option<NodeId>) {
        self.selection.clear();
        if let Some(scope) = scope {
            for id in self.document.traverse_pre_order(scope) {
                if id == scope {
                    continue;
                }
                if self.is_selectable_object(id) {
                    self.selection.add(id);
                }
            }
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    fn is_selectable_object(&self, id: NodeId) -> bool {
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        if self.document.root() == Some(id) || self.is_layer(id) {
            return false;
        }
        if self.is_inside_resource_tree(id) {
            return false;
        }
        node.element.is_renderable()
    }

    fn is_inside_resource_tree(&self, id: NodeId) -> bool {
        let mut parent = self.document.get_node(id).and_then(|node| node.parent);
        while let Some(parent_id) = parent {
            let Some(parent_node) = self.document.get_node(parent_id) else {
                return false;
            };
            if matches!(
                &parent_node.element,
                SvgElement::Defs(_)
                    | SvgElement::ClipPath(_)
                    | SvgElement::Mask(_)
                    | SvgElement::Marker(_)
                    | SvgElement::Pattern(_)
                    | SvgElement::Filter(_)
                    | SvgElement::Symbol(_)
                    | SvgElement::LinearGradient(_)
                    | SvgElement::RadialGradient(_)
                    | SvgElement::MeshGradient(_)
            ) {
                return true;
            }
            parent = parent_node.parent;
        }
        false
    }

    /// Clear the current selection.
    pub fn select_none(&mut self) {
        if !self.selection.is_empty() {
            self.selection.clear();
            self.events.emit(EditorEvent::SelectionChanged);
        }
    }

    /// Replace the current selection with a specific node, if it exists.
    pub fn select_node(&mut self, id: NodeId) {
        if self.document.get_node(id).is_none() {
            return;
        }
        self.selection.select(id);
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Invert selection among selectable direct children of the root element.
    pub fn invert_selection(&mut self) {
        let Some(root) = self.document.root() else {
            return;
        };

        let selectables: Vec<NodeId> = self
            .document
            .children(root)
            .iter()
            .copied()
            .filter(|id| {
                self.document
                    .get_node(*id)
                    .map(|node| !matches!(node.element, SvgElement::Defs(_)))
                    .unwrap_or(false)
            })
            .collect();

        if selectables.is_empty() {
            return;
        }

        let previously_selected = self.selection.selected().to_vec();
        self.selection.clear();
        for id in selectables {
            if !previously_selected.contains(&id) {
                self.selection.add(id);
            }
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Group the currently selected elements.
    pub fn group_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.len() < 2 {
            return;
        }
        let cmd = GroupCommand::new(ids);
        self.execute(Box::new(cmd));
    }

    /// Ungroup the currently selected group (if the selection is a single
    /// group).
    pub fn ungroup_selected(&mut self) {
        if self.selection.len() != 1 {
            return;
        }
        let id = self.selection.primary().unwrap();
        if let Some(node) = self.document.get_node(id) {
            if !matches!(&node.element, SvgElement::Group(_)) {
                return;
            }
        } else {
            return;
        }
        let children: Vec<NodeId> = self.document.children(id).to_vec();
        self.selection.clear();
        let cmd = UngroupCommand::new(id);
        self.execute(Box::new(cmd));
        // Select the ungrouped children.
        for child in children {
            self.selection.add(child);
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Raise selected elements one step in z-order.
    pub fn raise_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| {
                Box::new(ReorderCommand::new(id, ReorderDirection::Raise)) as Box<dyn Command>
            })
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Raise", commands)));
        }
    }

    /// Lower selected elements one step in z-order.
    pub fn lower_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| {
                Box::new(ReorderCommand::new(id, ReorderDirection::Lower)) as Box<dyn Command>
            })
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Lower", commands)));
        }
    }

    /// Reparent a node if it is legal to do so.
    pub fn reparent_node(
        &mut self,
        node_id: NodeId,
        new_parent: NodeId,
        new_index: Option<usize>,
    ) -> bool {
        if !self.can_reparent_node(node_id, new_parent) {
            return false;
        }

        self.execute(Box::new(ReparentNodeCommand::new(
            node_id, new_parent, new_index,
        )));
        self.events.emit(EditorEvent::SelectionChanged);
        true
    }

    /// Reparent a node according to an Objects-panel drop target.
    ///
    /// Dropping onto a container appends the node as a child. Dropping onto a
    /// leaf object moves the node after that target in the target's parent.
    pub fn reparent_node_for_object_drop(&mut self, node_id: NodeId, target_id: NodeId) -> bool {
        if node_id == target_id {
            return false;
        }

        let Some(target) = self.document.get_node(target_id) else {
            return false;
        };

        if target.element.is_container() {
            self.reparent_node(node_id, target_id, None)
        } else {
            let Some(parent) = target.parent else {
                return false;
            };
            let index = self.document.child_index(target_id).map(|idx| idx + 1);
            self.reparent_node(node_id, parent, index)
        }
    }

    /// Move a node to the root level, preserving visual order by appending it.
    pub fn reparent_node_to_root(&mut self, node_id: NodeId) -> bool {
        let Some(root) = self.document.root() else {
            return false;
        };
        self.reparent_node(node_id, root, None)
    }

    /// Move selected non-layer objects into the active layer.
    pub fn move_selection_to_active_layer(&mut self) -> bool {
        let Some(layer_id) = self.active_layer.filter(|id| self.is_layer(*id)) else {
            return false;
        };

        let ids = self.selected_in_document_order();
        if ids.is_empty() {
            return false;
        }

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for id in ids {
            if id == layer_id || self.is_layer(id) || !self.is_selectable_object(id) {
                continue;
            }
            if self.document.parent(id) == Some(layer_id) {
                continue;
            }
            if !self.can_reparent_node(id, layer_id) {
                continue;
            }
            commands.push(Box::new(ReparentNodeCommand::new(id, layer_id, None)));
        }

        if commands.is_empty() {
            return false;
        }

        self.execute_command_batch("Move selection to active layer", commands);
        self.events.emit(EditorEvent::SelectionChanged);
        true
    }

    fn can_reparent_node(&self, node_id: NodeId, new_parent: NodeId) -> bool {
        if node_id == new_parent {
            return false;
        }
        let Some(root) = self.document.root() else {
            return false;
        };
        if node_id == root {
            return false;
        }
        let Some(parent_node) = self.document.get_node(new_parent) else {
            return false;
        };
        if !parent_node.element.is_container() {
            return false;
        }
        if self.document.get_node(node_id).is_none() {
            return false;
        }

        !self
            .document
            .traverse_pre_order(node_id)
            .contains(&new_parent)
    }

    /// Duplicate selected elements (add copies with a small offset).
    pub fn duplicate_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for &id in &ids {
            if let Some(node) = self.document.get_node(id) {
                let parent = match node.parent {
                    Some(p) => p,
                    None => continue,
                };
                let mut pres = node.presentation.clone();
                // Apply a small offset via transform.
                let offset = sivgo_math::Transform2D::translate(10.0, 10.0);
                let current = pres.transform.unwrap_or(sivgo_math::Transform2D::IDENTITY);
                pres.transform = Some(current.then(&offset));

                commands.push(Box::new(AddNodeCommand::new(
                    parent,
                    node.element.clone(),
                    pres,
                    None,
                )));
            }
        }

        if !commands.is_empty() {
            if commands.len() == 1 {
                self.execute(commands.into_iter().next().unwrap());
            } else {
                self.execute(Box::new(CompoundCommand::new("Duplicate", commands)));
            }
        }
    }

    /// Create SVG `<use>` clones for the selected elements.
    ///
    /// Sources without an SVG `id` receive one in the same undoable command,
    /// so the clone reference remains valid after save/reload.
    pub fn clone_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let mut used_xml_ids = self.used_xml_ids();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for &id in &ids {
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            let Some(parent) = node.parent else {
                continue;
            };

            let source_xml_id = match node.xml_id.clone() {
                Some(xml_id) => {
                    used_xml_ids.insert(xml_id.clone());
                    xml_id
                }
                None => {
                    let generated = self.generate_clone_source_xml_id(id, &mut used_xml_ids);
                    commands.push(Box::new(ModifyXmlIdCommand::new(
                        id,
                        None,
                        Some(generated.clone()),
                    )));
                    generated
                }
            };

            let element = SvgElement::Use(UseAttrs {
                href: Some(format!("#{source_xml_id}")),
                x: Some(SvgLength::zero()),
                y: Some(SvgLength::zero()),
                width: None,
                height: None,
            });

            let pres = PresentationAttrs {
                transform: Some(Transform2D::translate(10.0, 10.0)),
                ..Default::default()
            };
            let insert_index = self.document.child_index(id).map(|idx| idx + 1);

            commands.push(Box::new(AddNodeCommand::new(
                parent,
                element,
                pres,
                insert_index,
            )));
        }

        if !commands.is_empty() {
            self.execute(Box::new(CompoundCommand::new("Create clone", commands)));
        }
    }

    /// Replace selected SVG `<use>` clones with independent object subtrees.
    pub fn unlink_selected_clones(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for &id in &ids {
            let Some(clone_node) = self.document.get_node(id).cloned() else {
                continue;
            };
            let SvgElement::Use(use_attrs) = &clone_node.element else {
                continue;
            };
            let Some(href) = use_attrs.href.as_deref() else {
                continue;
            };
            let Some(source_xml_id) = Self::href_target_id(href) else {
                continue;
            };
            let Some(source_id) = self.document.find_by_xml_id(&source_xml_id) else {
                continue;
            };
            if source_id == id {
                continue;
            }

            let Some(old_subtree) = self.snapshot_subtree_nodes(id) else {
                continue;
            };
            let Some(new_subtree) =
                self.build_unlinked_clone_subtree(id, &clone_node, source_id, use_attrs)
            else {
                continue;
            };

            commands.push(Box::new(UnlinkCloneCommand::new(
                id,
                old_subtree,
                new_subtree,
            )));
        }

        if !commands.is_empty() {
            self.execute(Box::new(CompoundCommand::new("Unlink clone", commands)));
        }
    }

    fn used_xml_ids(&self) -> HashSet<String> {
        self.document
            .nodes()
            .filter_map(|(_, node)| node.xml_id.clone())
            .collect()
    }

    fn generate_clone_source_xml_id(
        &self,
        source_id: NodeId,
        used_xml_ids: &mut HashSet<String>,
    ) -> String {
        let tag = self
            .document
            .get_node(source_id)
            .map(|node| Self::xml_id_token(node.element.tag_name()))
            .unwrap_or_else(|| "object".to_string());
        let compact_id = source_id.to_string().replace('-', "");
        let suffix_len = compact_id.len().min(8);
        let base = format!("sivgo-{tag}-{}", &compact_id[..suffix_len]);

        let mut candidate = base.clone();
        let mut counter = 2;
        while used_xml_ids.contains(&candidate) {
            candidate = format!("{base}-{counter}");
            counter += 1;
        }
        used_xml_ids.insert(candidate.clone());
        candidate
    }

    fn xml_id_token(value: &str) -> String {
        let mut token = String::new();
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                token.push(ch.to_ascii_lowercase());
            } else {
                token.push('-');
            }
        }
        if token
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
            .unwrap_or(false)
        {
            token
        } else {
            format!("object-{token}")
        }
    }

    fn href_target_id(href: &str) -> Option<String> {
        let href = href.trim();
        if let Some(id) = href.strip_prefix('#') {
            return (!id.is_empty()).then(|| id.to_string());
        }

        let inner = href.strip_prefix("url(")?.strip_suffix(')')?.trim();
        let inner = inner.trim_matches('"').trim_matches('\'');
        inner
            .strip_prefix('#')
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string())
    }

    fn snapshot_subtree_nodes(&self, root: NodeId) -> Option<Vec<SvgNode>> {
        self.document.get_node(root)?;
        let mut nodes = Vec::new();
        for id in self.document.traverse_pre_order(root) {
            nodes.push(self.document.get_node(id)?.clone());
        }
        Some(nodes)
    }

    fn build_unlinked_clone_subtree(
        &self,
        clone_id: NodeId,
        clone_node: &SvgNode,
        source_id: NodeId,
        use_attrs: &UseAttrs,
    ) -> Option<Vec<SvgNode>> {
        let source_ids = self.document.traverse_pre_order(source_id);
        if source_ids.is_empty() {
            return None;
        }

        let mut id_map = HashMap::new();
        for &source_node_id in &source_ids {
            let new_id = if source_node_id == source_id {
                clone_id
            } else {
                NodeId::new()
            };
            id_map.insert(source_node_id, new_id);
        }

        let mut snapshots = Vec::new();
        for &source_node_id in &source_ids {
            let source_node = self.document.get_node(source_node_id)?;
            let mut node = source_node.clone();
            node.id = *id_map.get(&source_node_id)?;
            node.parent = if source_node_id == source_id {
                clone_node.parent
            } else {
                source_node
                    .parent
                    .and_then(|parent| id_map.get(&parent).copied())
            };
            node.children = source_node
                .children
                .iter()
                .filter_map(|child| id_map.get(child).copied())
                .collect();

            if source_node_id == source_id {
                node.xml_id = clone_node.xml_id.clone();
                node.presentation = Self::presentation_for_unlinked_clone(
                    &source_node.presentation,
                    &clone_node.presentation,
                    use_attrs,
                );
                node.attributes = source_node.attributes.clone();
                let mut clone_attrs = clone_node.attributes.clone();
                clone_attrs.remove("href");
                clone_attrs.remove("xlink:href");
                node.attributes.extend(clone_attrs);
                node.namespaces = source_node.namespaces.clone();
                node.namespaces.extend(clone_node.namespaces.clone());
                node.required_features = clone_node
                    .required_features
                    .clone()
                    .or_else(|| source_node.required_features.clone());
                node.required_extensions = clone_node
                    .required_extensions
                    .clone()
                    .or_else(|| source_node.required_extensions.clone());
                node.system_language = clone_node
                    .system_language
                    .clone()
                    .or_else(|| source_node.system_language.clone());
                node.locked = clone_node.locked;
                node.hidden = clone_node.hidden;
            } else {
                node.xml_id = None;
            }

            snapshots.push(node);
        }

        Some(snapshots)
    }

    fn presentation_for_unlinked_clone(
        source: &PresentationAttrs,
        clone: &PresentationAttrs,
        use_attrs: &UseAttrs,
    ) -> PresentationAttrs {
        let mut pres = source.clone();

        macro_rules! clone_override {
            ($field:ident) => {
                if clone.$field.is_some() {
                    pres.$field = clone.$field.clone();
                }
            };
        }

        clone_override!(color);
        clone_override!(fill);
        clone_override!(stroke);
        clone_override!(stroke_width);
        clone_override!(opacity);
        clone_override!(display);
        clone_override!(visibility);
        clone_override!(fill_opacity);
        clone_override!(stroke_opacity);
        clone_override!(stroke_linecap);
        clone_override!(stroke_linejoin);
        clone_override!(stroke_miterlimit);
        clone_override!(stroke_dasharray);
        clone_override!(stroke_dashoffset);
        clone_override!(fill_rule);
        clone_override!(clip_rule);
        clone_override!(clip_path);
        clone_override!(mask);
        clone_override!(filter);
        clone_override!(marker_start);
        clone_override!(marker_mid);
        clone_override!(marker_end);
        clone_override!(mix_blend_mode);
        clone_override!(isolation);
        clone_override!(font_family);
        clone_override!(font_size);
        clone_override!(font_weight);
        clone_override!(font_style);
        clone_override!(text_anchor);
        clone_override!(letter_spacing);
        clone_override!(word_spacing);
        clone_override!(text_decoration);
        clone_override!(vector_effect);

        let source_transform = source.transform.unwrap_or(Transform2D::IDENTITY);
        let x = use_attrs.x.map(|length| length.to_px()).unwrap_or(0.0);
        let y = use_attrs.y.map(|length| length.to_px()).unwrap_or(0.0);
        let use_position = Transform2D::translate(x, y);
        let clone_transform = clone.transform.unwrap_or(Transform2D::IDENTITY);
        let transform = source_transform.then(&use_position).then(&clone_transform);
        pres.transform = (!transform.is_identity()).then_some(transform);

        pres
    }

    /// Set selected objects' clip path using the topmost selected object.
    pub fn set_clip_selected(&mut self) {
        self.set_clip_mask_selected(ClipMaskMode::Clip);
    }

    /// Release selected objects' clip path references.
    pub fn release_clip_selected(&mut self) {
        self.release_clip_mask_selected(ClipMaskMode::Clip);
    }

    /// Set selected objects' mask using the topmost selected object.
    pub fn set_mask_selected(&mut self) {
        self.set_clip_mask_selected(ClipMaskMode::Mask);
    }

    /// Release selected objects' mask references.
    pub fn release_mask_selected(&mut self) {
        self.release_clip_mask_selected(ClipMaskMode::Mask);
    }

    fn set_clip_mask_selected(&mut self, mode: ClipMaskMode) {
        let ordered = self.selected_in_document_order();
        if ordered.len() < 2 {
            return;
        }

        let source_id = *ordered.last().expect("len checked");
        let target_ids: Vec<NodeId> = ordered.into_iter().filter(|id| *id != source_id).collect();
        if target_ids.is_empty() {
            return;
        }

        self.execute(Box::new(SetClipMaskCommand::new(
            mode,
            target_ids.clone(),
            source_id,
        )));
        self.selection.clear();
        for target_id in target_ids {
            self.selection.add(target_id);
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    fn release_clip_mask_selected(&mut self, mode: ClipMaskMode) {
        let target_ids: Vec<NodeId> = self.selection.selected().to_vec();
        if target_ids.is_empty() {
            return;
        }
        self.execute(Box::new(ReleaseClipMaskCommand::new(mode, target_ids)));
    }

    fn selected_in_document_order(&self) -> Vec<NodeId> {
        let selected = self.selection.selected().to_vec();
        if selected.is_empty() {
            return Vec::new();
        }

        let mut ordered = Vec::new();
        if let Some(root) = self.document.root() {
            for id in self.document.traverse_pre_order(root) {
                if selected.contains(&id) {
                    ordered.push(id);
                }
            }
        }

        for id in selected {
            if !ordered.contains(&id) {
                ordered.push(id);
            }
        }
        ordered
    }

    // -----------------------------------------------------------------------
    // Clipboard operations (9.6.3)
    // -----------------------------------------------------------------------

    /// Serialize a single node (and its subtree) into an SVG XML string by
    /// wrapping it in a temporary `<svg>` document.
    fn serialize_node_to_svg(&self, node_id: NodeId) -> Option<String> {
        let mut tmp_doc = SvgDocument::new();
        let svg_id = tmp_doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        tmp_doc.set_root(svg_id);

        self.deep_clone_into(node_id, svg_id, &mut tmp_doc)?;

        Some(sivgo_dom::serialize_svg(&tmp_doc))
    }

    /// Deep-clone a node and its descendants from `self.document` into
    /// `target_doc` under `target_parent`.  Returns the new node id.
    fn deep_clone_into(
        &self,
        source_id: NodeId,
        target_parent: NodeId,
        target_doc: &mut SvgDocument,
    ) -> Option<NodeId> {
        Self::deep_clone_between(&self.document, source_id, target_parent, target_doc)
    }

    fn deep_clone_between(
        source_doc: &SvgDocument,
        source_id: NodeId,
        target_parent: NodeId,
        target_doc: &mut SvgDocument,
    ) -> Option<NodeId> {
        let source = source_doc.get_node(source_id)?;
        let new_id = target_doc.add_node(source.element.clone());
        if let Some(new_node) = target_doc.get_node_mut(new_id) {
            new_node.presentation = source.presentation.clone();
            new_node.xml_id = source.xml_id.clone();
            new_node.attributes = source.attributes.clone();
            new_node.namespaces = source.namespaces.clone();
            new_node.required_features = source.required_features.clone();
            new_node.required_extensions = source.required_extensions.clone();
            new_node.system_language = source.system_language.clone();
        }
        target_doc.append_child(target_parent, new_id);

        let children: Vec<NodeId> = source_doc.children(source_id).to_vec();
        for child_id in children {
            Self::deep_clone_between(source_doc, child_id, new_id, target_doc);
        }

        Some(new_id)
    }

    fn insertion_parent(&self) -> Option<NodeId> {
        if let Some(layer_id) = self.active_layer {
            if self.is_layer(layer_id) {
                return Some(layer_id);
            }
        }
        self.document.root()
    }

    /// Copy the selected elements' SVG XML into the clipboard without
    /// modifying the document.
    pub fn copy(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        self.clipboard.clear();
        for &id in &ids {
            if let Some(xml) = self.serialize_node_to_svg(id) {
                self.clipboard.push(ClipboardEntry { svg_xml: xml });
            }
        }
    }

    /// Return plain text for a text-only selection.
    pub fn selected_plain_text(&self) -> Option<String> {
        let selected: HashSet<NodeId> = self.selection.selected().iter().copied().collect();
        if selected.is_empty() {
            return None;
        }

        let mut roots = Vec::new();
        for id in self.selected_in_document_order() {
            let mut parent = self.document.get_node(id).and_then(|node| node.parent);
            let mut has_selected_ancestor = false;
            while let Some(parent_id) = parent {
                if selected.contains(&parent_id) {
                    has_selected_ancestor = true;
                    break;
                }
                parent = self
                    .document
                    .get_node(parent_id)
                    .and_then(|node| node.parent);
            }
            if !has_selected_ancestor {
                roots.push(id);
            }
        }

        let mut chunks = Vec::new();
        for id in roots {
            let mut text = String::new();
            self.collect_plain_text(id, &mut text);
            if text.trim().is_empty() {
                return None;
            }
            chunks.push(text);
        }

        (!chunks.is_empty()).then(|| chunks.join("\n"))
    }

    fn collect_plain_text(&self, id: NodeId, out: &mut String) {
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        match &node.element {
            SvgElement::Text(attrs) => out.push_str(&attrs.text_content),
            SvgElement::TSpan(attrs) => out.push_str(&attrs.text_content),
            SvgElement::TextPath(attrs) => out.push_str(&attrs.text_content),
            SvgElement::FlowPara(attrs) => out.push_str(&attrs.text_content),
            SvgElement::FlowSpan(attrs) => out.push_str(&attrs.text_content),
            _ => {}
        }

        for &child in self.document.children(id) {
            self.collect_plain_text(child, out);
        }
    }

    /// Return the internal clipboard as one SVG document suitable for a native
    /// text clipboard.
    pub fn clipboard_svg_text(&self) -> Option<String> {
        if self.clipboard.is_empty() {
            return None;
        }

        if self.clipboard.len() == 1 {
            return Some(self.clipboard[0].svg_xml.clone());
        }

        let mut tmp_doc = SvgDocument::new();
        let svg_id = tmp_doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        tmp_doc.set_root(svg_id);

        for entry in &self.clipboard {
            let parsed_doc = sivgo_dom::parse_svg(&entry.svg_xml).ok()?;
            let parsed_root = parsed_doc.root()?;
            let children: Vec<NodeId> = parsed_doc.children(parsed_root).to_vec();
            for child_id in children {
                Self::deep_clone_between(&parsed_doc, child_id, svg_id, &mut tmp_doc)?;
            }
        }

        Some(sivgo_dom::serialize_svg(&tmp_doc))
    }

    /// Replace the internal clipboard with one SVG document read from an
    /// external clipboard.
    pub fn set_clipboard_svg_text(&mut self, svg_xml: impl Into<String>) {
        self.clipboard.clear();
        self.clipboard.push(ClipboardEntry {
            svg_xml: svg_xml.into(),
        });
    }

    /// Copy the selected elements to the clipboard, then delete them.
    pub fn cut(&mut self) {
        self.copy();
        if !self.clipboard.is_empty() {
            self.delete_selected();
        }
    }

    /// Deserialize clipboard entries and add them to the document with a
    /// small positional offset (+10, +10).
    pub fn paste(&mut self) {
        self.paste_internal(true);
    }

    /// Deserialize clipboard entries and add them at their original position.
    pub fn paste_in_place(&mut self) {
        self.paste_internal(false);
    }

    /// Shared implementation for paste / paste-in-place.
    fn paste_internal(&mut self, apply_offset: bool) {
        if self.clipboard.is_empty() {
            return;
        }

        let parent = match self.insertion_parent() {
            Some(parent) => parent,
            None => return,
        };

        let entries = self.clipboard.clone();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for entry in &entries {
            let parsed_doc = match sivgo_dom::parse_svg(&entry.svg_xml) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let parsed_root = match parsed_doc.root() {
                Some(r) => r,
                None => continue,
            };

            // The children of the parsed `<svg>` wrapper are the actual elements.
            let children: Vec<NodeId> = parsed_doc.children(parsed_root).to_vec();
            for child_id in children {
                if let Some(child_node) = parsed_doc.get_node(child_id) {
                    let mut pres = child_node.presentation.clone();

                    if apply_offset {
                        let offset = Transform2D::translate(10.0, 10.0);
                        let current = pres.transform.unwrap_or(Transform2D::IDENTITY);
                        pres.transform = Some(current.then(&offset));
                    }

                    let child_children: Vec<NodeId> = parsed_doc.children(child_id).to_vec();
                    if child_children.is_empty() {
                        // Leaf node -- use AddNodeCommand directly.
                        commands.push(Box::new(AddNodeCommand::new(
                            parent,
                            child_node.element.clone(),
                            pres,
                            None,
                        )));
                    } else {
                        // Subtree -- use PasteSubtreeCommand.
                        commands.push(Box::new(PasteSubtreeCommand::from_parsed(
                            &parsed_doc,
                            child_id,
                            parent,
                            pres,
                        )));
                    }
                }
            }
        }

        if !commands.is_empty() {
            self.selection.clear();
            if commands.len() == 1 {
                self.execute(commands.into_iter().next().unwrap());
            } else {
                self.execute(Box::new(CompoundCommand::new("Paste", commands)));
            }
            self.events.emit(EditorEvent::SelectionChanged);
        }
    }

    /// Whether the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    /// Record that the document has just been saved or loaded from disk.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Insert every top-level element of `source` into this document.
    ///
    /// Used by the Import command to merge another SVG file, or the output of
    /// the bitmap tracer, into the open drawing. Returns how many elements
    /// were added; the whole import is one undoable step.
    pub fn import_document(&mut self, source: &SvgDocument) -> usize {
        let Some(source_root) = source.root() else {
            return 0;
        };
        let Some(parent) = self.insertion_parent() else {
            return 0;
        };

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for child_id in source.children(source_root).to_vec() {
            let Some(child) = source.get_node(child_id) else {
                continue;
            };
            // `<defs>` and other non-rendering containers come along with the
            // elements that reference them.
            let presentation = child.presentation.clone();
            if source.children(child_id).is_empty() {
                commands.push(Box::new(AddNodeCommand::new(
                    parent,
                    child.element.clone(),
                    presentation,
                    None,
                )));
            } else {
                commands.push(Box::new(PasteSubtreeCommand::from_parsed(
                    source,
                    child_id,
                    parent,
                    presentation,
                )));
            }
        }

        let count = commands.len();
        if count > 0 {
            self.selection.clear();
            self.execute(Box::new(CompoundCommand::new("Import", commands)));
            self.events.emit(EditorEvent::SelectionChanged);
        }
        count
    }

    /// Copy presentation attributes (fill, stroke, opacity, etc.) from the
    /// clipboard's first element to all currently selected elements.
    pub fn paste_style(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }

        let source_pres = match self.extract_clipboard_presentation() {
            Some(p) => p,
            None => return,
        };

        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for &id in &ids {
            if let Some(node) = self.document.get_node(id) {
                let old_pres = node.presentation.clone();
                let mut new_pres = node.presentation.clone();

                // Copy style-related presentation attributes from the source,
                // but preserve the target's transform (positional).
                new_pres.color = source_pres.color;
                new_pres.fill = source_pres.fill.clone();
                new_pres.stroke = source_pres.stroke.clone();
                new_pres.stroke_width = source_pres.stroke_width;
                new_pres.opacity = source_pres.opacity;
                new_pres.fill_opacity = source_pres.fill_opacity;
                new_pres.stroke_opacity = source_pres.stroke_opacity;
                new_pres.stroke_linecap = source_pres.stroke_linecap;
                new_pres.stroke_linejoin = source_pres.stroke_linejoin;
                new_pres.stroke_miterlimit = source_pres.stroke_miterlimit;
                new_pres.stroke_dasharray = source_pres.stroke_dasharray.clone();
                new_pres.stroke_dashoffset = source_pres.stroke_dashoffset;
                new_pres.fill_rule = source_pres.fill_rule;
                new_pres.display = source_pres.display;
                new_pres.visibility = source_pres.visibility;
                new_pres.mix_blend_mode = source_pres.mix_blend_mode;
                new_pres.filter = source_pres.filter.clone();
                new_pres.clip_path = source_pres.clip_path.clone();
                new_pres.mask = source_pres.mask.clone();

                commands.push(Box::new(ModifyAttributeCommand::new(
                    id, old_pres, new_pres,
                )));
            }
        }

        if !commands.is_empty() {
            if commands.len() == 1 {
                self.execute(commands.into_iter().next().unwrap());
            } else {
                self.execute(Box::new(CompoundCommand::new("Paste style", commands)));
            }
        }
    }

    /// Extract the presentation attributes from the first clipboard entry.
    fn extract_clipboard_presentation(&self) -> Option<PresentationAttrs> {
        let entry = self.clipboard.first()?;
        let parsed_doc = sivgo_dom::parse_svg(&entry.svg_xml).ok()?;
        let parsed_root = parsed_doc.root()?;
        let children = parsed_doc.children(parsed_root);
        let first_child_id = children.first()?;
        let node = parsed_doc.get_node(*first_child_id)?;
        Some(node.presentation.clone())
    }

    // -----------------------------------------------------------------------
    // Filter editor operations
    // -----------------------------------------------------------------------

    fn normalize_filter_kind(kind: &str) -> Option<&'static str> {
        let normalized = kind.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        match normalized.as_str() {
            "gaussian_blur" | "fegaussianblur" | "blur" => Some("gaussian_blur"),
            "offset" | "feoffset" => Some("offset"),
            "color_matrix" | "fecolormatrix" => Some("color_matrix"),
            "blend" | "feblend" => Some("blend"),
            "composite" | "fecomposite" => Some("composite"),
            "morphology" | "femorphology" => Some("morphology"),
            "turbulence" | "feturbulence" => Some("turbulence"),
            "displacement_map" | "fedisplacementmap" => Some("displacement_map"),
            "convolve_matrix" | "feconvolvematrix" => Some("convolve_matrix"),
            "flood" | "feflood" => Some("flood"),
            "merge" | "femerge" => Some("merge"),
            _ => None,
        }
    }

    fn default_filter_params(kind: &str) -> Vec<EngineFilterParam> {
        fn param(name: &str, value: &str) -> EngineFilterParam {
            EngineFilterParam {
                name: name.to_string(),
                value: value.to_string(),
            }
        }

        match Self::normalize_filter_kind(kind).unwrap_or(kind) {
            "gaussian_blur" => vec![
                param("input", "SourceGraphic"),
                param("stdDeviationX", "3"),
                param("stdDeviationY", "3"),
                param("result", "blur"),
            ],
            "offset" => vec![
                param("input", "SourceGraphic"),
                param("dx", "4"),
                param("dy", "4"),
                param("result", "offset"),
            ],
            "color_matrix" => vec![
                param("input", "SourceGraphic"),
                param("type", "saturate"),
                param("values", "0"),
                param("result", "color"),
            ],
            "blend" => vec![
                param("input", "SourceGraphic"),
                param("in2", "SourceGraphic"),
                param("mode", "normal"),
                param("result", "blend"),
            ],
            "composite" => vec![
                param("input", "SourceGraphic"),
                param("in2", "SourceGraphic"),
                param("operator", "over"),
                param("k1", "0"),
                param("k2", "1"),
                param("k3", "1"),
                param("k4", "0"),
                param("result", "composite"),
            ],
            "morphology" => vec![
                param("input", "SourceAlpha"),
                param("operator", "dilate"),
                param("radiusX", "2"),
                param("radiusY", "2"),
                param("result", "morphology"),
            ],
            "turbulence" => vec![
                param("type", "turbulence"),
                param("baseFrequencyX", "0.05"),
                param("baseFrequencyY", "0.05"),
                param("numOctaves", "2"),
                param("seed", "0"),
                param("result", "noise"),
            ],
            "displacement_map" => vec![
                param("input", "SourceGraphic"),
                param("in2", "noise"),
                param("scale", "10"),
                param("xChannelSelector", "R"),
                param("yChannelSelector", "G"),
                param("result", "displaced"),
            ],
            "convolve_matrix" => vec![
                param("input", "SourceGraphic"),
                param("orderX", "3"),
                param("orderY", "3"),
                param("kernelMatrix", "0 -1 0 -1 5 -1 0 -1 0"),
                param("divisor", "1"),
                param("bias", "0"),
                param("result", "convolved"),
            ],
            "flood" => vec![
                param("flood-color", "#000000"),
                param("flood-opacity", "0.5"),
                param("result", "flood"),
            ],
            "merge" => vec![param("inputs", "SourceGraphic"), param("result", "merge")],
            _ => Vec::new(),
        }
    }

    fn filter_param_string(
        entry: &EngineFilterPrimitiveEntry,
        name: &str,
        fallback: &str,
    ) -> String {
        entry
            .params
            .iter()
            .find(|param| param.name == name)
            .map(|param| param.value.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn filter_param_f64(entry: &EngineFilterPrimitiveEntry, name: &str, fallback: f64) -> f64 {
        Self::filter_param_string(entry, name, "")
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
    }

    fn filter_param_i32(entry: &EngineFilterPrimitiveEntry, name: &str, fallback: i32) -> i32 {
        Self::filter_param_f64(entry, name, fallback as f64).round() as i32
    }

    fn common_filter_attrs(
        entry: &EngineFilterPrimitiveEntry,
        fallback: &str,
    ) -> FilterPrimitiveAttrs {
        FilterPrimitiveAttrs {
            result: Some(Self::filter_param_string(entry, "result", fallback)),
            ..Default::default()
        }
    }

    fn filter_input_param(
        entry: &EngineFilterPrimitiveEntry,
        name: &str,
        fallback: &str,
    ) -> Option<FilterInput> {
        Some(FilterInput::parse(&Self::filter_param_string(
            entry, name, fallback,
        )))
    }

    fn next_filter_entry_id(entries: &[EngineFilterPrimitiveEntry]) -> String {
        let next = entries
            .iter()
            .filter_map(|entry| entry.id.strip_prefix("fp"))
            .filter_map(|suffix| suffix.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        format!("fp{next}")
    }

    fn selected_filter_target_id(&self) -> Option<NodeId> {
        let id = self.selection.primary()?;
        let node = self.document.get_node(id)?;
        if node.element.is_renderable() && !matches!(node.element, SvgElement::Svg(_)) {
            Some(id)
        } else {
            None
        }
    }

    /// Whether the primary selection can receive an SVG filter.
    pub fn selected_filter_target_available(&self) -> bool {
        self.selected_filter_target_id().is_some()
    }

    fn filter_id_from_ref(filter_ref: &str) -> Option<String> {
        let trimmed = filter_ref.trim();
        trimmed
            .strip_prefix("url(#")
            .and_then(|value| value.strip_suffix(')'))
            .map(str::to_string)
    }

    fn selected_filter_node_id_in_doc(doc: &SvgDocument, target_id: NodeId) -> Option<NodeId> {
        let target = doc.get_node(target_id)?;
        let filter_ref = target
            .presentation
            .filter
            .as_deref()
            .or_else(|| target.attributes.get("filter").map(String::as_str))?;
        let filter_id = Self::filter_id_from_ref(filter_ref)?;
        let node_id = doc.find_by_xml_id(&filter_id)?;
        matches!(doc.get_node(node_id)?.element, SvgElement::Filter(_)).then_some(node_id)
    }

    fn unique_xml_id(doc: &SvgDocument, base: &str) -> String {
        if doc.find_by_xml_id(base).is_none() {
            return base.to_string();
        }
        for index in 2..10_000 {
            let candidate = format!("{base}-{index}");
            if doc.find_by_xml_id(&candidate).is_none() {
                return candidate;
            }
        }
        format!("{base}-{}", NodeId::new())
    }

    fn filter_xml_id_for_target(target_id: NodeId) -> String {
        let raw = target_id.to_string();
        let suffix: String = raw
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .take(16)
            .collect();
        let suffix = if suffix.is_empty() {
            "target".to_string()
        } else {
            suffix
        };
        format!("sivgo-filter-{suffix}")
    }

    fn ensure_defs_in_doc(doc: &mut SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        if let Some(id) = doc.children(root).iter().copied().find(|id| {
            matches!(
                doc.get_node(*id).map(|n| &n.element),
                Some(SvgElement::Defs(_))
            )
        }) {
            return Some(id);
        }

        let defs_id = doc.add_node(SvgElement::Defs(DefsAttrs));
        doc.insert_child_at(root, defs_id, 0);
        Some(defs_id)
    }

    fn ensure_filter_for_target_in_doc(doc: &mut SvgDocument, target_id: NodeId) -> Option<NodeId> {
        if let Some(filter_id) = Self::selected_filter_node_id_in_doc(doc, target_id) {
            return Some(filter_id);
        }

        let defs_id = Self::ensure_defs_in_doc(doc)?;
        let base_id = Self::filter_xml_id_for_target(target_id);
        let xml_id = Self::unique_xml_id(doc, &base_id);
        let filter_id = doc.add_node(SvgElement::Filter(FilterAttrs {
            x: Some(SvgLength::new(-25.0, LengthUnit::Percent)),
            y: Some(SvgLength::new(-25.0, LengthUnit::Percent)),
            width: Some(SvgLength::new(150.0, LengthUnit::Percent)),
            height: Some(SvgLength::new(150.0, LengthUnit::Percent)),
            ..Default::default()
        }));
        if let Some(filter_node) = doc.get_node_mut(filter_id) {
            filter_node.xml_id = Some(xml_id.clone());
        }
        doc.append_child(defs_id, filter_id);

        if let Some(target) = doc.get_node_mut(target_id) {
            let reference = format!("url(#{xml_id})");
            target.presentation.filter = Some(reference.clone());
            target.attributes.insert("filter".to_string(), reference);
        }

        Some(filter_id)
    }

    fn filter_entry_attrs(entry: &EngineFilterPrimitiveEntry) -> HashMap<String, String> {
        let mut attrs = HashMap::new();
        attrs.insert("data-sivgo-filter-entry-id".to_string(), entry.id.clone());
        attrs.insert(
            "data-sivgo-enabled".to_string(),
            if entry.enabled { "true" } else { "false" }.to_string(),
        );
        attrs
    }

    fn parse_kernel_matrix(value: &str) -> Vec<f64> {
        value
            .split_ascii_whitespace()
            .filter_map(|part| part.parse::<f64>().ok())
            .collect()
    }

    fn filter_entry_to_node(entry: &EngineFilterPrimitiveEntry) -> SvgNode {
        let element = match entry.kind.as_str() {
            "gaussian_blur" => SvgElement::FeGaussianBlur(FeGaussianBlurAttrs {
                common: Self::common_filter_attrs(entry, "blur"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                std_deviation_x: Self::filter_param_f64(entry, "stdDeviationX", 3.0),
                std_deviation_y: Self::filter_param_f64(entry, "stdDeviationY", 3.0),
            }),
            "offset" => SvgElement::FeOffset(FeOffsetAttrs {
                common: Self::common_filter_attrs(entry, "offset"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                dx: Self::filter_param_f64(entry, "dx", 4.0),
                dy: Self::filter_param_f64(entry, "dy", 4.0),
            }),
            "color_matrix" => SvgElement::FeColorMatrix(FeColorMatrixAttrs {
                common: Self::common_filter_attrs(entry, "color"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                matrix_type: SvgColorMatrixType::parse(&Self::filter_param_string(
                    entry, "type", "saturate",
                )),
                values: Some(Self::filter_param_string(entry, "values", "0")),
            }),
            "blend" => SvgElement::FeBlend(FeBlendAttrs {
                common: Self::common_filter_attrs(entry, "blend"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                in2: Self::filter_input_param(entry, "in2", "SourceGraphic"),
                mode: SvgBlendMode::parse(&Self::filter_param_string(entry, "mode", "normal")),
            }),
            "composite" => SvgElement::FeComposite(FeCompositeAttrs {
                common: Self::common_filter_attrs(entry, "composite"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                in2: Self::filter_input_param(entry, "in2", "SourceGraphic"),
                operator: SvgCompositeOperator::parse(&Self::filter_param_string(
                    entry, "operator", "over",
                )),
                k1: Self::filter_param_f64(entry, "k1", 0.0),
                k2: Self::filter_param_f64(entry, "k2", 1.0),
                k3: Self::filter_param_f64(entry, "k3", 1.0),
                k4: Self::filter_param_f64(entry, "k4", 0.0),
            }),
            "morphology" => SvgElement::FeMorphology(FeMorphologyAttrs {
                common: Self::common_filter_attrs(entry, "morphology"),
                input: Self::filter_input_param(entry, "input", "SourceAlpha"),
                operator: SvgMorphologyOperator::parse(&Self::filter_param_string(
                    entry, "operator", "dilate",
                )),
                radius_x: Self::filter_param_f64(entry, "radiusX", 2.0),
                radius_y: Self::filter_param_f64(entry, "radiusY", 2.0),
            }),
            "turbulence" => SvgElement::FeTurbulence(FeTurbulenceAttrs {
                common: Self::common_filter_attrs(entry, "noise"),
                turbulence_type: SvgTurbulenceType::parse(&Self::filter_param_string(
                    entry,
                    "type",
                    "turbulence",
                )),
                base_frequency_x: Self::filter_param_f64(entry, "baseFrequencyX", 0.05),
                base_frequency_y: Self::filter_param_f64(entry, "baseFrequencyY", 0.05),
                num_octaves: Some(Self::filter_param_i32(entry, "numOctaves", 2).max(0)),
                seed: Some(Self::filter_param_f64(entry, "seed", 0.0)),
                stitch_tiles: None,
            }),
            "displacement_map" => SvgElement::FeDisplacementMap(FeDisplacementMapAttrs {
                common: Self::common_filter_attrs(entry, "displaced"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                in2: Self::filter_input_param(entry, "in2", "noise"),
                scale: Some(Self::filter_param_f64(entry, "scale", 10.0)),
                x_channel_selector: SvgChannelSelector::parse(&Self::filter_param_string(
                    entry,
                    "xChannelSelector",
                    "R",
                )),
                y_channel_selector: SvgChannelSelector::parse(&Self::filter_param_string(
                    entry,
                    "yChannelSelector",
                    "G",
                )),
            }),
            "convolve_matrix" => SvgElement::FeConvolveMatrix(FeConvolveMatrixAttrs {
                common: Self::common_filter_attrs(entry, "convolved"),
                input: Self::filter_input_param(entry, "input", "SourceGraphic"),
                order_x: Self::filter_param_i32(entry, "orderX", 3).max(1),
                order_y: Self::filter_param_i32(entry, "orderY", 3).max(1),
                kernel_matrix: Self::parse_kernel_matrix(&Self::filter_param_string(
                    entry,
                    "kernelMatrix",
                    "0 -1 0 -1 5 -1 0 -1 0",
                )),
                divisor: Some(Self::filter_param_f64(entry, "divisor", 1.0)),
                bias: Self::filter_param_f64(entry, "bias", 0.0),
                target_x: None,
                target_y: None,
                edge_mode: None,
                preserve_alpha: false,
            }),
            "flood" => SvgElement::FeFlood(FeFloodAttrs {
                common: Self::common_filter_attrs(entry, "flood"),
                flood_color: Color::parse(&Self::filter_param_string(
                    entry,
                    "flood-color",
                    "#000000",
                )),
                flood_opacity: Some(Self::filter_param_f64(entry, "flood-opacity", 0.5)),
            }),
            "merge" => SvgElement::FeMerge(FeMergeAttrs {
                common: Self::common_filter_attrs(entry, "merge"),
            }),
            _ => SvgElement::FeGaussianBlur(FeGaussianBlurAttrs {
                common: Self::common_filter_attrs(entry, "blur"),
                input: Some(FilterInput::SourceGraphic),
                std_deviation_x: 3.0,
                std_deviation_y: 3.0,
            }),
        };

        let mut node = SvgNode::new(NodeId::new(), element);
        node.attributes = Self::filter_entry_attrs(entry);
        node
    }

    fn append_filter_entry_to_doc(
        doc: &mut SvgDocument,
        filter_id: NodeId,
        entry: &EngineFilterPrimitiveEntry,
    ) {
        let node = Self::filter_entry_to_node(entry);
        let primitive_id = node.id;
        doc.insert_node(node);
        doc.append_child(filter_id, primitive_id);

        if entry.kind == "merge" {
            let inputs = Self::filter_param_string(entry, "inputs", "SourceGraphic");
            for input in inputs
                .split([',', ';'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let merge_node_id = doc.add_node(SvgElement::FeMergeNode(FeMergeNodeAttrs {
                    input: Some(FilterInput::parse(input)),
                }));
                doc.append_child(primitive_id, merge_node_id);
            }
        }
    }

    fn filter_entry_from_node(
        doc: &SvgDocument,
        node_id: NodeId,
        fallback_index: usize,
    ) -> Option<EngineFilterPrimitiveEntry> {
        let node = doc.get_node(node_id)?;
        let id = node
            .attributes
            .get("data-sivgo-filter-entry-id")
            .or_else(|| node.attributes.get("sivgo:filter-entry-id"))
            .cloned()
            .unwrap_or_else(|| format!("fp{}", fallback_index + 1));
        let enabled = node
            .attributes
            .get("data-sivgo-enabled")
            .or_else(|| node.attributes.get("sivgo:enabled"))
            .map(|value| value != "false" && value != "0")
            .unwrap_or(true);
        let result = |common: &FilterPrimitiveAttrs, fallback: &str| {
            common
                .result
                .clone()
                .unwrap_or_else(|| fallback.to_string())
        };
        let param = |name: &str, value: String| EngineFilterParam {
            name: name.to_string(),
            value,
        };
        let input_value = |input: &Option<FilterInput>, fallback: &str| {
            input
                .as_ref()
                .map(FilterInput::to_svg)
                .unwrap_or_else(|| fallback.to_string())
        };

        let (kind, params) = match &node.element {
            SvgElement::FeGaussianBlur(attrs) => (
                "gaussian_blur",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("stdDeviationX", attrs.std_deviation_x.to_string()),
                    param("stdDeviationY", attrs.std_deviation_y.to_string()),
                    param("result", result(&attrs.common, "blur")),
                ],
            ),
            SvgElement::FeOffset(attrs) => (
                "offset",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("dx", attrs.dx.to_string()),
                    param("dy", attrs.dy.to_string()),
                    param("result", result(&attrs.common, "offset")),
                ],
            ),
            SvgElement::FeColorMatrix(attrs) => (
                "color_matrix",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param(
                        "type",
                        attrs
                            .matrix_type
                            .map(|kind| kind.to_svg().to_string())
                            .unwrap_or_else(|| "matrix".to_string()),
                    ),
                    param("values", attrs.values.clone().unwrap_or_default()),
                    param("result", result(&attrs.common, "color")),
                ],
            ),
            SvgElement::FeBlend(attrs) => (
                "blend",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("in2", input_value(&attrs.in2, "SourceGraphic")),
                    param(
                        "mode",
                        attrs
                            .mode
                            .map(|mode| mode.to_svg().to_string())
                            .unwrap_or_else(|| "normal".to_string()),
                    ),
                    param("result", result(&attrs.common, "blend")),
                ],
            ),
            SvgElement::FeComposite(attrs) => (
                "composite",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("in2", input_value(&attrs.in2, "SourceGraphic")),
                    param(
                        "operator",
                        attrs
                            .operator
                            .map(|op| op.to_svg().to_string())
                            .unwrap_or_else(|| "over".to_string()),
                    ),
                    param("k1", attrs.k1.to_string()),
                    param("k2", attrs.k2.to_string()),
                    param("k3", attrs.k3.to_string()),
                    param("k4", attrs.k4.to_string()),
                    param("result", result(&attrs.common, "composite")),
                ],
            ),
            SvgElement::FeMorphology(attrs) => (
                "morphology",
                vec![
                    param("input", input_value(&attrs.input, "SourceAlpha")),
                    param(
                        "operator",
                        attrs
                            .operator
                            .map(|op| op.to_svg().to_string())
                            .unwrap_or_else(|| "dilate".to_string()),
                    ),
                    param("radiusX", attrs.radius_x.to_string()),
                    param("radiusY", attrs.radius_y.to_string()),
                    param("result", result(&attrs.common, "morphology")),
                ],
            ),
            SvgElement::FeTurbulence(attrs) => (
                "turbulence",
                vec![
                    param(
                        "type",
                        attrs
                            .turbulence_type
                            .map(|kind| kind.to_svg().to_string())
                            .unwrap_or_else(|| "turbulence".to_string()),
                    ),
                    param("baseFrequencyX", attrs.base_frequency_x.to_string()),
                    param("baseFrequencyY", attrs.base_frequency_y.to_string()),
                    param("numOctaves", attrs.num_octaves.unwrap_or(1).to_string()),
                    param("seed", attrs.seed.unwrap_or(0.0).to_string()),
                    param("result", result(&attrs.common, "noise")),
                ],
            ),
            SvgElement::FeDisplacementMap(attrs) => (
                "displacement_map",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("in2", input_value(&attrs.in2, "noise")),
                    param("scale", attrs.scale.unwrap_or(10.0).to_string()),
                    param(
                        "xChannelSelector",
                        attrs
                            .x_channel_selector
                            .map(|channel| channel.to_svg().to_string())
                            .unwrap_or_else(|| "R".to_string()),
                    ),
                    param(
                        "yChannelSelector",
                        attrs
                            .y_channel_selector
                            .map(|channel| channel.to_svg().to_string())
                            .unwrap_or_else(|| "G".to_string()),
                    ),
                    param("result", result(&attrs.common, "displaced")),
                ],
            ),
            SvgElement::FeConvolveMatrix(attrs) => (
                "convolve_matrix",
                vec![
                    param("input", input_value(&attrs.input, "SourceGraphic")),
                    param("orderX", attrs.order_x.to_string()),
                    param("orderY", attrs.order_y.to_string()),
                    param(
                        "kernelMatrix",
                        attrs
                            .kernel_matrix
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                    param("divisor", attrs.divisor.unwrap_or(1.0).to_string()),
                    param("bias", attrs.bias.to_string()),
                    param("result", result(&attrs.common, "convolved")),
                ],
            ),
            SvgElement::FeFlood(attrs) => (
                "flood",
                vec![
                    param(
                        "flood-color",
                        attrs
                            .flood_color
                            .map(|color| format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b))
                            .unwrap_or_else(|| "#000000".to_string()),
                    ),
                    param(
                        "flood-opacity",
                        attrs.flood_opacity.unwrap_or(1.0).to_string(),
                    ),
                    param("result", result(&attrs.common, "flood")),
                ],
            ),
            SvgElement::FeMerge(attrs) => {
                let inputs = doc
                    .children(node_id)
                    .iter()
                    .filter_map(|child_id| match &doc.get_node(*child_id)?.element {
                        SvgElement::FeMergeNode(attrs) => Some(input_value(&attrs.input, "")),
                        _ => None,
                    })
                    .filter(|input| !input.is_empty())
                    .collect::<Vec<_>>()
                    .join(",");
                (
                    "merge",
                    vec![
                        param("inputs", inputs),
                        param("result", result(&attrs.common, "merge")),
                    ],
                )
            }
            _ => return None,
        };

        Some(EngineFilterPrimitiveEntry {
            id,
            kind: kind.to_string(),
            enabled,
            params,
        })
    }

    fn selected_filter_primitives_in_doc(
        doc: &SvgDocument,
        target_id: NodeId,
    ) -> Vec<EngineFilterPrimitiveEntry> {
        let Some(filter_id) = Self::selected_filter_node_id_in_doc(doc, target_id) else {
            return Vec::new();
        };
        doc.children(filter_id)
            .iter()
            .enumerate()
            .filter_map(|(idx, child_id)| Self::filter_entry_from_node(doc, *child_id, idx))
            .collect()
    }

    fn rebuild_filter_in_doc(
        doc: &mut SvgDocument,
        target_id: NodeId,
        entries: &[EngineFilterPrimitiveEntry],
    ) -> Option<()> {
        if entries.is_empty() {
            if let Some(filter_id) = Self::selected_filter_node_id_in_doc(doc, target_id) {
                doc.remove_node(filter_id);
            }
            if let Some(target) = doc.get_node_mut(target_id) {
                target.presentation.filter = None;
                target.attributes.remove("filter");
            }
            return Some(());
        }

        let filter_id = Self::ensure_filter_for_target_in_doc(doc, target_id)?;
        for child in doc.children(filter_id).to_vec() {
            doc.remove_node(child);
        }
        for entry in entries {
            Self::append_filter_entry_to_doc(doc, filter_id, entry);
        }
        Some(())
    }

    fn modify_selected_filter_stack<F>(&mut self, description: &str, update: F) -> bool
    where
        F: FnOnce(&mut Vec<EngineFilterPrimitiveEntry>),
    {
        let Some(target_id) = self.selected_filter_target_id() else {
            return false;
        };
        let mut after = self.document.clone();
        let mut entries = Self::selected_filter_primitives_in_doc(&after, target_id);
        update(&mut entries);
        if Self::rebuild_filter_in_doc(&mut after, target_id, &entries).is_none() {
            return false;
        }
        let before = self.document.clone();
        self.execute(Box::new(DocumentSnapshotCommand::new(
            description,
            before,
            after,
        )));
        self.events.emit(EditorEvent::SelectionChanged);
        true
    }

    /// Return the editable SVG filter chain for the primary selected object.
    pub fn selected_filter_primitives(&self) -> Vec<EngineFilterPrimitiveEntry> {
        let Some(target_id) = self.selected_filter_target_id() else {
            return Vec::new();
        };
        Self::selected_filter_primitives_in_doc(&self.document, target_id)
    }

    /// Add an SVG filter primitive to the primary selected object.
    pub fn add_filter_primitive_to_selected(&mut self, kind: &str) -> Option<String> {
        let kind = Self::normalize_filter_kind(kind)?.to_string();
        let mut added_id = None;
        let updated = self.modify_selected_filter_stack("Add filter primitive", |entries| {
            let id = Self::next_filter_entry_id(entries);
            let params = Self::default_filter_params(&kind);
            added_id = Some(id.clone());
            entries.push(EngineFilterPrimitiveEntry {
                id,
                kind: kind.clone(),
                enabled: true,
                params,
            });
        });
        updated.then_some(added_id).flatten()
    }

    /// Remove an SVG filter primitive from the selected object's chain.
    pub fn remove_filter_primitive_from_selected(&mut self, primitive_id: &str) -> bool {
        self.modify_selected_filter_stack("Remove filter primitive", |entries| {
            entries.retain(|entry| entry.id != primitive_id);
        })
    }

    /// Toggle one SVG filter primitive.
    pub fn toggle_filter_primitive_on_selected(&mut self, primitive_id: &str) -> bool {
        self.modify_selected_filter_stack("Toggle filter primitive", |entries| {
            if let Some(entry) = entries.iter_mut().find(|entry| entry.id == primitive_id) {
                entry.enabled = !entry.enabled;
            }
        })
    }

    /// Move a filter primitive up or down in the chain.
    pub fn move_filter_primitive_on_selected(&mut self, primitive_id: &str, delta: isize) -> bool {
        self.modify_selected_filter_stack("Move filter primitive", |entries| {
            let Some(index) = entries.iter().position(|entry| entry.id == primitive_id) else {
                return;
            };
            let target = if delta < 0 {
                index.saturating_sub(1)
            } else {
                (index + 1).min(entries.len().saturating_sub(1))
            };
            if target != index {
                let entry = entries.remove(index);
                entries.insert(target, entry);
            }
        })
    }

    /// Set one parameter on a selected object's filter primitive.
    pub fn set_filter_param_on_selected(
        &mut self,
        primitive_id: &str,
        param_name: &str,
        value: String,
    ) -> bool {
        self.modify_selected_filter_stack("Set filter parameter", |entries| {
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == primitive_id) else {
                return;
            };
            if let Some(param) = entry
                .params
                .iter_mut()
                .find(|param| param.name == param_name)
            {
                param.value = value;
            } else {
                entry.params.push(EngineFilterParam {
                    name: param_name.to_string(),
                    value,
                });
            }
        })
    }

    /// Apply a built-in filter preset to the selected object.
    pub fn apply_filter_preset_to_selected(&mut self, preset: &str) -> bool {
        let entries = match preset {
            "blur" | "simple_blur" => vec![EngineFilterPrimitiveEntry {
                id: "fp1".to_string(),
                kind: "gaussian_blur".to_string(),
                enabled: true,
                params: Self::default_filter_params("gaussian_blur"),
            }],
            "drop_shadow" => Self::drop_shadow_filter_entries(),
            _ => return false,
        };

        self.modify_selected_filter_stack("Apply filter preset", |current| {
            *current = entries;
        })
    }

    fn filter_param(name: &str, value: &str) -> EngineFilterParam {
        EngineFilterParam {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn drop_shadow_filter_entries() -> Vec<EngineFilterPrimitiveEntry> {
        vec![
            EngineFilterPrimitiveEntry {
                id: "fp1".to_string(),
                kind: "gaussian_blur".to_string(),
                enabled: true,
                params: vec![
                    Self::filter_param("input", "SourceAlpha"),
                    Self::filter_param("stdDeviationX", "3"),
                    Self::filter_param("stdDeviationY", "3"),
                    Self::filter_param("result", "blur"),
                ],
            },
            EngineFilterPrimitiveEntry {
                id: "fp2".to_string(),
                kind: "offset".to_string(),
                enabled: true,
                params: vec![
                    Self::filter_param("input", "blur"),
                    Self::filter_param("dx", "4"),
                    Self::filter_param("dy", "4"),
                    Self::filter_param("result", "offsetBlur"),
                ],
            },
            EngineFilterPrimitiveEntry {
                id: "fp3".to_string(),
                kind: "flood".to_string(),
                enabled: true,
                params: vec![
                    Self::filter_param("flood-color", "#000000"),
                    Self::filter_param("flood-opacity", "0.5"),
                    Self::filter_param("result", "flood"),
                ],
            },
            EngineFilterPrimitiveEntry {
                id: "fp4".to_string(),
                kind: "composite".to_string(),
                enabled: true,
                params: vec![
                    Self::filter_param("input", "flood"),
                    Self::filter_param("in2", "offsetBlur"),
                    Self::filter_param("operator", "in"),
                    Self::filter_param("k1", "0"),
                    Self::filter_param("k2", "1"),
                    Self::filter_param("k3", "1"),
                    Self::filter_param("k4", "0"),
                    Self::filter_param("result", "shadow"),
                ],
            },
            EngineFilterPrimitiveEntry {
                id: "fp5".to_string(),
                kind: "merge".to_string(),
                enabled: true,
                params: vec![
                    Self::filter_param("inputs", "shadow,SourceGraphic"),
                    Self::filter_param("result", "merge"),
                ],
            },
        ]
    }

    // -----------------------------------------------------------------------
    // Document resource inventory
    // -----------------------------------------------------------------------

    fn resource_kind_for_element(element: &SvgElement) -> Option<&'static str> {
        match element {
            SvgElement::LinearGradient(_)
            | SvgElement::RadialGradient(_)
            | SvgElement::MeshGradient(_) => Some("gradient"),
            SvgElement::Pattern(_) => Some("pattern"),
            SvgElement::Marker(_) => Some("marker"),
            SvgElement::Symbol(_) => Some("symbol"),
            SvgElement::Filter(_) => Some("filter"),
            SvgElement::ClipPath(_) => Some("clip_path"),
            SvgElement::Mask(_) => Some("mask"),
            SvgElement::Image(_) => Some("image"),
            SvgElement::SvgFont(_)
            | SvgElement::FontFace(_)
            | SvgElement::Glyph(_)
            | SvgElement::MissingGlyph(_) => Some("font"),
            _ => None,
        }
    }

    fn resource_name_for_node(node: &SvgNode, xml_id: &str) -> String {
        node.attributes
            .get("inkscape:label")
            .or_else(|| node.attributes.get("aria-label"))
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| xml_id.to_string())
    }

    fn url_value_matches(value: &str, xml_id: &str) -> bool {
        let needle = format!("#{xml_id}");
        value.contains(&needle)
    }

    fn paint_uses_resource(paint: &Option<Paint>, xml_id: &str) -> bool {
        matches!(paint, Some(Paint::Url(url)) if Self::url_value_matches(url, xml_id))
    }

    fn href_uses_resource(href: &Option<String>, xml_id: &str) -> bool {
        href.as_deref()
            .is_some_and(|href| Self::url_value_matches(href, xml_id))
    }

    fn node_uses_resource(node: &SvgNode, xml_id: &str) -> bool {
        let pres = &node.presentation;
        Self::paint_uses_resource(&pres.fill, xml_id)
            || Self::paint_uses_resource(&pres.stroke, xml_id)
            || pres
                .filter
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || pres
                .clip_path
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || pres
                .mask
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || pres
                .marker_start
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || pres
                .marker_mid
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || pres
                .marker_end
                .as_deref()
                .is_some_and(|value| Self::url_value_matches(value, xml_id))
            || node
                .attributes
                .values()
                .any(|value| Self::url_value_matches(value, xml_id))
            || match &node.element {
                SvgElement::Use(attrs) => Self::href_uses_resource(&attrs.href, xml_id),
                SvgElement::LinearGradient(attrs) => Self::href_uses_resource(&attrs.href, xml_id),
                SvgElement::RadialGradient(attrs) => Self::href_uses_resource(&attrs.href, xml_id),
                SvgElement::Pattern(attrs) => Self::href_uses_resource(&attrs.href, xml_id),
                _ => false,
            }
    }

    fn resource_usage_count(&self, resource_node_id: NodeId, xml_id: &str) -> usize {
        self.document
            .nodes()
            .filter(|(id, node)| **id != resource_node_id && Self::node_uses_resource(node, xml_id))
            .count()
    }

    /// Return all reusable resources currently defined in the document.
    pub fn document_resources(&self) -> Vec<EngineResourceEntry> {
        let mut resources = self
            .document
            .nodes()
            .filter_map(|(&node_id, node)| {
                let kind = Self::resource_kind_for_element(&node.element)?;
                let xml_id = node
                    .xml_id
                    .clone()
                    .unwrap_or_else(|| format!("node-{}", node_id));
                let usage_count = if node.xml_id.is_some() {
                    self.resource_usage_count(node_id, &xml_id)
                } else if matches!(node.element, SvgElement::Image(_)) {
                    1
                } else {
                    0
                };
                Some(EngineResourceEntry {
                    id: xml_id.clone(),
                    kind: kind.to_string(),
                    name: Self::resource_name_for_node(node, &xml_id),
                    usage_count,
                })
            })
            .collect::<Vec<_>>();

        resources.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id)));
        resources
    }

    fn unused_resource_node_ids(&self) -> Vec<NodeId> {
        self.document
            .nodes()
            .filter_map(|(&node_id, node)| {
                let _ = Self::resource_kind_for_element(&node.element)?;
                let xml_id = node.xml_id.as_deref()?;
                (self.resource_usage_count(node_id, xml_id) == 0).then_some(node_id)
            })
            .collect()
    }

    /// Delete one unused resource by SVG id.
    pub fn delete_unused_resource(&mut self, resource_id: &str) -> bool {
        let Some(node_id) = self.document.find_by_xml_id(resource_id) else {
            return false;
        };
        let Some(node) = self.document.get_node(node_id) else {
            return false;
        };
        if Self::resource_kind_for_element(&node.element).is_none()
            || self.resource_usage_count(node_id, resource_id) != 0
        {
            return false;
        }
        self.execute(Box::new(RemoveNodeCommand::new(node_id)));
        true
    }

    /// Delete every unused resource with an SVG id.
    pub fn delete_all_unused_resources(&mut self) -> usize {
        let ids = self.unused_resource_node_ids();
        if ids.is_empty() {
            return 0;
        }
        let count = ids.len();
        let commands = ids
            .into_iter()
            .map(|id| Box::new(RemoveNodeCommand::new(id)) as Box<dyn Command>)
            .collect();
        self.execute_command_batch("Delete unused resources", commands);
        count
    }

    // -----------------------------------------------------------------------
    // Text editing operations
    // -----------------------------------------------------------------------

    fn text_content_from_element(element: &SvgElement) -> Option<String> {
        match element {
            SvgElement::Text(attrs) => Some(attrs.text_content.clone()),
            SvgElement::TSpan(attrs) => Some(attrs.text_content.clone()),
            SvgElement::TextPath(attrs) => Some(attrs.text_content.clone()),
            SvgElement::FlowPara(attrs) => Some(attrs.text_content.clone()),
            SvgElement::FlowSpan(attrs) => Some(attrs.text_content.clone()),
            _ => None,
        }
    }

    fn set_text_content_on_element(element: &mut SvgElement, content: String) -> bool {
        match element {
            SvgElement::Text(attrs) => attrs.text_content = content,
            SvgElement::TSpan(attrs) => attrs.text_content = content,
            SvgElement::TextPath(attrs) => attrs.text_content = content,
            SvgElement::FlowPara(attrs) => attrs.text_content = content,
            SvgElement::FlowSpan(attrs) => attrs.text_content = content,
            _ => return false,
        }
        true
    }

    fn selected_text_node_id(&self) -> Option<NodeId> {
        let id = self.selection.primary()?;
        let node = self.document.get_node(id)?;
        Self::text_content_from_element(&node.element).map(|_| id)
    }

    fn text_font_family(node: &SvgNode) -> String {
        match &node.element {
            SvgElement::Text(attrs) => attrs
                .font_family
                .clone()
                .or_else(|| node.presentation.font_family.clone()),
            SvgElement::TSpan(attrs) => attrs
                .font_family
                .clone()
                .or_else(|| node.presentation.font_family.clone()),
            _ => node.presentation.font_family.clone(),
        }
        .unwrap_or_else(|| "sans-serif".to_string())
    }

    fn text_font_size(node: &SvgNode) -> f64 {
        match &node.element {
            SvgElement::Text(attrs) => attrs.font_size.or(node.presentation.font_size),
            SvgElement::TSpan(attrs) => attrs.font_size.or(node.presentation.font_size),
            _ => node.presentation.font_size,
        }
        .map(|length| length.to_px())
        .unwrap_or(16.0)
    }

    fn text_font_weight(node: &SvgNode) -> String {
        let weight = match &node.element {
            SvgElement::Text(attrs) => attrs.font_weight.or(node.presentation.font_weight),
            SvgElement::TSpan(attrs) => attrs.font_weight.or(node.presentation.font_weight),
            _ => node.presentation.font_weight,
        }
        .unwrap_or(sivgo_dom::FontWeight::Normal);
        match weight {
            sivgo_dom::FontWeight::Normal => "Normal".to_string(),
            sivgo_dom::FontWeight::Bold => "Bold".to_string(),
            sivgo_dom::FontWeight::Number(value) => value.to_string(),
        }
    }

    fn text_font_style(node: &SvgNode) -> String {
        let style = match &node.element {
            SvgElement::Text(attrs) => attrs.font_style.or(node.presentation.font_style),
            SvgElement::TSpan(attrs) => attrs.font_style.or(node.presentation.font_style),
            _ => node.presentation.font_style,
        }
        .unwrap_or(sivgo_dom::FontStyle::Normal);
        match style {
            sivgo_dom::FontStyle::Normal => "Normal",
            sivgo_dom::FontStyle::Italic => "Italic",
            sivgo_dom::FontStyle::Oblique => "Oblique",
        }
        .to_string()
    }

    fn text_anchor(node: &SvgNode) -> String {
        let anchor = match &node.element {
            SvgElement::Text(attrs) => attrs.text_anchor.or(node.presentation.text_anchor),
            SvgElement::TSpan(attrs) => attrs.text_anchor.or(node.presentation.text_anchor),
            _ => node.presentation.text_anchor,
        }
        .unwrap_or(SvgTextAnchor::Start);
        match anchor {
            SvgTextAnchor::Start => "Start",
            SvgTextAnchor::Middle => "Middle",
            SvgTextAnchor::End => "End",
        }
        .to_string()
    }

    /// Return editable text state for the primary selected text-like node.
    pub fn selected_text_state(&self) -> Option<EngineTextState> {
        let id = self.selected_text_node_id()?;
        let node = self.document.get_node(id)?;
        Some(EngineTextState {
            content: Self::text_content_from_element(&node.element)?,
            font_family: Self::text_font_family(node),
            font_size: Self::text_font_size(node),
            font_weight: Self::text_font_weight(node),
            font_style: Self::text_font_style(node),
            line_height: node
                .attributes
                .get("line-height")
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(1.2),
            letter_spacing: node
                .presentation
                .letter_spacing
                .map(|length| length.to_px())
                .unwrap_or(0.0),
            word_spacing: node
                .presentation
                .word_spacing
                .map(|length| length.to_px())
                .unwrap_or(0.0),
            text_anchor: Self::text_anchor(node),
        })
    }

    /// Set text content on the primary selected text-like node.
    pub fn set_selected_text_content(&mut self, content: String) -> bool {
        let Some(id) = self.selected_text_node_id() else {
            return false;
        };
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        let old_element = node.element.clone();
        let old_presentation = node.presentation.clone();
        let mut new_element = old_element.clone();
        if !Self::set_text_content_on_element(&mut new_element, content) {
            return false;
        }
        self.execute(Box::new(ModifyElementCommand::new(
            id,
            old_element,
            new_element,
            old_presentation.clone(),
            old_presentation,
        )));
        true
    }

    fn modify_selected_text_presentation<F>(&mut self, description: &str, update: F) -> bool
    where
        F: FnOnce(&mut PresentationAttrs),
    {
        let Some(id) = self.selected_text_node_id() else {
            return false;
        };
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        let old = node.presentation.clone();
        let mut new = old.clone();
        update(&mut new);
        self.execute_command_batch(
            description,
            vec![Box::new(ModifyAttributeCommand::new(id, old, new))],
        );
        true
    }

    fn modify_selected_text_style<F>(&mut self, update: F) -> bool
    where
        F: FnOnce(&mut SvgElement, &mut PresentationAttrs),
    {
        let Some(id) = self.selected_text_node_id() else {
            return false;
        };
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        let old_element = node.element.clone();
        let old_presentation = node.presentation.clone();
        let mut new_element = old_element.clone();
        let mut new_presentation = old_presentation.clone();
        update(&mut new_element, &mut new_presentation);
        self.execute(Box::new(ModifyElementCommand::new(
            id,
            old_element,
            new_element,
            old_presentation,
            new_presentation,
        )));
        true
    }

    /// Set the corner radius on every selected rectangle.
    ///
    /// Returns the number of rectangles changed. Both `rx` and `ry` are set so
    /// the corners stay circular; a radius of zero restores square corners.
    pub fn set_selected_corner_radius(&mut self, radius: f64) -> usize {
        let radius = radius.max(0.0);
        let ids: Vec<NodeId> = self.selection.selected().to_vec();

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for id in ids {
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            let SvgElement::Rect(attrs) = &node.element else {
                continue;
            };
            let old_element = node.element.clone();
            let mut new_attrs = attrs.clone();
            new_attrs.rx = SvgLength::px(radius);
            new_attrs.ry = SvgLength::px(radius);
            let presentation = node.presentation.clone();
            commands.push(Box::new(ModifyElementCommand::new(
                id,
                old_element,
                SvgElement::Rect(new_attrs),
                presentation.clone(),
                presentation,
            )));
        }

        let changed = commands.len();
        if changed > 0 {
            self.execute(Box::new(CompoundCommand::new(
                "Set Corner Radius",
                commands,
            )));
        }
        changed
    }

    pub fn set_selected_text_font_family(&mut self, family: String) -> bool {
        let family = if family.trim().is_empty() {
            "sans-serif".to_string()
        } else {
            family
        };
        self.modify_selected_text_style(|element, presentation| {
            match element {
                SvgElement::Text(attrs) => attrs.font_family = Some(family.clone()),
                SvgElement::TSpan(attrs) => attrs.font_family = Some(family.clone()),
                _ => {}
            }
            presentation.font_family = Some(family);
        })
    }

    pub fn set_selected_text_font_size(&mut self, size: f64) -> bool {
        let length = SvgLength::new(size.clamp(1.0, 1000.0), LengthUnit::None);
        self.modify_selected_text_style(|element, presentation| {
            match element {
                SvgElement::Text(attrs) => attrs.font_size = Some(length),
                SvgElement::TSpan(attrs) => attrs.font_size = Some(length),
                _ => {}
            }
            presentation.font_size = Some(length);
        })
    }

    pub fn set_selected_text_font_weight(&mut self, weight: &str) -> bool {
        let parsed = match weight.trim() {
            "Bold" | "bold" => sivgo_dom::FontWeight::Bold,
            "Normal" | "normal" => sivgo_dom::FontWeight::Normal,
            value => value
                .parse::<f64>()
                .map(sivgo_dom::FontWeight::Number)
                .unwrap_or(sivgo_dom::FontWeight::Normal),
        };
        self.modify_selected_text_style(|element, presentation| {
            match element {
                SvgElement::Text(attrs) => attrs.font_weight = Some(parsed),
                SvgElement::TSpan(attrs) => attrs.font_weight = Some(parsed),
                _ => {}
            }
            presentation.font_weight = Some(parsed);
        })
    }

    pub fn set_selected_text_font_style(&mut self, style: &str) -> bool {
        let parsed = match style.trim() {
            "Italic" | "italic" => sivgo_dom::FontStyle::Italic,
            "Oblique" | "oblique" => sivgo_dom::FontStyle::Oblique,
            _ => sivgo_dom::FontStyle::Normal,
        };
        self.modify_selected_text_style(|element, presentation| {
            match element {
                SvgElement::Text(attrs) => attrs.font_style = Some(parsed),
                SvgElement::TSpan(attrs) => attrs.font_style = Some(parsed),
                _ => {}
            }
            presentation.font_style = Some(parsed);
        })
    }

    pub fn set_selected_text_letter_spacing(&mut self, spacing: f64) -> bool {
        self.modify_selected_text_presentation("Set text letter spacing", |presentation| {
            presentation.letter_spacing = Some(SvgLength::new(spacing, LengthUnit::None));
        })
    }

    pub fn set_selected_text_word_spacing(&mut self, spacing: f64) -> bool {
        self.modify_selected_text_presentation("Set text word spacing", |presentation| {
            presentation.word_spacing = Some(SvgLength::new(spacing, LengthUnit::None));
        })
    }

    pub fn set_selected_text_anchor(&mut self, anchor: &str) -> bool {
        let parsed = match anchor.trim() {
            "Middle" | "middle" => SvgTextAnchor::Middle,
            "End" | "end" => SvgTextAnchor::End,
            _ => SvgTextAnchor::Start,
        };
        self.modify_selected_text_style(|element, presentation| {
            match element {
                SvgElement::Text(attrs) => attrs.text_anchor = Some(parsed),
                SvgElement::TSpan(attrs) => attrs.text_anchor = Some(parsed),
                _ => {}
            }
            presentation.text_anchor = Some(parsed);
        })
    }

    pub fn set_selected_text_line_height(&mut self, line_height: f64) -> bool {
        let Some(id) = self.selected_text_node_id() else {
            return false;
        };
        let mut after = self.document.clone();
        if let Some(node) = after.get_node_mut(id) {
            node.attributes.insert(
                "line-height".to_string(),
                line_height.clamp(0.1, 20.0).to_string(),
            );
        }
        let before = self.document.clone();
        self.execute(Box::new(DocumentSnapshotCommand::new(
            "Set text line height",
            before,
            after,
        )));
        true
    }

    // -----------------------------------------------------------------------
    // Path operations
    // -----------------------------------------------------------------------

    fn path_element(path: BezPath) -> SvgElement {
        SvgElement::Path(PathAttrs {
            d: path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        })
    }

    fn execute_command_batch(&mut self, description: &str, mut commands: Vec<Box<dyn Command>>) {
        if commands.is_empty() {
            return;
        }

        if commands.len() == 1 {
            self.execute(commands.pop().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new(description, commands)));
        }
    }

    fn selected_convertible_paths(&self) -> Vec<PathSelectionItem> {
        self.selection
            .selected()
            .iter()
            .filter_map(|&id| {
                let node = self.document.get_node(id)?;
                let mut path = element_to_path(&node.element)?;
                if path.is_empty() {
                    return None;
                }
                let mut baked_presentation = node.presentation.clone();
                if let Some(transform) = baked_presentation.transform {
                    if !transform.is_identity() {
                        path = path.transformed(&transform);
                    }
                    baked_presentation.transform = None;
                }
                Some(PathSelectionItem {
                    id,
                    element: node.element.clone(),
                    presentation: node.presentation.clone(),
                    baked_presentation,
                    path,
                })
            })
            .collect()
    }

    fn replace_primary_path_and_remove_rest(
        &mut self,
        description: &str,
        items: Vec<PathSelectionItem>,
        path: BezPath,
        presentation: Option<PresentationAttrs>,
    ) {
        if items.is_empty() || path.is_empty() {
            return;
        }

        let primary = &items[0];
        let new_presentation = presentation.unwrap_or_else(|| primary.baked_presentation.clone());
        let mut commands: Vec<Box<dyn Command>> = vec![Box::new(ModifyElementCommand::new(
            primary.id,
            primary.element.clone(),
            Self::path_element(path),
            primary.presentation.clone(),
            new_presentation,
        ))];

        commands.extend(
            items
                .iter()
                .skip(1)
                .map(|item| Box::new(RemoveNodeCommand::new(item.id)) as Box<dyn Command>),
        );

        let primary_id = primary.id;
        self.execute_command_batch(description, commands);
        self.selection.select(primary_id);
        self.events.emit(EditorEvent::SelectionChanged);
    }

    fn stroke_params_from_presentation(presentation: &PresentationAttrs) -> Option<StrokeParams> {
        let width = presentation
            .stroke_width
            .unwrap_or(SvgLength::new(1.0, LengthUnit::None))
            .to_px();
        if width <= 0.0 {
            return None;
        }

        Some(StrokeParams {
            width,
            line_cap: match presentation.stroke_linecap.unwrap_or(SvgLineCap::Butt) {
                SvgLineCap::Butt => PathLineCap::Butt,
                SvgLineCap::Round => PathLineCap::Round,
                SvgLineCap::Square => PathLineCap::Square,
            },
            line_join: match presentation.stroke_linejoin.unwrap_or(SvgLineJoin::Miter) {
                SvgLineJoin::Miter => PathLineJoin::Miter,
                SvgLineJoin::Round => PathLineJoin::Round,
                SvgLineJoin::Bevel => PathLineJoin::Bevel,
            },
            miter_limit: presentation.stroke_miterlimit.unwrap_or(4.0),
            dash_pattern: presentation.stroke_dasharray.clone().unwrap_or_default(),
            dash_offset: presentation.stroke_dashoffset.unwrap_or(0.0),
        })
    }

    fn non_scaling_stroke(presentation: &PresentationAttrs) -> bool {
        presentation.vector_effect.as_deref().is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|token| token.eq_ignore_ascii_case("non-scaling-stroke"))
        })
    }

    fn transform_stroke_scale(transform: Option<Transform2D>) -> f64 {
        let Some(transform) = transform else {
            return 1.0;
        };
        if transform.is_identity() {
            return 1.0;
        }

        let [a, b, c, d, _, _] = transform.as_coeffs();
        let scale_x = a.hypot(b);
        let scale_y = c.hypot(d);
        let scale = if scale_x > f64::EPSILON && scale_y > f64::EPSILON {
            (scale_x + scale_y) * 0.5
        } else {
            scale_x.max(scale_y)
        };

        if scale.is_finite() && scale > f64::EPSILON {
            scale
        } else {
            1.0
        }
    }

    fn stroke_params_for_item(item: &PathSelectionItem) -> Option<StrokeParams> {
        let mut params = Self::stroke_params_from_presentation(&item.presentation)?;
        if !Self::non_scaling_stroke(&item.presentation) {
            let scale = Self::transform_stroke_scale(item.presentation.transform);
            params.width *= scale;
            for dash in &mut params.dash_pattern {
                *dash *= scale;
            }
            params.dash_offset *= scale;
        }

        if params.width <= 0.0 {
            return None;
        }
        Some(params)
    }

    fn stroke_to_path_effect_marker(presentation: &PresentationAttrs) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(marker) = &presentation.marker_start {
            parts.push(format!("marker-start={marker}"));
        }
        if let Some(marker) = &presentation.marker_mid {
            parts.push(format!("marker-mid={marker}"));
        }
        if let Some(marker) = &presentation.marker_end {
            parts.push(format!("marker-end={marker}"));
        }
        if let Some(vector_effect) = &presentation.vector_effect {
            parts.push(format!("vector-effect={vector_effect}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(format!("sivgo:stroke-to-path({})", parts.join(";")))
        }
    }

    fn text_to_path_geometry(
        attrs: &sivgo_dom::TextAttrs,
        presentation: &PresentationAttrs,
        text_engine: &mut TextEngine,
    ) -> Option<BezPath> {
        let text = attrs.text_content.trim_matches('\0');
        if text.is_empty() {
            return None;
        }

        let parent_font_size = 16.0_f64;
        let font_family = attrs
            .font_family
            .as_deref()
            .or(presentation.font_family.as_deref())
            .unwrap_or("sans-serif");
        let font_size = attrs
            .font_size
            .or(presentation.font_size)
            .map(|l| l.resolve_px(parent_font_size, parent_font_size))
            .unwrap_or(parent_font_size);
        let font_weight = match attrs
            .font_weight
            .or(presentation.font_weight)
            .unwrap_or(sivgo_dom::FontWeight::Normal)
        {
            sivgo_dom::FontWeight::Normal => 400,
            sivgo_dom::FontWeight::Bold => 700,
            sivgo_dom::FontWeight::Number(n) => n as u16,
        };
        let font_style = match attrs
            .font_style
            .or(presentation.font_style)
            .unwrap_or(sivgo_dom::FontStyle::Normal)
        {
            sivgo_dom::FontStyle::Normal => FontStyleKind::Normal,
            sivgo_dom::FontStyle::Italic | sivgo_dom::FontStyle::Oblique => FontStyleKind::Italic,
        };
        let anchor = match attrs
            .text_anchor
            .or(presentation.text_anchor)
            .unwrap_or(SvgTextAnchor::Start)
        {
            SvgTextAnchor::Start => TextEngineAnchor::Start,
            SvgTextAnchor::Middle => TextEngineAnchor::Middle,
            SvgTextAnchor::End => TextEngineAnchor::End,
        };
        let x = attrs.x.first().map_or(0.0, |l| l.to_px())
            + attrs.dx.first().map_or(0.0, |l| l.to_px());
        let y = attrs.y.first().map_or(0.0, |l| l.to_px())
            + attrs.dy.first().map_or(0.0, |l| l.to_px());
        let letter_spacing = presentation.letter_spacing.map_or(0.0, |l| l.to_px());
        let word_spacing = presentation.word_spacing.map_or(0.0, |l| l.to_px());
        let x_list: Vec<f64> = attrs.x.iter().map(|l| l.to_px()).collect();
        let y_list: Vec<f64> = attrs.y.iter().map(|l| l.to_px()).collect();
        let dx_list: Vec<f64> = attrs.dx.iter().map(|l| l.to_px()).collect();
        let dy_list: Vec<f64> = attrs.dy.iter().map(|l| l.to_px()).collect();

        if x_list.len() > 1 || y_list.len() > 1 || !dx_list.is_empty() || !dy_list.is_empty() {
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
                &x_list,
                &y_list,
                &dx_list,
                &dy_list,
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
        }
    }

    fn presentation_without_text_state(mut presentation: PresentationAttrs) -> PresentationAttrs {
        presentation.transform = None;
        presentation.font_family = None;
        presentation.font_size = None;
        presentation.font_weight = None;
        presentation.font_style = None;
        presentation.text_anchor = None;
        presentation.text_decoration = None;
        presentation.letter_spacing = None;
        presentation.word_spacing = None;
        presentation
    }

    fn boolean_selected_paths(&mut self, op: BooleanOp, description: &str) {
        let items = self.selected_convertible_paths();
        if items.len() < 2 {
            return;
        }

        let mut result = items[0].path.clone();
        for item in &items[1..] {
            result = boolean_op(&result, &item.path, op, 0.25);
            if result.is_empty() {
                return;
            }
        }

        self.replace_primary_path_and_remove_rest(description, items, result, None);
    }

    fn cut_primary_path_with_selected_knives(&mut self, description: &str) {
        let items = self.selected_convertible_paths();
        if items.len() < 2 {
            return;
        }

        let subject = &items[0];
        let knife_paths: Vec<BezPath> =
            items.iter().skip(1).map(|item| item.path.clone()).collect();
        let knife = combine_paths(&knife_paths);
        let fragments = cut(&subject.path, &knife, 0.25);
        if fragments.len() <= 1 {
            return;
        }

        let Some(parent_id) = self.document.parent(subject.id) else {
            return;
        };
        let insert_index = self
            .document
            .child_index(subject.id)
            .unwrap_or_else(|| self.document.children(parent_id).len().saturating_sub(1));
        let presentation = subject.baked_presentation.clone();
        let mut commands: Vec<Box<dyn Command>> = vec![Box::new(ModifyElementCommand::new(
            subject.id,
            subject.element.clone(),
            Self::path_element(fragments[0].clone()),
            subject.presentation.clone(),
            presentation.clone(),
        ))];

        for (offset, fragment) in fragments.into_iter().skip(1).enumerate() {
            commands.push(Box::new(AddNodeCommand::new(
                parent_id,
                Self::path_element(fragment),
                presentation.clone(),
                Some(insert_index + 1 + offset),
            )));
        }

        commands.extend(
            items
                .iter()
                .skip(1)
                .map(|item| Box::new(RemoveNodeCommand::new(item.id)) as Box<dyn Command>),
        );

        let primary_id = subject.id;
        self.execute_command_batch(description, commands);
        self.selection.select(primary_id);
        self.events.emit(EditorEvent::SelectionChanged);
    }

    fn modify_selected_path_data<F>(&mut self, description: &str, transform_path: F)
    where
        F: Fn(&sivgo_math::BezPath) -> sivgo_math::BezPath,
    {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for id in ids {
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            let SvgElement::Path(attrs) = &node.element else {
                continue;
            };
            let old_path = attrs.d.clone();
            let new_path = transform_path(&old_path);
            if new_path != old_path {
                commands.push(Box::new(ModifyPathCommand::new(id, old_path, new_path)));
            }
        }

        if commands.is_empty() {
            return;
        }

        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new(description, commands)));
        }
    }

    /// Simplify selected path elements with the given tolerance.
    pub fn simplify_selected_paths(&mut self, tolerance: f64) {
        let tolerance = tolerance.clamp(0.01, 10_000.0);
        self.modify_selected_path_data("Simplify paths", |path| simplify_path(path, tolerance));
    }

    /// Reverse selected path elements.
    pub fn reverse_selected_paths(&mut self) {
        self.modify_selected_path_data("Reverse paths", reverse_path);
    }

    /// Offset selected path elements. Positive values outset; negative values inset.
    pub fn offset_selected_paths(&mut self, distance: f64) {
        if distance.abs() < f64::EPSILON {
            return;
        }
        self.modify_selected_path_data("Offset paths", |path| offset_path(path, distance, 0.25));
    }

    fn offset_effect_marker(kind: &str, amount: f64, source: Option<&str>) -> String {
        match source {
            Some(source) => format!("sivgo:{kind}(amount={amount};source={source})"),
            None => format!("sivgo:{kind}(amount={amount})"),
        }
    }

    /// Format a parameter value the way the panel shows it: trimmed of
    /// trailing zeros so `5.0` reads as `5`.
    fn format_lpe_value(value: f64) -> String {
        let text = format!("{value}");
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        }
    }

    /// Every live path effect the engine can apply, with a display label.
    ///
    /// This is the single source of truth for the effect list. The panel used
    /// to keep its own shorter enum and silently drop any stack entry whose
    /// kind it did not recognise, which hid most of the effects from the user
    /// entirely and made them impossible to remove once applied.
    pub fn available_lpe_kinds() -> &'static [(&'static str, &'static str)] {
        &[
            ("bend_path", "Bend Path"),
            ("bspline", "B-Spline"),
            ("copy_rotate", "Copy Rotate"),
            ("dashed_stroke", "Dashed Stroke"),
            ("envelope", "Envelope Deformation"),
            ("extrude", "Extrude"),
            ("fillet", "Fillet / Chamfer"),
            ("gears", "Gears"),
            ("hatching", "Hatching"),
            ("interpolate", "Interpolate"),
            ("knot", "Knot"),
            ("lattice", "Lattice Deformation"),
            ("measure", "Measure Segments"),
            ("mirror", "Mirror Symmetry"),
            ("offset", "Offset"),
            ("pattern_along_path", "Pattern Along Path"),
            ("perspective_envelope", "Perspective Envelope"),
            ("power_stroke", "Power Stroke"),
            ("roughen", "Roughen"),
            ("simplify", "Simplify"),
            ("sketch", "Sketch"),
            ("slice", "Slice"),
            ("spiro", "Spiro Spline"),
            ("taper_stroke", "Taper Stroke"),
            ("tiling", "Tiling"),
            ("von_koch", "Von Koch Fractal"),
        ]
    }

    /// Display label for a canonical effect kind.
    pub fn lpe_kind_label(kind: &str) -> Option<&'static str> {
        let canonical = Self::normalize_lpe_kind(kind)?;
        Self::available_lpe_kinds()
            .iter()
            .find(|(key, _)| *key == canonical)
            .map(|(_, label)| *label)
    }

    /// Default parameters for a kind, as name/value string pairs.
    ///
    /// Mirrors what the engine will store, so a panel can show the same rows
    /// the effect will actually use.
    pub fn lpe_kind_default_params(kind: &str) -> Vec<(String, String)> {
        let Some(canonical) = Self::normalize_lpe_kind(kind) else {
            return Vec::new();
        };
        Self::default_lpe_params(canonical)
            .into_iter()
            .map(|param| (param.name, Self::format_lpe_value(param.value)))
            .collect()
    }

    pub(crate) fn normalize_lpe_kind(kind: &str) -> Option<&'static str> {
        let normalized = kind.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        match normalized.as_str() {
            "bend_path" | "bend" => Some("bend_path"),
            "pattern_along_path" | "pattern" => Some("pattern_along_path"),
            "stitch_sub_paths" | "stitch_subpaths" | "stitch" => Some("stitch_sub_paths"),
            "offset" | "dynamic_offset" | "linked_offset" => Some("offset"),
            "simplify" => Some("simplify"),
            "power_stroke" => Some("power_stroke"),
            "roughen" => Some("roughen"),
            "mirror" | "mirror_symmetry" => Some("mirror"),
            "boolean_operation" | "boolean" => Some("boolean_operation"),
            "measure" | "measure_segments" => Some("measure"),
            "bspline" | "b_spline" => Some("bspline"),
            "copy_rotate" | "rotate_copies" => Some("copy_rotate"),
            "dashed_stroke" | "dash" | "dashes" => Some("dashed_stroke"),
            "envelope" | "envelope_deformation" => Some("envelope"),
            "perspective_envelope" | "perspective" => Some("perspective_envelope"),
            "extrude" => Some("extrude"),
            "fillet" | "fillet_chamfer" | "chamfer" => Some("fillet"),
            "gears" | "gear" => Some("gears"),
            "hatching" | "hatch" => Some("hatching"),
            "knot" => Some("knot"),
            "sketch" => Some("sketch"),
            "slice" | "slice_lpe" => Some("slice"),
            "spiro" | "spiro_spline" => Some("spiro"),
            "taper_stroke" | "taper" => Some("taper_stroke"),
            "tiling" | "tile" => Some("tiling"),
            "von_koch" | "koch" | "fractal" => Some("von_koch"),
            "interpolate" => Some("interpolate"),
            "lattice" | "lattice_deformation" => Some("lattice"),
            _ => None,
        }
    }

    pub(crate) fn default_lpe_params(kind: &str) -> Vec<EngineLpeParam> {
        fn param(name: &str, value: f64) -> EngineLpeParam {
            EngineLpeParam {
                name: name.to_string(),
                value,
            }
        }

        match Self::normalize_lpe_kind(kind).unwrap_or(kind) {
            "bend_path" => vec![param("bend_amount", 1.0)],
            "pattern_along_path" => vec![
                param("spacing", 10.0),
                param("normal_offset", 0.0),
                param("tangential_offset", 0.0),
            ],
            "stitch_sub_paths" => vec![param("gap", 5.0)],
            "offset" => vec![param("offset", 5.0)],
            "simplify" => vec![param("threshold", 2.0)],
            "power_stroke" => vec![param("width", 5.0)],
            "roughen" => vec![
                param("max_displacement", 2.0),
                param("segment_length", 10.0),
                param("seed", 42.0),
            ],
            "mirror" => vec![
                param("axis_origin_x", 0.0),
                param("axis_origin_y", 0.0),
                param("axis_angle", 0.0),
            ],
            "boolean_operation" => vec![param("operation", 0.0)],
            "measure" => vec![param("font_size", 12.0), param("measure_offset", 10.0)],
            "bspline" => vec![param("degree", 3.0)],
            "copy_rotate" => vec![
                param("center_x", 0.0),
                param("center_y", 0.0),
                param("copies", 6.0),
                param("include_original", 1.0),
            ],
            "dashed_stroke" => vec![param("dash_length", 10.0), param("gap_length", 5.0)],
            // Envelope/perspective corners are expressed as per-corner offsets
            // from the path's bounding box; all-zero offsets = identity map.
            "envelope" | "perspective_envelope" => vec![
                param("tl_dx", 0.0),
                param("tl_dy", 0.0),
                param("tr_dx", 0.0),
                param("tr_dy", 0.0),
                param("br_dx", 0.0),
                param("br_dy", 0.0),
                param("bl_dx", 0.0),
                param("bl_dy", 0.0),
            ],
            "extrude" => vec![param("offset_x", 10.0), param("offset_y", 10.0)],
            "fillet" => vec![param("radius", 10.0), param("mode", 0.0)],
            "gears" => vec![
                param("teeth", 12.0),
                param("module", 5.0),
                param("pressure_angle", 20.0),
            ],
            "hatching" => vec![
                param("angle", 45.0),
                param("spacing", 5.0),
                param("line_width", 0.5),
            ],
            "knot" => vec![param("gap_width", 5.0)],
            "sketch" => vec![
                param("lines", 3.0),
                param("max_displacement", 2.0),
                param("seed", 42.0),
            ],
            "slice" => vec![
                param("slice_x", 0.0),
                param("slice_y", 0.0),
                param("slice_angle", 0.0),
            ],
            "spiro" => Vec::new(),
            "taper_stroke" => vec![
                param("start_width", 5.0),
                param("end_width", 1.0),
                param("taper_length", 0.25),
            ],
            "tiling" => vec![
                param("rows", 2.0),
                param("cols", 2.0),
                param("spacing_x", 10.0),
                param("spacing_y", 10.0),
            ],
            "von_koch" => vec![param("iterations", 3.0), param("scale", 1.0)],
            "interpolate" => vec![param("steps", 2.0)],
            "lattice" => vec![param("rows", 4.0), param("cols", 4.0)],
            _ => Vec::new(),
        }
    }

    fn lpe_param_value(entry: &EngineLpeEntry, name: &str, fallback: f64) -> f64 {
        entry
            .params
            .iter()
            .find(|param| param.name == name)
            .map(|param| param.value)
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
    }

    fn encode_lpe_stack(entries: &[EngineLpeEntry]) -> Option<String> {
        if entries.is_empty() {
            return None;
        }

        let encoded = entries
            .iter()
            .map(|entry| {
                let mut fields = vec![
                    format!("id={}", entry.id),
                    format!("kind={}", entry.kind),
                    format!("enabled={}", if entry.enabled { 1 } else { 0 }),
                ];
                for param in &entry.params {
                    fields.push(format!("{}={}", param.name, param.value));
                }
                fields.join(",")
            })
            .collect::<Vec<_>>()
            .join("|");
        Some(format!("sivgo:lpe-stack:{encoded}"))
    }

    pub(crate) fn decode_lpe_stack(marker: Option<&str>) -> Vec<EngineLpeEntry> {
        let Some(marker) = marker else {
            return Vec::new();
        };
        let Some(encoded) = marker.strip_prefix("sivgo:lpe-stack:") else {
            return Vec::new();
        };

        encoded
            .split('|')
            .filter_map(|entry_text| {
                let mut id = None;
                let mut kind = None;
                let mut enabled = true;
                let mut values: HashMap<String, f64> = HashMap::new();

                for field in entry_text.split(',') {
                    let Some((name, value)) = field.split_once('=') else {
                        continue;
                    };
                    match name {
                        "id" => id = Some(value.to_string()),
                        "kind" => kind = Self::normalize_lpe_kind(value).map(str::to_string),
                        "enabled" => enabled = value != "0" && !value.eq_ignore_ascii_case("false"),
                        _ => {
                            if let Ok(number) = value.parse::<f64>() {
                                if number.is_finite() {
                                    values.insert(name.to_string(), number);
                                }
                            }
                        }
                    }
                }

                let kind = kind?;
                let mut params = Self::default_lpe_params(&kind);
                for param in &mut params {
                    if let Some(value) = values.remove(&param.name) {
                        param.value = value;
                    }
                }
                for (name, value) in values {
                    params.push(EngineLpeParam { name, value });
                }

                Some(EngineLpeEntry {
                    id: id.unwrap_or_else(|| "lpe1".to_string()),
                    kind,
                    enabled,
                    params,
                })
            })
            .collect()
    }

    pub(crate) fn next_lpe_id(entries: &[EngineLpeEntry]) -> String {
        let next = entries
            .iter()
            .filter_map(|entry| entry.id.strip_prefix("lpe"))
            .filter_map(|suffix| suffix.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        format!("lpe{next}")
    }

    fn apply_lpe_entry(input: &BezPath, entry: &EngineLpeEntry) -> BezPath {
        let effect: Option<Box<dyn LivePathEffect>> = match entry.kind.as_str() {
            "offset" => Some(Box::new(OffsetLpe::new(
                Self::lpe_param_value(entry, "offset", 5.0),
                OffsetJoin::Miter,
            ))),
            "simplify" => Some(Box::new(SimplifyLpe::new(Self::lpe_param_value(
                entry,
                "threshold",
                2.0,
            )))),
            "power_stroke" => {
                let width = Self::lpe_param_value(entry, "width", 5.0).max(0.0);
                Some(Box::new(PowerStrokeLpe::new(vec![
                    (0.0, width),
                    (1.0, width),
                ])))
            }
            "roughen" => {
                let seed = Self::lpe_param_value(entry, "seed", 42.0).max(0.0) as u64;
                Some(Box::new(RoughenLpe::new(
                    Self::lpe_param_value(entry, "max_displacement", 2.0),
                    Self::lpe_param_value(entry, "segment_length", 10.0),
                    seed,
                )))
            }
            "mirror" => Some(Box::new(MirrorSymmetryLpe::new(
                Point::new(
                    Self::lpe_param_value(entry, "axis_origin_x", 0.0),
                    Self::lpe_param_value(entry, "axis_origin_y", 0.0),
                ),
                Self::lpe_param_value(entry, "axis_angle", 0.0),
            ))),
            "measure" => {
                let mut lpe =
                    MeasureSegmentsLpe::new("px", Self::lpe_param_value(entry, "font_size", 12.0));
                lpe.set_param(
                    "measure_offset",
                    Self::lpe_param_value(entry, "measure_offset", 10.0),
                );
                Some(Box::new(lpe))
            }
            "bspline" => {
                let degree = Self::lpe_param_value(entry, "degree", 3.0).max(1.0) as u32;
                Some(Box::new(BSplineLpe::new(degree)))
            }
            "copy_rotate" => {
                let center = Point::new(
                    Self::lpe_param_value(entry, "center_x", 0.0),
                    Self::lpe_param_value(entry, "center_y", 0.0),
                );
                let copies = Self::lpe_param_value(entry, "copies", 6.0).max(1.0) as usize;
                let include_original = Self::lpe_param_value(entry, "include_original", 1.0) >= 0.5;
                Some(Box::new(CopyRotateLpe::new(
                    center,
                    copies,
                    include_original,
                )))
            }
            "dashed_stroke" => Some(Box::new(DashedStrokeLpe::new(
                Self::lpe_param_value(entry, "dash_length", 10.0).max(0.0),
                Self::lpe_param_value(entry, "gap_length", 5.0).max(0.0),
            ))),
            "envelope" | "perspective_envelope" => {
                // Build the four corners from the input bounding box plus the
                // stored per-corner offsets (zero offsets => identity map).
                let bb = input.bounding_box();
                let tl = Point::new(
                    bb.x0 + Self::lpe_param_value(entry, "tl_dx", 0.0),
                    bb.y0 + Self::lpe_param_value(entry, "tl_dy", 0.0),
                );
                let tr = Point::new(
                    bb.x1 + Self::lpe_param_value(entry, "tr_dx", 0.0),
                    bb.y0 + Self::lpe_param_value(entry, "tr_dy", 0.0),
                );
                let br = Point::new(
                    bb.x1 + Self::lpe_param_value(entry, "br_dx", 0.0),
                    bb.y1 + Self::lpe_param_value(entry, "br_dy", 0.0),
                );
                let bl = Point::new(
                    bb.x0 + Self::lpe_param_value(entry, "bl_dx", 0.0),
                    bb.y1 + Self::lpe_param_value(entry, "bl_dy", 0.0),
                );
                if entry.kind == "perspective_envelope" {
                    Some(Box::new(PerspectiveEnvelopeLpe::new(tl, tr, br, bl)))
                } else {
                    // EnvelopeLpe::new(top_left, top_right, bottom_left, bottom_right)
                    Some(Box::new(EnvelopeLpe::new(tl, tr, bl, br)))
                }
            }
            "extrude" => Some(Box::new(ExtrudeLpe::new(
                Self::lpe_param_value(entry, "offset_x", 10.0),
                Self::lpe_param_value(entry, "offset_y", 10.0),
            ))),
            "fillet" => {
                let radius = Self::lpe_param_value(entry, "radius", 10.0).max(0.0);
                let mode = if Self::lpe_param_value(entry, "mode", 0.0) >= 0.5 {
                    FilletMode::Chamfer
                } else {
                    FilletMode::Fillet
                };
                Some(Box::new(FilletChamferLpe::new(radius, mode)))
            }
            "gears" => {
                let teeth = Self::lpe_param_value(entry, "teeth", 12.0).max(3.0) as usize;
                Some(Box::new(GearsLpe::new(
                    teeth,
                    Self::lpe_param_value(entry, "module", 5.0).max(0.1),
                    Self::lpe_param_value(entry, "pressure_angle", 20.0),
                )))
            }
            "hatching" => Some(Box::new(HatchingLpe::new(
                Self::lpe_param_value(entry, "angle", 45.0),
                Self::lpe_param_value(entry, "spacing", 5.0).max(0.1),
                Self::lpe_param_value(entry, "line_width", 0.5).max(0.0),
            ))),
            "knot" => Some(Box::new(KnotLpe::new(
                Self::lpe_param_value(entry, "gap_width", 5.0).max(0.0),
            ))),
            "sketch" => {
                let lines = Self::lpe_param_value(entry, "lines", 3.0).max(1.0) as u32;
                let seed = Self::lpe_param_value(entry, "seed", 42.0).max(0.0) as u64;
                Some(Box::new(SketchLpe::new(
                    lines,
                    Self::lpe_param_value(entry, "max_displacement", 2.0),
                    seed,
                )))
            }
            "slice" => Some(Box::new(SliceLpe::new(
                Self::lpe_param_value(entry, "slice_x", 0.0),
                Self::lpe_param_value(entry, "slice_y", 0.0),
                Self::lpe_param_value(entry, "slice_angle", 0.0),
            ))),
            "spiro" => Some(Box::new(SpiroLpe::new())),
            "taper_stroke" => Some(Box::new(TaperStrokeLpe::new(
                Self::lpe_param_value(entry, "start_width", 5.0).max(0.0),
                Self::lpe_param_value(entry, "end_width", 1.0).max(0.0),
                Self::lpe_param_value(entry, "taper_length", 0.25),
            ))),
            "tiling" => {
                let rows = Self::lpe_param_value(entry, "rows", 2.0).max(1.0) as usize;
                let cols = Self::lpe_param_value(entry, "cols", 2.0).max(1.0) as usize;
                Some(Box::new(TilingLpe::new(
                    rows,
                    cols,
                    Self::lpe_param_value(entry, "spacing_x", 10.0),
                    Self::lpe_param_value(entry, "spacing_y", 10.0),
                )))
            }
            "von_koch" => {
                let iterations =
                    Self::lpe_param_value(entry, "iterations", 3.0).clamp(0.0, 8.0) as u32;
                Some(Box::new(VonKochLpe::new(
                    iterations,
                    Self::lpe_param_value(entry, "scale", 1.0),
                )))
            }
            "bend_path" => {
                // Default skeleton: a straight horizontal line across the input
                // bounding box (identity baseline; a custom skeleton can be set
                // once geometry parameters are supported).
                let bb = input.bounding_box();
                let cy = (bb.y0 + bb.y1) * 0.5;
                let mut skeleton = BezPath::new();
                skeleton.move_to(Point::new(bb.x0, cy));
                skeleton.line_to(Point::new(bb.x1, cy));
                Some(Box::new(BendPathLpe::new(skeleton)))
            }
            "pattern_along_path" => {
                // The input path acts as the skeleton; stamp a short default
                // unit-segment pattern along it. spacing/offsets come from the
                // (flat f64) params, so this effect is genuinely tunable.
                let spacing = Self::lpe_param_value(entry, "spacing", 10.0).max(1.0);
                let normal = Self::lpe_param_value(entry, "normal_offset", 0.0);
                let tangent = Self::lpe_param_value(entry, "tangential_offset", 0.0);
                let mut pattern = BezPath::new();
                pattern.move_to(Point::new(0.0, 0.0));
                pattern.line_to(Point::new(spacing * 0.5, 0.0));
                Some(Box::new(PatternAlongPathLpe::new(
                    pattern, 0, spacing, normal, tangent, false,
                )))
            }
            "interpolate" => {
                let steps = Self::lpe_param_value(entry, "steps", 2.0).max(1.0) as u32;
                // Default reference = the input itself => identity interpolation
                // until a distinct target path is provided.
                Some(Box::new(InterpolateLpe::new(steps, input.clone())))
            }
            "lattice" => {
                let rows = Self::lpe_param_value(entry, "rows", 4.0).max(1.0) as usize;
                let cols = Self::lpe_param_value(entry, "cols", 4.0).max(1.0) as usize;
                Some(Box::new(LatticeLpe::identity(rows, cols)))
            }
            // These remaining effects need multi-object context (other paths in
            // the document) that the single-path LPE pipeline does not yet
            // provide; they are persisted/editable but preview as the original.
            "stitch_sub_paths" | "boolean_operation" => None,
            _ => None,
        };

        effect.map_or_else(|| input.clone(), |effect| effect.apply(input))
    }

    fn apply_lpe_stack(original: &BezPath, entries: &[EngineLpeEntry]) -> BezPath {
        let mut current = original.clone();
        for entry in entries {
            if entry.enabled {
                current = Self::apply_lpe_entry(&current, entry);
            }
        }
        current
    }

    pub(crate) fn rebuild_lpe_path_attrs(
        attrs: &PathAttrs,
        entries: &[EngineLpeEntry],
    ) -> PathAttrs {
        let original = attrs
            .inkscape_original_d
            .clone()
            .unwrap_or_else(|| attrs.d.clone());
        if entries.is_empty() {
            return PathAttrs {
                d: original,
                inkscape_path_effect: None,
                inkscape_original_d: None,
            };
        }

        PathAttrs {
            d: Self::apply_lpe_stack(&original, entries),
            inkscape_path_effect: Self::encode_lpe_stack(entries),
            inkscape_original_d: Some(original),
        }
    }

    fn modify_primary_lpe_stack<F>(&mut self, _description: &str, update: F) -> bool
    where
        F: FnOnce(&mut Vec<EngineLpeEntry>),
    {
        let Some(id) = self.selection.primary() else {
            return false;
        };
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        let SvgElement::Path(attrs) = &node.element else {
            return false;
        };

        let mut entries = Self::decode_lpe_stack(attrs.inkscape_path_effect.as_deref());
        update(&mut entries);
        let new_attrs = Self::rebuild_lpe_path_attrs(attrs, &entries);
        let old_element = node.element.clone();
        let old_presentation = node.presentation.clone();
        self.execute(Box::new(ModifyElementCommand::new(
            id,
            old_element,
            SvgElement::Path(new_attrs),
            old_presentation.clone(),
            old_presentation,
        )));
        self.events.emit(EditorEvent::SelectionChanged);
        true
    }

    /// Return the editable LPE stack for the primary selected path.
    pub fn selected_lpe_entries(&self) -> Vec<EngineLpeEntry> {
        let Some(id) = self.selection.primary() else {
            return Vec::new();
        };
        let Some(node) = self.document.get_node(id) else {
            return Vec::new();
        };
        let SvgElement::Path(attrs) = &node.element else {
            return Vec::new();
        };
        Self::decode_lpe_stack(attrs.inkscape_path_effect.as_deref())
    }

    /// Add an LPE to the primary selected path and return its generated id.
    pub fn add_lpe_to_selected_path(&mut self, kind: &str) -> Option<String> {
        let kind = Self::normalize_lpe_kind(kind)?.to_string();
        let mut added_id = None;
        let updated = self.modify_primary_lpe_stack("Add LPE", |entries| {
            let id = Self::next_lpe_id(entries);
            let params = Self::default_lpe_params(kind.as_str());
            added_id = Some(id.clone());
            entries.push(EngineLpeEntry {
                id,
                kind: kind.clone(),
                enabled: true,
                params,
            });
        });
        updated.then_some(added_id).flatten()
    }

    /// Remove an LPE from the primary selected path.
    pub fn remove_lpe_from_selected_path(&mut self, effect_id: &str) -> bool {
        self.modify_primary_lpe_stack("Remove LPE", |entries| {
            entries.retain(|entry| entry.id != effect_id);
        })
    }

    /// Toggle one LPE entry on the primary selected path.
    pub fn toggle_lpe_on_selected_path(&mut self, effect_id: &str) -> bool {
        self.modify_primary_lpe_stack("Toggle LPE", |entries| {
            if let Some(entry) = entries.iter_mut().find(|entry| entry.id == effect_id) {
                entry.enabled = !entry.enabled;
            }
        })
    }

    /// Move one LPE entry up or down in the stack on the primary selected path.
    pub fn move_lpe_on_selected_path(&mut self, effect_id: &str, delta: isize) -> bool {
        self.modify_primary_lpe_stack("Move LPE", |entries| {
            let Some(index) = entries.iter().position(|entry| entry.id == effect_id) else {
                return;
            };
            let target = if delta < 0 {
                index.saturating_sub(1)
            } else {
                (index + 1).min(entries.len().saturating_sub(1))
            };
            if target != index {
                let entry = entries.remove(index);
                entries.insert(target, entry);
            }
        })
    }

    /// Set a numeric LPE parameter on the primary selected path.
    pub fn set_lpe_param_on_selected_path(
        &mut self,
        effect_id: &str,
        param_name: &str,
        value: f64,
    ) -> bool {
        if !value.is_finite() {
            return false;
        }

        self.modify_primary_lpe_stack("Set LPE parameter", |entries| {
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == effect_id) else {
                return;
            };
            if let Some(param) = entry
                .params
                .iter_mut()
                .find(|param| param.name == param_name)
            {
                param.value = value;
            } else {
                entry.params.push(EngineLpeParam {
                    name: param_name.to_string(),
                    value,
                });
            }
        })
    }

    /// Create dynamic offset paths in place, preserving the original path in
    /// `inkscape:original-d` so later UI can expose interactive adjustment.
    pub fn dynamic_offset_selected_paths(&mut self, distance: f64) {
        if distance.abs() < f64::EPSILON {
            return;
        }

        let items = self.selected_convertible_paths();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        let mut selected_ids = Vec::new();

        for item in items {
            let dynamic = DynamicOffset::new(item.path.clone(), distance);
            let offset_path = dynamic.compute();
            if offset_path.is_empty() {
                continue;
            }

            commands.push(Box::new(ModifyElementCommand::new(
                item.id,
                item.element.clone(),
                SvgElement::Path(PathAttrs {
                    d: offset_path,
                    inkscape_path_effect: Some(Self::offset_effect_marker(
                        "dynamic-offset",
                        dynamic.offset_amount(),
                        None,
                    )),
                    inkscape_original_d: Some(item.path),
                }),
                item.presentation,
                item.baked_presentation,
            )));
            selected_ids.push(item.id);
        }

        if commands.is_empty() {
            return;
        }

        self.execute_command_batch("Dynamic offset", commands);
        self.selection.clear();
        for id in selected_ids {
            self.selection.add(id);
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Create linked offset paths next to the selected source paths. The source
    /// object is left intact and the new path stores a source marker string.
    pub fn linked_offset_selected_paths(&mut self, distance: f64) {
        if distance.abs() < f64::EPSILON {
            return;
        }

        let items = self.selected_convertible_paths();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        let mut source_contexts = Vec::new();

        for item in items {
            let Some(parent_id) = self.document.parent(item.id) else {
                continue;
            };
            let insert_index = self.document.child_index(item.id).map(|index| index + 1);
            let source_marker = self
                .document
                .get_node(item.id)
                .and_then(|node| node.xml_id.clone())
                .unwrap_or_else(|| format!("node:{}", item.id.0));
            let linked = LinkedOffset::new(source_marker.clone(), distance);
            let offset_path = linked.compute(&item.path);
            if offset_path.is_empty() {
                continue;
            }

            commands.push(Box::new(AddNodeCommand::new(
                parent_id,
                SvgElement::Path(PathAttrs {
                    d: offset_path,
                    inkscape_path_effect: Some(Self::offset_effect_marker(
                        "linked-offset",
                        linked.offset_amount(),
                        Some(linked.source_id()),
                    )),
                    inkscape_original_d: Some(item.path),
                }),
                item.baked_presentation,
                insert_index,
            )));
            source_contexts.push((parent_id, item.id));
        }

        if commands.is_empty() {
            return;
        }

        self.execute_command_batch("Linked offset", commands);
        self.selection.clear();
        for (parent_id, source_id) in source_contexts {
            if let Some(index) = self.document.child_index(source_id) {
                if let Some(&linked_id) = self.document.children(parent_id).get(index + 1) {
                    self.selection.add(linked_id);
                }
            }
        }
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Convert selected basic shape elements to `<path>` elements in place.
    pub fn object_to_path_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for id in ids {
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            if matches!(node.element, SvgElement::Path(_)) {
                continue;
            }
            let Some(path) = element_to_path(&node.element) else {
                continue;
            };
            let mut new_presentation = node.presentation.clone();
            let path = if let Some(transform) = new_presentation.transform {
                new_presentation.transform = None;
                if transform.is_identity() {
                    path
                } else {
                    path.transformed(&transform)
                }
            } else {
                path
            };

            commands.push(Box::new(ModifyElementCommand::new(
                id,
                node.element.clone(),
                Self::path_element(path),
                node.presentation.clone(),
                new_presentation,
            )));
        }

        self.execute_command_batch("Object to path", commands);
    }

    /// Convert selected stroked shapes to filled outline paths.
    pub fn stroke_to_path_selected(&mut self) {
        let items = self.selected_convertible_paths();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for item in items {
            let stroke = match &item.presentation.stroke {
                Some(Paint::None) | None => continue,
                Some(stroke) => stroke.clone(),
            };
            let Some(params) = Self::stroke_params_for_item(&item) else {
                continue;
            };

            let outline = stroke_to_path(&item.path, &params, 0.25);
            if outline.is_empty() {
                continue;
            }

            let effect_marker = Self::stroke_to_path_effect_marker(&item.presentation);
            let original_d = effect_marker.as_ref().map(|_| item.path.clone());
            let mut new_presentation = item.baked_presentation.clone();
            new_presentation.fill = Some(stroke);
            new_presentation.fill_opacity = item.presentation.stroke_opacity;
            new_presentation.stroke = Some(Paint::None);
            new_presentation.stroke_width = None;
            new_presentation.stroke_opacity = None;
            new_presentation.stroke_linecap = None;
            new_presentation.stroke_linejoin = None;
            new_presentation.stroke_miterlimit = None;
            new_presentation.stroke_dasharray = None;
            new_presentation.stroke_dashoffset = None;
            new_presentation.marker_start = None;
            new_presentation.marker_mid = None;
            new_presentation.marker_end = None;
            new_presentation.vector_effect = None;

            commands.push(Box::new(ModifyElementCommand::new(
                item.id,
                item.element,
                SvgElement::Path(PathAttrs {
                    d: outline,
                    inkscape_path_effect: effect_marker,
                    inkscape_original_d: original_d,
                }),
                item.presentation,
                new_presentation,
            )));
        }

        self.execute_command_batch("Stroke to path", commands);
    }

    /// Convert selected SVG text objects to filled path outlines.
    pub fn text_to_path_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        let mut text_engine = TextEngine::new();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for id in ids {
            let Some(node) = self.document.get_node(id) else {
                continue;
            };
            let SvgElement::Text(attrs) = &node.element else {
                continue;
            };
            let Some(mut path) =
                Self::text_to_path_geometry(attrs, &node.presentation, &mut text_engine)
            else {
                continue;
            };
            let new_presentation = Self::presentation_without_text_state(node.presentation.clone());
            if let Some(transform) = node.presentation.transform {
                if !transform.is_identity() {
                    path = path.transformed(&transform);
                }
            }

            commands.push(Box::new(ModifyElementCommand::new(
                id,
                node.element.clone(),
                Self::path_element(path),
                node.presentation.clone(),
                new_presentation,
            )));
        }

        self.execute_command_batch("Text to path", commands);
    }

    /// Combine selected path-convertible elements into the primary selected node.
    pub fn combine_selected_paths(&mut self) {
        let items = self.selected_convertible_paths();
        if items.len() < 2 {
            return;
        }

        let paths: Vec<BezPath> = items.iter().map(|item| item.path.clone()).collect();
        let combined = combine_paths(&paths);
        self.replace_primary_path_and_remove_rest("Combine paths", items, combined, None);
    }

    /// Split selected compound paths into one path element per subpath.
    pub fn break_apart_selected_paths(&mut self) {
        let items = self.selected_convertible_paths();
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for item in items {
            let parts = break_apart(&item.path);
            if parts.len() <= 1 {
                continue;
            }

            let Some(parent_id) = self.document.parent(item.id) else {
                continue;
            };
            let insert_index = self
                .document
                .child_index(item.id)
                .unwrap_or_else(|| self.document.children(parent_id).len().saturating_sub(1));
            let presentation = item.baked_presentation.clone();

            commands.push(Box::new(ModifyElementCommand::new(
                item.id,
                item.element.clone(),
                Self::path_element(parts[0].clone()),
                item.presentation.clone(),
                presentation.clone(),
            )));

            for (offset, part) in parts.into_iter().skip(1).enumerate() {
                commands.push(Box::new(AddNodeCommand::new(
                    parent_id,
                    Self::path_element(part),
                    presentation.clone(),
                    Some(insert_index + 1 + offset),
                )));
            }
        }

        self.execute_command_batch("Break apart paths", commands);
    }

    /// Union selected path-convertible elements into the primary selected node.
    pub fn union_selected_paths(&mut self) {
        self.boolean_selected_paths(BooleanOp::Union, "Union paths");
    }

    /// Subtract subsequent selected paths from the primary selected node.
    pub fn difference_selected_paths(&mut self) {
        self.boolean_selected_paths(BooleanOp::Difference, "Difference paths");
    }

    /// Intersect selected path-convertible elements into the primary selected node.
    pub fn intersection_selected_paths(&mut self) {
        self.boolean_selected_paths(BooleanOp::Intersection, "Intersect paths");
    }

    /// Exclude overlapping areas of selected path-convertible elements.
    pub fn exclusion_selected_paths(&mut self) {
        self.boolean_selected_paths(BooleanOp::Xor, "Exclude paths");
    }

    /// Divide the primary selected path with subsequent selected paths as knives.
    pub fn division_selected_paths(&mut self) {
        self.cut_primary_path_with_selected_knives("Divide paths");
    }

    /// Cut the primary selected path with subsequent selected paths as knives.
    pub fn cut_selected_paths(&mut self) {
        self.cut_primary_path_with_selected_knives("Cut paths");
    }

    // -----------------------------------------------------------------------
    // Align & Distribute (9.6.6)
    // -----------------------------------------------------------------------

    /// Align selected elements by the given edge/center.
    pub fn align_selected(&mut self, alignment: Alignment, relative_to: AlignRelativeTo) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.len() < 2 && relative_to == AlignRelativeTo::Selection {
            return;
        }

        // Compute bounding boxes for each selected element.
        let bboxes: Vec<(NodeId, Rect)> = ids
            .iter()
            .filter_map(|&id| {
                let node = self.document.get_node(id)?;
                let bb = element_bounding_box(&node.element)?;
                Some((id, bb))
            })
            .collect();

        if bboxes.is_empty() {
            return;
        }

        // Determine the reference line.
        let ref_rect = match relative_to {
            AlignRelativeTo::Selection => {
                let mut r = bboxes[0].1;
                for (_, bb) in &bboxes[1..] {
                    r = r.union(*bb);
                }
                r
            }
            AlignRelativeTo::Page => {
                // Use document page dimensions.
                let (w, h) = self.get_page_size();
                Rect::new(0.0, 0.0, w, h)
            }
        };

        let target = match alignment {
            Alignment::Left => ref_rect.x0,
            Alignment::CenterH => (ref_rect.x0 + ref_rect.x1) / 2.0,
            Alignment::Right => ref_rect.x1,
            Alignment::Top => ref_rect.y0,
            Alignment::CenterV => (ref_rect.y0 + ref_rect.y1) / 2.0,
            Alignment::Bottom => ref_rect.y1,
        };

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for (id, bb) in &bboxes {
            let shift = match alignment {
                Alignment::Left => Point::new(target - bb.x0, 0.0),
                Alignment::CenterH => Point::new(target - (bb.x0 + bb.x1) / 2.0, 0.0),
                Alignment::Right => Point::new(target - bb.x1, 0.0),
                Alignment::Top => Point::new(0.0, target - bb.y0),
                Alignment::CenterV => Point::new(0.0, target - (bb.y0 + bb.y1) / 2.0),
                Alignment::Bottom => Point::new(0.0, target - bb.y1),
            };

            if shift.x.abs() < 1e-10 && shift.y.abs() < 1e-10 {
                continue;
            }

            let old_t = self
                .document
                .get_node(*id)
                .and_then(|n| n.presentation.transform);
            let offset = sivgo_math::Transform2D::translate(shift.x, shift.y);
            let new_t = Some(
                old_t
                    .unwrap_or(sivgo_math::Transform2D::IDENTITY)
                    .then(&offset),
            );

            commands.push(Box::new(crate::commands::TransformCommand::new(
                *id, old_t, new_t,
            )));
        }

        if !commands.is_empty() {
            self.execute(Box::new(CompoundCommand::new("Align", commands)));
        }
    }

    /// Distribute selected elements evenly along an axis.
    pub fn distribute_selected(&mut self, axis: DistributeAxis) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.len() < 3 {
            return;
        }

        let mut items: Vec<(NodeId, Rect)> = ids
            .iter()
            .filter_map(|&id| {
                let node = self.document.get_node(id)?;
                let bb = element_bounding_box(&node.element)?;
                Some((id, bb))
            })
            .collect();

        if items.len() < 3 {
            return;
        }

        // Sort by center position on the relevant axis.
        match axis {
            DistributeAxis::Horizontal => {
                items.sort_by(|a, b| {
                    let ca = (a.1.x0 + a.1.x1) / 2.0;
                    let cb = (b.1.x0 + b.1.x1) / 2.0;
                    ca.partial_cmp(&cb).unwrap()
                });
            }
            DistributeAxis::Vertical => {
                items.sort_by(|a, b| {
                    let ca = (a.1.y0 + a.1.y1) / 2.0;
                    let cb = (b.1.y0 + b.1.y1) / 2.0;
                    ca.partial_cmp(&cb).unwrap()
                });
            }
        }

        let n = items.len();
        let (first_center, last_center) = match axis {
            DistributeAxis::Horizontal => (
                (items[0].1.x0 + items[0].1.x1) / 2.0,
                (items[n - 1].1.x0 + items[n - 1].1.x1) / 2.0,
            ),
            DistributeAxis::Vertical => (
                (items[0].1.y0 + items[0].1.y1) / 2.0,
                (items[n - 1].1.y0 + items[n - 1].1.y1) / 2.0,
            ),
        };

        let step = (last_center - first_center) / (n - 1) as f64;

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for (i, (id, bb)) in items.iter().enumerate() {
            let target_center = first_center + step * i as f64;
            let current_center = match axis {
                DistributeAxis::Horizontal => (bb.x0 + bb.x1) / 2.0,
                DistributeAxis::Vertical => (bb.y0 + bb.y1) / 2.0,
            };
            let shift = target_center - current_center;
            if shift.abs() < 1e-10 {
                continue;
            }

            let old_t = self
                .document
                .get_node(*id)
                .and_then(|n| n.presentation.transform);
            let offset = match axis {
                DistributeAxis::Horizontal => sivgo_math::Transform2D::translate(shift, 0.0),
                DistributeAxis::Vertical => sivgo_math::Transform2D::translate(0.0, shift),
            };
            let new_t = Some(
                old_t
                    .unwrap_or(sivgo_math::Transform2D::IDENTITY)
                    .then(&offset),
            );
            commands.push(Box::new(crate::commands::TransformCommand::new(
                *id, old_t, new_t,
            )));
        }

        if !commands.is_empty() {
            self.execute(Box::new(CompoundCommand::new("Distribute", commands)));
        }
    }

    // -----------------------------------------------------------------------
    // Transform operations (9.6.7)
    // -----------------------------------------------------------------------

    /// Decompose the primary selected object's affine transform.
    pub fn primary_transform_decomposition(&self) -> Option<TransformDecomposition> {
        let id = self.selection.primary()?;
        let transform = self.document.get_node(id)?.presentation.transform;
        Some(Self::decompose_transform(transform))
    }

    /// Decompose an affine transform into editor-friendly values.
    pub fn decompose_transform(transform: Option<Transform2D>) -> TransformDecomposition {
        let transform = transform.unwrap_or(Transform2D::IDENTITY);
        let [a, b, c, d, e, f] = transform.as_coeffs();
        let scale_x = (a * a + b * b).sqrt();
        let det = a * d - b * c;
        let scale_y = if scale_x > 1e-12 {
            det / scale_x
        } else {
            (c * c + d * d).sqrt()
        };
        let rotation_degrees = b.atan2(a).to_degrees();
        let shear = if scale_x > 1e-12 {
            (a * c + b * d) / (scale_x * scale_x)
        } else {
            0.0
        };
        let skew_x_degrees = shear.atan().to_degrees();

        TransformDecomposition {
            translate_x: e,
            translate_y: f,
            scale_x,
            scale_y,
            rotation_degrees,
            skew_x_degrees,
        }
    }

    /// Flip selected elements horizontally.
    pub fn flip_horizontal(&mut self) {
        self.apply_transform_to_selected("Flip Horizontal", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            // `then` applies the receiver first: move the centre to the
            // origin, mirror there, then move it back.
            sivgo_math::Transform2D::translate(-cx, 0.0)
                .then(&sivgo_math::Transform2D::scale_xy(-1.0, 1.0))
                .then(&sivgo_math::Transform2D::translate(cx, 0.0))
        });
    }

    /// Flip selected elements vertically.
    pub fn flip_vertical(&mut self) {
        self.apply_transform_to_selected("Flip Vertical", |bb| {
            let cy = (bb.y0 + bb.y1) / 2.0;
            sivgo_math::Transform2D::translate(0.0, -cy)
                .then(&sivgo_math::Transform2D::scale_xy(1.0, -1.0))
                .then(&sivgo_math::Transform2D::translate(0.0, cy))
        });
    }

    /// Rotate selected elements by the given degrees around their combined center.
    pub fn rotate_selected(&mut self, degrees: f64) {
        self.apply_transform_to_selected("Rotate", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            let cy = (bb.y0 + bb.y1) / 2.0;
            let radians = degrees.to_radians();
            sivgo_math::Transform2D::rotate_about(radians, sivgo_math::Point::new(cx, cy))
        });
    }

    /// Set selected elements to an absolute rotation based on the primary object.
    pub fn set_selected_rotation(&mut self, degrees: f64) {
        let current = self
            .primary_transform_decomposition()
            .map(|value| value.rotation_degrees)
            .unwrap_or(0.0);
        self.rotate_selected(degrees - current);
    }

    /// Scale selected elements by the given factor around their combined center.
    pub fn scale_selected(&mut self, sx: f64, sy: f64) {
        self.apply_transform_to_selected("Scale", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            let cy = (bb.y0 + bb.y1) / 2.0;
            sivgo_math::Transform2D::translate(-cx, -cy)
                .then(&sivgo_math::Transform2D::scale_xy(sx, sy))
                .then(&sivgo_math::Transform2D::translate(cx, cy))
        });
    }

    /// Translate selected elements by a fixed delta.
    pub fn translate_selected(&mut self, dx: f64, dy: f64) {
        if dx.abs() < f64::EPSILON && dy.abs() < f64::EPSILON {
            return;
        }
        self.apply_transform_to_selected("Move", |_| sivgo_math::Transform2D::translate(dx, dy));
    }

    /// Skew selected elements around their combined center.
    pub fn skew_selected(&mut self, skew_x_degrees: f64, skew_y_degrees: f64) {
        if skew_x_degrees.abs() < f64::EPSILON && skew_y_degrees.abs() < f64::EPSILON {
            return;
        }
        self.apply_transform_to_selected("Skew", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            let cy = (bb.y0 + bb.y1) / 2.0;
            sivgo_math::Transform2D::translate(-cx, -cy)
                .then(&sivgo_math::Transform2D::skew_x(
                    skew_x_degrees.to_radians(),
                ))
                .then(&sivgo_math::Transform2D::skew_y(
                    skew_y_degrees.to_radians(),
                ))
                .then(&sivgo_math::Transform2D::translate(cx, cy))
        });
    }

    /// Set selected elements to an absolute horizontal skew based on the primary object.
    pub fn set_selected_skew_x(&mut self, degrees: f64) {
        let current = self
            .primary_transform_decomposition()
            .map(|value| value.skew_x_degrees)
            .unwrap_or(0.0);
        self.skew_selected(degrees - current, 0.0);
    }

    /// Helper: apply a transform derived from the selection bounding box to all selected elements.
    fn apply_transform_to_selected<F>(&mut self, desc: &str, make_transform: F)
    where
        F: Fn(Rect) -> sivgo_math::Transform2D,
    {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }

        let bboxes: Vec<(NodeId, Rect)> = ids
            .iter()
            .filter_map(|&id| {
                let node = self.document.get_node(id)?;
                let bb = element_bounding_box(&node.element)?;
                Some((id, bb))
            })
            .collect();

        if bboxes.is_empty() {
            return;
        }

        // Compute combined bounding box.
        let mut combined = bboxes[0].1;
        for (_, bb) in &bboxes[1..] {
            combined = combined.union(*bb);
        }

        let additional = make_transform(combined);

        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        for (id, _) in &bboxes {
            let old_t = self
                .document
                .get_node(*id)
                .and_then(|n| n.presentation.transform);
            let new_t = Some(
                old_t
                    .unwrap_or(sivgo_math::Transform2D::IDENTITY)
                    .then(&additional),
            );
            commands.push(Box::new(crate::commands::TransformCommand::new(
                *id, old_t, new_t,
            )));
        }

        if !commands.is_empty() {
            self.execute(Box::new(CompoundCommand::new(desc, commands)));
        }
    }

    /// Get page dimensions from the root SVG element.
    fn get_page_size(&self) -> (f64, f64) {
        if let Some(root) = self.document.root() {
            if let Some(node) = self.document.get_node(root) {
                if let SvgElement::Svg(attrs) = &node.element {
                    let w = attrs.width.map(|l| l.to_px()).unwrap_or(800.0);
                    let h = attrs.height.map(|l| l.to_px()).unwrap_or(600.0);
                    return (w, h);
                }
            }
        }
        (800.0, 600.0)
    }

    /// Current root SVG dimensions in user-space pixels.
    pub fn document_size(&self) -> (f64, f64) {
        self.get_page_size()
    }

    /// Set the root SVG dimensions.
    pub fn set_document_size(&mut self, width: f64, height: f64) {
        let Some(root) = self.document.root() else {
            return;
        };
        let Some(node) = self.document.get_node_mut(root) else {
            return;
        };
        if let SvgElement::Svg(attrs) = &mut node.element {
            attrs.width = Some(SvgLength::new(width.max(1.0), LengthUnit::None));
            attrs.height = Some(SvgLength::new(height.max(1.0), LengthUnit::None));
            self.viewport.width = width.max(1.0);
            self.viewport.height = height.max(1.0);
            self.events.emit(EditorEvent::DocumentModified);
            self.events.emit(EditorEvent::ViewportChanged);
        }
    }

    /// Set Inkscape page background colour on `<sodipodi:namedview>`.
    pub fn set_page_color(&mut self, color: impl Into<String>) {
        let Some(namedview) = self.ensure_namedview() else {
            return;
        };
        if let Some(node) = self.document.get_node_mut(namedview) {
            if let SvgElement::SodipodiNamedView(attrs) = &mut node.element {
                attrs.page_color = Some(color.into());
                self.events.emit(EditorEvent::DocumentModified);
            }
        }
    }

    /// Set namedview grid visibility.
    pub fn set_grid_visible(&mut self, enabled: bool) {
        let Some(namedview) = self.ensure_namedview() else {
            return;
        };
        if let Some(node) = self.document.get_node_mut(namedview) {
            if let SvgElement::SodipodiNamedView(attrs) = &mut node.element {
                attrs.show_grid = Some(enabled.to_string());
                self.events.emit(EditorEvent::DocumentModified);
            }
        }
    }

    /// Set or create the document `<title>` element.
    pub fn set_document_title(&mut self, title: impl Into<String>) {
        self.set_root_text_element("title", title.into());
    }

    /// Set or create the document `<desc>` element.
    pub fn set_document_description(&mut self, description: impl Into<String>) {
        self.set_root_text_element("desc", description.into());
    }

    /// Set or create raw `<metadata>` content.
    pub fn set_document_metadata(&mut self, metadata: impl Into<String>) {
        self.set_root_text_element("metadata", metadata.into());
    }

    fn set_root_text_element(&mut self, tag_name: &str, value: String) {
        let Some(root) = self.document.root() else {
            return;
        };

        let existing = self.document.children(root).iter().copied().find(|id| {
            self.document
                .get_node(*id)
                .map(|node| node.element.tag_name() == tag_name)
                .unwrap_or(false)
        });

        if let Some(id) = existing {
            if let Some(node) = self.document.get_node_mut(id) {
                match &mut node.element {
                    SvgElement::Title(attrs) => attrs.text_content = value,
                    SvgElement::Desc(attrs) => attrs.text_content = value,
                    SvgElement::Metadata(attrs) => attrs.raw_content = value,
                    _ => {}
                }
                self.events.emit(EditorEvent::DocumentModified);
            }
            return;
        }

        let element = match tag_name {
            "title" => SvgElement::Title(TitleAttrs {
                text_content: value,
            }),
            "desc" => SvgElement::Desc(DescAttrs {
                text_content: value,
            }),
            "metadata" => SvgElement::Metadata(MetadataAttrs { raw_content: value }),
            _ => return,
        };
        let id = self.document.add_node(element);
        self.document.append_child(root, id);
        self.events.emit(EditorEvent::DocumentModified);
    }

    // -----------------------------------------------------------------------
    // Lock/Unlock, Hide/Show (9.6.9)
    // -----------------------------------------------------------------------

    /// Toggle the visibility (hidden state) of a single object.
    pub fn toggle_visibility(&mut self, id: NodeId) {
        if let Some(node) = self.document.get_node(id) {
            let cmd = SetVisibilityCommand::toggle(id, node.hidden);
            self.execute(Box::new(cmd));
        }
    }

    /// Toggle the locked state of a single object.
    pub fn toggle_lock(&mut self, id: NodeId) {
        if let Some(node) = self.document.get_node(id) {
            let cmd = SetLockCommand::toggle(id, node.locked);
            self.execute(Box::new(cmd));
        }
    }

    /// Set or clear an object's display label.
    pub fn set_object_label(&mut self, id: NodeId, label: impl Into<String>) {
        let label = label.into();
        let new_label = match label.trim() {
            "" => None,
            value => Some(value.to_string()),
        };
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        let old_label = node.attributes.get("inkscape:label").cloned();
        if old_label == new_label {
            return;
        }

        self.execute(Box::new(ModifyObjectLabelCommand::new(
            id, old_label, new_label,
        )));
    }

    /// Set or clear an object's serialized SVG `id`.
    ///
    /// Returns `false` when the object is missing or the requested id already
    /// belongs to a different node.
    pub fn set_object_xml_id(&mut self, id: NodeId, xml_id: impl Into<String>) -> bool {
        let new_xml_id = Self::normalize_user_xml_id(&xml_id.into());
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        if let Some(value) = new_xml_id.as_deref() {
            if let Some(existing) = self.document.find_by_xml_id(value) {
                if existing != id {
                    return false;
                }
            }
        }

        let old_xml_id = node.xml_id.clone();
        if old_xml_id == new_xml_id {
            return true;
        }

        self.execute(Box::new(ModifyXmlIdCommand::new(
            id, old_xml_id, new_xml_id,
        )));
        true
    }

    fn normalize_user_xml_id(value: &str) -> Option<String> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let mut normalized = String::new();
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') {
                normalized.push(ch);
            } else {
                normalized.push('-');
            }
        }

        let starts_valid = normalized
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
            .unwrap_or(false);
        if !starts_valid {
            normalized.insert_str(0, "object-");
        }

        Some(normalized)
    }

    /// Hide all currently selected elements.
    pub fn hide_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| Box::new(SetVisibilityCommand::new(id, true)) as Box<dyn Command>)
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Hide selected", commands)));
        }
    }

    /// Show all currently selected elements (un-hide).
    pub fn show_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| Box::new(SetVisibilityCommand::new(id, false)) as Box<dyn Command>)
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Show selected", commands)));
        }
    }

    /// Lock all currently selected elements.
    pub fn lock_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| Box::new(SetLockCommand::new(id, true)) as Box<dyn Command>)
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Lock selected", commands)));
        }
    }

    /// Unlock all currently selected elements.
    pub fn unlock_selected(&mut self) {
        let ids: Vec<NodeId> = self.selection.selected().to_vec();
        if ids.is_empty() {
            return;
        }
        let commands: Vec<Box<dyn Command>> = ids
            .into_iter()
            .map(|id| Box::new(SetLockCommand::new(id, false)) as Box<dyn Command>)
            .collect();
        if commands.len() == 1 {
            self.execute(commands.into_iter().next().unwrap());
        } else {
            self.execute(Box::new(CompoundCommand::new("Unlock selected", commands)));
        }
    }

    // -----------------------------------------------------------------------
    // Layer management (9.6.10)
    // -----------------------------------------------------------------------

    /// Create a new layer (a `<g>` with `inkscape:groupmode="layer"`) and add
    /// it as a child of the root `<svg>` element.
    ///
    /// Returns the [`NodeId`] of the created layer, or `None` if there is no root.
    pub fn add_layer(&mut self, name: &str) -> Option<NodeId> {
        self.add_layer_under(None, name)
    }

    /// Create a new layer under `parent`, or under the root when `parent` is `None`.
    ///
    /// Nested layers are represented the same way as top-level layers: an SVG
    /// group with `inkscape:groupmode="layer"`.
    pub fn add_layer_under(&mut self, parent: Option<NodeId>, name: &str) -> Option<NodeId> {
        let root = self.document.root()?;
        let parent = match parent {
            Some(id) if self.is_layer(id) || id == root => id,
            Some(_) => return None,
            None => root,
        };
        let element = SvgElement::Group(GroupAttrs {
            inkscape_groupmode: Some("layer".to_string()),
            inkscape_label: Some(name.to_string()),
        });
        let cmd = AddNodeCommand::new(parent, element, PresentationAttrs::default(), None);
        let cmd = Box::new(cmd);
        self.execute(cmd);

        // The last child of the target parent should be the new layer.
        let children = self.document.children(parent);
        let layer_id = *children.last()?;
        self.active_layer = Some(layer_id);
        self.events.emit(EditorEvent::DocumentModified);
        Some(layer_id)
    }

    /// Create a child layer under the active layer, falling back to the root.
    pub fn add_sublayer(&mut self, name: &str) -> Option<NodeId> {
        let parent = self.active_layer.filter(|id| self.is_layer(*id));
        self.add_layer_under(parent, name)
    }

    /// Delete a layer. Children of the layer are removed along with it.
    pub fn delete_layer(&mut self, id: NodeId) {
        // Verify it is actually a layer.
        if !self.is_layer(id) {
            return;
        }

        // If this was the active layer, clear the active layer.
        if self.active_layer == Some(id) {
            self.active_layer = None;
        }

        // Remove any selected nodes that are children of this layer.
        let descendants = self.document.traverse_pre_order(id);
        for &desc_id in &descendants {
            self.selection.remove(desc_id);
        }
        self.events.emit(EditorEvent::SelectionChanged);

        self.execute(Box::new(RemoveNodeCommand::new(id)));
    }

    /// Rename a layer by setting its `inkscape:label`.
    pub fn rename_layer(&mut self, id: NodeId, name: &str) {
        if !self.is_layer(id) {
            return;
        }

        let Some(node) = self.document.get_node(id) else {
            return;
        };
        let SvgElement::Group(attrs) = &node.element else {
            return;
        };
        let name = name.trim();
        if name.is_empty() || attrs.inkscape_label.as_deref() == Some(name) {
            return;
        }

        let old_element = node.element.clone();
        let old_presentation = node.presentation.clone();
        let mut new_attrs = attrs.clone();
        new_attrs.inkscape_label = Some(name.to_string());
        let new_element = SvgElement::Group(new_attrs);

        self.execute(Box::new(ModifyElementCommand::new(
            id,
            old_element,
            new_element,
            old_presentation.clone(),
            old_presentation,
        )));
    }

    /// Raise a layer one step among its siblings (move it toward the end /
    /// front of the children list).
    pub fn raise_layer(&mut self, id: NodeId) {
        if !self.is_layer(id) {
            return;
        }
        self.execute(Box::new(ReorderCommand::new(id, ReorderDirection::Raise)));
    }

    /// Lower a layer one step among its siblings (move it toward the start /
    /// back of the children list).
    pub fn lower_layer(&mut self, id: NodeId) {
        if !self.is_layer(id) {
            return;
        }
        self.execute(Box::new(ReorderCommand::new(id, ReorderDirection::Lower)));
    }

    /// Merge a layer into the layer directly below it in document order.
    pub fn merge_layer_down(&mut self, id: NodeId) {
        if !self.is_layer(id) {
            return;
        }

        let layers = self.get_layers();
        let Some(index) = layers.iter().position(|(layer_id, _)| *layer_id == id) else {
            return;
        };
        if index == 0 {
            return;
        }
        let target_id = layers[index - 1].0;

        if self.active_layer == Some(id) {
            self.active_layer = Some(target_id);
        }
        self.selection.remove(id);
        self.execute(Box::new(MergeLayerCommand::new(id, target_id)));
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Flatten all top-level layers by ungrouping each layer into the root.
    pub fn flatten_layers(&mut self) {
        let layers = self.get_layers();
        if layers.is_empty() {
            return;
        }

        let layer_ids: Vec<NodeId> = layers.into_iter().map(|(id, _)| id).collect();
        let commands: Vec<Box<dyn Command>> = layer_ids
            .iter()
            .copied()
            .map(|id| Box::new(UngroupCommand::new(id)) as Box<dyn Command>)
            .collect();

        self.active_layer = None;
        for id in layer_ids {
            self.selection.remove(id);
        }
        self.execute_command_batch("Flatten layers", commands);
        self.events.emit(EditorEvent::SelectionChanged);
    }

    /// Return a list of all layers (groups with `inkscape:groupmode="layer"`)
    /// that are direct children of the root element, along with their names.
    pub fn get_layers(&self) -> Vec<(NodeId, String)> {
        let root = match self.document.root() {
            Some(r) => r,
            None => return Vec::new(),
        };

        self.document
            .children(root)
            .iter()
            .filter_map(|&child_id| {
                let node = self.document.get_node(child_id)?;
                if let SvgElement::Group(attrs) = &node.element {
                    if attrs.inkscape_groupmode.as_deref() == Some("layer") {
                        let label = attrs
                            .inkscape_label
                            .clone()
                            .unwrap_or_else(|| "Unnamed layer".to_string());
                        return Some((child_id, label));
                    }
                }
                None
            })
            .collect()
    }

    /// Set the active layer. New objects created by tools will be added to
    /// this layer.
    ///
    /// Pass `None` to reset to the root.
    pub fn set_active_layer(&mut self, id: NodeId) {
        if self.is_layer(id) {
            self.active_layer = Some(id);
        }
    }

    /// Helper: check whether a node is a layer (a Group with
    /// `inkscape:groupmode="layer"`).
    fn is_layer(&self, id: NodeId) -> bool {
        if let Some(node) = self.document.get_node(id) {
            if let SvgElement::Group(attrs) = &node.element {
                return attrs.inkscape_groupmode.as_deref() == Some("layer");
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Multi-page support (inkscape:page)
    // -----------------------------------------------------------------------

    /// Find the `<sodipodi:namedview>` element, or create one under root.
    fn ensure_namedview(&mut self) -> Option<NodeId> {
        let root = self.document.root()?;
        // Search for existing namedview
        for &child in self.document.children(root) {
            if let Some(node) = self.document.get_node(child) {
                if matches!(&node.element, SvgElement::SodipodiNamedView(_)) {
                    return Some(child);
                }
            }
        }
        // Create one
        let nv_id = self.document.add_node(SvgElement::SodipodiNamedView(
            sivgo_dom::NamedViewAttrs::default(),
        ));
        self.document.append_child(root, nv_id);
        Some(nv_id)
    }

    /// Add a new page with the given position and size.
    ///
    /// Returns the `NodeId` of the created `<inkscape:page>` element.
    pub fn add_page(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        label: Option<&str>,
    ) -> Option<NodeId> {
        let cmd =
            AddPageCommand::new(&self.document, x, y, width, height, label.map(String::from))?;
        let page_id = cmd.page_id();
        self.execute(Box::new(cmd));
        Some(page_id)
    }

    /// Delete a page by its `NodeId`.
    pub fn delete_page(&mut self, page_id: NodeId) {
        if let Some(node) = self.document.get_node(page_id) {
            if !matches!(&node.element, SvgElement::InkscapePage(_)) {
                return;
            }
        } else {
            return;
        }
        self.execute(Box::new(RemoveNodeCommand::new(page_id)));
    }

    /// Resize a page.
    pub fn resize_page(&mut self, page_id: NodeId, width: f64, height: f64) {
        let Some(old_attrs) = self.page_attrs(page_id) else {
            return;
        };
        let mut new_attrs = old_attrs.clone();
        new_attrs.width = Some(width.max(1.0).to_string());
        new_attrs.height = Some(height.max(1.0).to_string());
        self.modify_page_attrs(page_id, old_attrs, new_attrs);
    }

    /// Move a page to a new position.
    pub fn move_page(&mut self, page_id: NodeId, x: f64, y: f64) {
        let Some(old_attrs) = self.page_attrs(page_id) else {
            return;
        };
        let mut new_attrs = old_attrs.clone();
        new_attrs.x = Some(x.to_string());
        new_attrs.y = Some(y.to_string());
        self.modify_page_attrs(page_id, old_attrs, new_attrs);
    }

    /// Set a page label.
    pub fn set_page_label(&mut self, page_id: NodeId, label: impl Into<String>) {
        let Some(old_attrs) = self.page_attrs(page_id) else {
            return;
        };
        let mut new_attrs = old_attrs.clone();
        let label = label.into();
        new_attrs.inkscape_label = if label.trim().is_empty() {
            None
        } else {
            Some(label)
        };
        self.modify_page_attrs(page_id, old_attrs, new_attrs);
    }

    fn page_attrs(&self, page_id: NodeId) -> Option<InkscapePageAttrs> {
        let node = self.document.get_node(page_id)?;
        match &node.element {
            SvgElement::InkscapePage(attrs) => Some(attrs.clone()),
            _ => None,
        }
    }

    fn modify_page_attrs(
        &mut self,
        page_id: NodeId,
        old_attrs: InkscapePageAttrs,
        new_attrs: InkscapePageAttrs,
    ) {
        let old_values = (
            old_attrs.x.as_deref(),
            old_attrs.y.as_deref(),
            old_attrs.width.as_deref(),
            old_attrs.height.as_deref(),
            old_attrs.inkscape_label.as_deref(),
        );
        let new_values = (
            new_attrs.x.as_deref(),
            new_attrs.y.as_deref(),
            new_attrs.width.as_deref(),
            new_attrs.height.as_deref(),
            new_attrs.inkscape_label.as_deref(),
        );
        if old_values == new_values {
            return;
        }
        self.execute(Box::new(ModifyPageCommand::new(
            page_id, old_attrs, new_attrs,
        )));
    }

    /// List all pages in the document.
    ///
    /// Returns `(NodeId, label, x, y, width, height)` for each page.
    pub fn get_pages(&self) -> Vec<(NodeId, String, f64, f64, f64, f64)> {
        let mut pages = Vec::new();
        let root = match self.document.root() {
            Some(r) => r,
            None => return pages,
        };
        for &child in self.document.children(root) {
            if let Some(node) = self.document.get_node(child) {
                if matches!(&node.element, SvgElement::SodipodiNamedView(_)) {
                    for &nv_child in self.document.children(child) {
                        if let Some(page_node) = self.document.get_node(nv_child) {
                            if let SvgElement::InkscapePage(attrs) = &page_node.element {
                                let x = attrs
                                    .x
                                    .as_deref()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                let y = attrs
                                    .y
                                    .as_deref()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                let w = attrs
                                    .width
                                    .as_deref()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                let h = attrs
                                    .height
                                    .as_deref()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                let label = attrs.inkscape_label.clone().unwrap_or_default();
                                pages.push((nv_child, label, x, y, w, h));
                            }
                        }
                    }
                }
            }
        }
        pages
    }

    /// Navigate viewport to center on a specific page.
    pub fn navigate_to_page(&mut self, page_id: NodeId) {
        if let Some(node) = self.document.get_node(page_id) {
            if let SvgElement::InkscapePage(attrs) = &node.element {
                let x = attrs
                    .x
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = attrs
                    .y
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = attrs
                    .width
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(100.0);
                let h = attrs
                    .height
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(100.0);
                let rect = Rect::new(x, y, x + w, y + h);
                self.viewport.fit_rect(rect);
                self.events.emit(EditorEvent::ViewportChanged);
            }
        }
    }

    /// Return the number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.get_pages().len()
    }

    /// Navigate to the next page (wraps around to the first page).
    ///
    /// Returns `true` if navigation occurred, `false` if there are no pages.
    pub fn navigate_to_next_page(&mut self) -> bool {
        let pages = self.get_pages();
        if pages.is_empty() {
            return false;
        }

        // Determine the currently displayed page by finding the one whose
        // center is closest to the viewport center.
        let vp_center = self.viewport.screen_to_document(Point::new(
            self.viewport.width / 2.0,
            self.viewport.height / 2.0,
        ));

        let current_index = Self::find_closest_page_index(&pages, vp_center);
        let next_index = (current_index + 1) % pages.len();
        let next_page_id = pages[next_index].0;
        self.navigate_to_page(next_page_id);
        true
    }

    /// Navigate to the previous page (wraps around to the last page).
    ///
    /// Returns `true` if navigation occurred, `false` if there are no pages.
    pub fn navigate_to_prev_page(&mut self) -> bool {
        let pages = self.get_pages();
        if pages.is_empty() {
            return false;
        }

        let vp_center = self.viewport.screen_to_document(Point::new(
            self.viewport.width / 2.0,
            self.viewport.height / 2.0,
        ));

        let current_index = Self::find_closest_page_index(&pages, vp_center);
        let prev_index = if current_index == 0 {
            pages.len() - 1
        } else {
            current_index - 1
        };
        let prev_page_id = pages[prev_index].0;
        self.navigate_to_page(prev_page_id);
        true
    }

    /// Find the index of the page whose center is closest to `point`.
    fn find_closest_page_index(
        pages: &[(NodeId, String, f64, f64, f64, f64)],
        point: Point,
    ) -> usize {
        let mut best_index = 0;
        let mut best_dist = f64::MAX;
        for (i, (_id, _label, x, y, w, h)) in pages.iter().enumerate() {
            let cx = x + w / 2.0;
            let cy = y + h / 2.0;
            let d = (cx - point.x) * (cx - point.x) + (cy - point.y) * (cy - point.y);
            if d < best_dist {
                best_dist = d;
                best_index = i;
            }
        }
        best_index
    }

    /// Export a single page as a standalone SVG string.
    ///
    /// The returned SVG has its `width`/`height` set to the page dimensions,
    /// and a `viewBox` matching the page area so only content within the page
    /// region is visible.  All document content is preserved (the viewBox
    /// clips visually).
    ///
    /// Returns `None` if the page does not exist or is not an InkscapePage.
    pub fn export_page_svg(&self, page_id: NodeId) -> Option<String> {
        let node = self.document.get_node(page_id)?;
        let (x, y, w, h) = match &node.element {
            SvgElement::InkscapePage(a) => {
                let x =
                    a.x.as_deref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                let y =
                    a.y.as_deref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                let w = a
                    .width
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(100.0);
                let h = a
                    .height
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(100.0);
                (x, y, w, h)
            }
            _ => return None,
        };

        // Verify the document has a root SVG element and serialize it,
        // then patch the root `<svg>` tag attributes to match the page area.
        let root = self.document.root()?;
        match self.document.get_node(root) {
            Some(n) if matches!(&n.element, SvgElement::Svg(_)) => {}
            _ => return None,
        }
        let serialized = sivgo_dom::serialize_svg(&self.document);
        if serialized.is_empty() {
            return None;
        }

        // Find the first `<svg` opening and inject/replace width, height,
        // viewBox attributes.
        let vb = format!("{} {} {} {}", x, y, w, h);
        let patched = patch_svg_root_attrs(&serialized, w, h, &vb);
        Some(patched)
    }

    /// Export all pages as a vector of `(label, svg_string)` pairs.
    ///
    /// This enables batch per-page export (17.6.3).
    pub fn export_all_pages_svg(&self) -> Vec<(String, String)> {
        let pages = self.get_pages();
        let mut results = Vec::new();
        for (page_id, label, _x, _y, _w, _h) in &pages {
            if let Some(svg) = self.export_page_svg(*page_id) {
                let name = if label.is_empty() {
                    format!("page_{}", results.len() + 1)
                } else {
                    label.clone()
                };
                results.push((name, svg));
            }
        }
        results
    }
}

#[cfg(test)]
mod interaction_performance_tests {
    use super::*;
    use crate::snap::SnapTarget;

    #[test]
    fn mouse_move_clears_snap_feedback_without_running_snap_pass() {
        let mut engine = EditorEngine::new(100.0, 100.0);
        engine.tools.switch_to(ToolId::Rect);
        engine.last_snap_indicator = Some(SnapIndicator {
            crosshair_lines: Vec::new(),
            distance_label: None,
            target: SnapTarget::Grid,
        });

        engine.on_mouse_move(MouseEvent::left(9.0, 9.0));

        assert!(engine.last_snap_indicator.is_none());
    }
}

// ---------------------------------------------------------------------------
// PasteSubtreeCommand -- inserts a subtree into the document
// ---------------------------------------------------------------------------

/// Snapshot of one node for serialization-independent paste.
#[derive(Debug, Clone)]
struct SubtreeSnapshot {
    element: SvgElement,
    presentation: PresentationAttrs,
    xml_id: Option<String>,
    attributes: std::collections::HashMap<String, String>,
    namespaces: std::collections::HashMap<String, String>,
    children: Vec<SubtreeSnapshot>,
}

/// A command that pastes a full subtree (a node with descendants) into the
/// document.  Unlike `AddNodeCommand` (which only adds leaf nodes), this
/// command deep-inserts the whole subtree in a single execute call.
#[derive(Debug)]
struct PasteSubtreeCommand {
    parent_id: NodeId,
    snapshot: SubtreeSnapshot,
    /// Ids created during execute, used for undo.
    created_ids: Vec<NodeId>,
    /// The top-level node id, for undo bookkeeping.
    root_created_id: Option<NodeId>,
}

impl PasteSubtreeCommand {
    /// Build a `PasteSubtreeCommand` from a parsed temporary document.
    ///
    /// `node_id` is the id of the root of the subtree inside `parsed_doc`.
    /// `parent_id` is the id in the *real* document where the subtree will
    /// be inserted.  `pres_override` replaces the root node's presentation
    /// attrs (used to apply the paste offset).
    fn from_parsed(
        parsed_doc: &SvgDocument,
        node_id: NodeId,
        parent_id: NodeId,
        pres_override: PresentationAttrs,
    ) -> Self {
        let mut snapshot = Self::snapshot_subtree(parsed_doc, node_id);
        snapshot.presentation = pres_override;
        Self {
            parent_id,
            snapshot,
            created_ids: Vec::new(),
            root_created_id: None,
        }
    }

    fn snapshot_subtree(doc: &SvgDocument, node_id: NodeId) -> SubtreeSnapshot {
        let node = doc
            .get_node(node_id)
            .expect("node must exist in parsed doc");
        let children_ids: Vec<NodeId> = doc.children(node_id).to_vec();
        let children = children_ids
            .iter()
            .map(|&cid| Self::snapshot_subtree(doc, cid))
            .collect();

        SubtreeSnapshot {
            element: node.element.clone(),
            presentation: node.presentation.clone(),
            xml_id: node.xml_id.clone(),
            attributes: node.attributes.clone(),
            namespaces: node.namespaces.clone(),
            children,
        }
    }

    /// Recursively insert a snapshot subtree into the document under `parent`.
    fn insert_snapshot(
        doc: &mut SvgDocument,
        snapshot: &SubtreeSnapshot,
        parent: NodeId,
        created_ids: &mut Vec<NodeId>,
    ) -> NodeId {
        let id = doc.add_node(snapshot.element.clone());
        if let Some(node) = doc.get_node_mut(id) {
            node.presentation = snapshot.presentation.clone();
            node.xml_id = snapshot.xml_id.clone();
            node.attributes = snapshot.attributes.clone();
            node.namespaces = snapshot.namespaces.clone();
        }
        doc.append_child(parent, id);
        created_ids.push(id);

        for child_snap in &snapshot.children {
            Self::insert_snapshot(doc, child_snap, id, created_ids);
        }

        id
    }
}

impl Command for PasteSubtreeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        self.created_ids.clear();
        let root_id =
            Self::insert_snapshot(doc, &self.snapshot, self.parent_id, &mut self.created_ids);
        self.root_created_id = Some(root_id);
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Remove in reverse order so children are removed before parents.
        for &id in self.created_ids.iter().rev() {
            doc.remove_node(id);
        }
        self.created_ids.clear();
        self.root_created_id = None;
    }

    fn description(&self) -> &str {
        "Paste subtree"
    }
}

// ---------------------------------------------------------------------------
// Alignment / Distribution types
// ---------------------------------------------------------------------------

/// Alignment direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

/// What to align relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignRelativeTo {
    Selection,
    Page,
}

/// Patch the root `<svg>` tag in an SVG string, replacing width, height, and
/// viewBox attributes so the output represents a specific page area.
fn patch_svg_root_attrs(svg: &str, width: f64, height: f64, view_box: &str) -> String {
    // Find the first `<svg` tag and its closing `>`.
    let svg_start = match svg.find("<svg") {
        Some(pos) => pos,
        None => return svg.to_string(),
    };
    let rest = &svg[svg_start..];
    let tag_end = match rest.find('>') {
        Some(pos) => svg_start + pos,
        None => return svg.to_string(),
    };

    let tag = &svg[svg_start..tag_end];

    // Helper to replace or insert an attribute in the tag.
    let replace_attr = |tag: &str, name: &str, value: &str| -> String {
        let attr_pattern = format!("{}=\"", name);
        if let Some(start) = tag.find(&attr_pattern) {
            // Find closing quote.
            let value_start = start + attr_pattern.len();
            if let Some(end) = tag[value_start..].find('"') {
                let mut result = String::new();
                result.push_str(&tag[..value_start]);
                result.push_str(value);
                result.push_str(&tag[value_start + end..]);
                return result;
            }
        }
        // Attribute not found: insert before the end of the tag.
        // Find last space or the `<svg ` prefix to insert before.
        let mut result = tag.to_string();
        result.push(' ');
        result.push_str(&format!("{}=\"{}\"", name, value));
        result
    };

    let mut new_tag = tag.to_string();
    new_tag = replace_attr(&new_tag, "width", &format!("{}", width));
    new_tag = replace_attr(&new_tag, "height", &format!("{}", height));
    new_tag = replace_attr(&new_tag, "viewBox", view_box);

    let mut result = String::new();
    result.push_str(&svg[..svg_start]);
    result.push_str(&new_tag);
    result.push_str(&svg[tag_end..]);
    result
}

/// Distribute axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributeAxis {
    Horizontal,
    Vertical,
}

impl std::fmt::Debug for EditorEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorEngine")
            .field("document_nodes", &self.document.len())
            .field("selection", &self.selection)
            .field("viewport", &self.viewport)
            .field("history", &self.history)
            .finish()
    }
}
