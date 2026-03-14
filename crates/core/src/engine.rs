//! Editor engine facade.
//!
//! [`EditorEngine`] ties together the document, command history, selection,
//! tools, viewport, snapping, and event bus into one coherent API.

use sivgo_dom::{GroupAttrs, NodeId, PresentationAttrs, SvgDocument, SvgElement, SvgSvgAttrs};
use sivgo_math::{Point, Rect, Transform2D};

use crate::selection::element_bounding_box;

use crate::command::Command;
use crate::command::CommandHistory;
use crate::commands::{
    AddNodeCommand, CompoundCommand, GroupCommand, ModifyAttributeCommand, RemoveNodeCommand,
    ReorderCommand, ReorderDirection, SetLockCommand, SetVisibilityCommand, UngroupCommand,
};
use crate::event::{EditorEvent, EventBus};
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::snap::SnappingEngine;
use crate::tool::{ToolId, ToolManager, ToolResponse};
use crate::tools::{
    DropperTool, EllipseTool, EraserTool, LpeTool, MeasureTool, PenTool, RectTool, SelectTool,
    ZoomTool,
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

/// The top-level editor engine.
pub struct EditorEngine {
    pub document: SvgDocument,
    pub history: CommandHistory,
    pub selection: SelectionSet,
    pub tools: ToolManager,
    pub viewport: Viewport,
    pub snapping: SnappingEngine,
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
        tools.register(ToolId::Rect, Box::new(RectTool::new()));
        tools.register(ToolId::Ellipse, Box::new(EllipseTool::new()));
        tools.register(ToolId::Pen, Box::new(PenTool::new()));
        tools.register(ToolId::Dropper, Box::new(DropperTool::new()));
        tools.register(ToolId::Eraser, Box::new(EraserTool::new()));
        tools.register(ToolId::Measure, Box::new(MeasureTool::new()));
        tools.register(ToolId::Zoom, Box::new(ZoomTool::new()));
        tools.register(ToolId::Lpe, Box::new(LpeTool::new()));

        let mut engine = Self {
            document: SvgDocument::new(),
            history: CommandHistory::new(200),
            selection: SelectionSet::new(),
            tools,
            viewport: Viewport::new(width, height),
            snapping: SnappingEngine::new(),
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
            self.events.emit(EditorEvent::DocumentModified);
            self.events.emit(EditorEvent::UndoRedoChanged);
        }
    }

    /// Redo the last undone command.
    pub fn redo(&mut self) {
        if self.history.redo(&mut self.document) {
            self.events.emit(EditorEvent::DocumentModified);
            self.events.emit(EditorEvent::UndoRedoChanged);
        }
    }

    // -----------------------------------------------------------------------
    // Input dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a mouse-down event to the active tool.
    pub fn on_mouse_down(&mut self, event: MouseEvent) {
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
    }

    /// Dispatch a mouse-move event to the active tool.
    pub fn on_mouse_move(&mut self, event: MouseEvent) {
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
                self.execute(cmd);
            }
            ToolResponse::Handled | ToolResponse::None => {}
        }
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

    /// Select all elements that are direct children of the root.
    pub fn select_all(&mut self) {
        self.selection.clear();
        if let Some(root) = self.document.root() {
            for &child in self.document.children(root) {
                // Skip non-graphical children like defs.
                if let Some(node) = self.document.get_node(child) {
                    match &node.element {
                        SvgElement::Defs(_) => continue,
                        _ => self.selection.add(child),
                    }
                }
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
        let source = self.document.get_node(source_id)?;
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

        let children: Vec<NodeId> = self.document.children(source_id).to_vec();
        for child_id in children {
            self.deep_clone_into(child_id, new_id, target_doc);
        }

        Some(new_id)
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

        let root = match self.document.root() {
            Some(r) => r,
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
                            root,
                            child_node.element.clone(),
                            pres,
                            None,
                        )));
                    } else {
                        // Subtree -- use PasteSubtreeCommand.
                        commands.push(Box::new(PasteSubtreeCommand::from_parsed(
                            &parsed_doc,
                            child_id,
                            root,
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
                new_pres.color = source_pres.color.clone();
                new_pres.fill = source_pres.fill.clone();
                new_pres.stroke = source_pres.stroke.clone();
                new_pres.stroke_width = source_pres.stroke_width.clone();
                new_pres.opacity = source_pres.opacity;
                new_pres.fill_opacity = source_pres.fill_opacity;
                new_pres.stroke_opacity = source_pres.stroke_opacity;
                new_pres.stroke_linecap = source_pres.stroke_linecap.clone();
                new_pres.stroke_linejoin = source_pres.stroke_linejoin.clone();
                new_pres.stroke_miterlimit = source_pres.stroke_miterlimit;
                new_pres.stroke_dasharray = source_pres.stroke_dasharray.clone();
                new_pres.stroke_dashoffset = source_pres.stroke_dashoffset;
                new_pres.fill_rule = source_pres.fill_rule.clone();
                new_pres.display = source_pres.display.clone();
                new_pres.visibility = source_pres.visibility.clone();
                new_pres.mix_blend_mode = source_pres.mix_blend_mode.clone();
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

    /// Flip selected elements horizontally.
    pub fn flip_horizontal(&mut self) {
        self.apply_transform_to_selected("Flip Horizontal", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            sivgo_math::Transform2D::translate(cx, 0.0)
                .then(&sivgo_math::Transform2D::scale_xy(-1.0, 1.0))
                .then(&sivgo_math::Transform2D::translate(-cx, 0.0))
        });
    }

    /// Flip selected elements vertically.
    pub fn flip_vertical(&mut self) {
        self.apply_transform_to_selected("Flip Vertical", |bb| {
            let cy = (bb.y0 + bb.y1) / 2.0;
            sivgo_math::Transform2D::translate(0.0, cy)
                .then(&sivgo_math::Transform2D::scale_xy(1.0, -1.0))
                .then(&sivgo_math::Transform2D::translate(0.0, -cy))
        });
    }

    /// Rotate selected elements by the given degrees around their combined center.
    pub fn rotate_selected(&mut self, degrees: f64) {
        self.apply_transform_to_selected("Rotate", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            let cy = (bb.y0 + bb.y1) / 2.0;
            let radians = degrees.to_radians();
            sivgo_math::Transform2D::translate(cx, cy)
                .then(&sivgo_math::Transform2D::rotate(radians))
                .then(&sivgo_math::Transform2D::translate(-cx, -cy))
        });
    }

    /// Scale selected elements by the given factor around their combined center.
    pub fn scale_selected(&mut self, sx: f64, sy: f64) {
        self.apply_transform_to_selected("Scale", |bb| {
            let cx = (bb.x0 + bb.x1) / 2.0;
            let cy = (bb.y0 + bb.y1) / 2.0;
            sivgo_math::Transform2D::translate(cx, cy)
                .then(&sivgo_math::Transform2D::scale_xy(sx, sy))
                .then(&sivgo_math::Transform2D::translate(-cx, -cy))
        });
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
        let root = self.document.root()?;
        let element = SvgElement::Group(GroupAttrs {
            inkscape_groupmode: Some("layer".to_string()),
            inkscape_label: Some(name.to_string()),
        });
        let cmd = AddNodeCommand::new(root, element, PresentationAttrs::default(), None);
        let cmd = Box::new(cmd);
        self.execute(cmd);

        // The last child of root should be the new layer.
        let children = self.document.children(root);
        let layer_id = *children.last()?;
        self.active_layer = Some(layer_id);
        self.events.emit(EditorEvent::DocumentModified);
        Some(layer_id)
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
        if let Some(node) = self.document.get_node_mut(id) {
            if let SvgElement::Group(ref mut attrs) = node.element {
                attrs.inkscape_label = Some(name.to_string());
                self.events.emit(EditorEvent::DocumentModified);
            }
        }
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
        let nv = self.ensure_namedview()?;
        let page = SvgElement::InkscapePage(sivgo_dom::InkscapePageAttrs {
            x: Some(x.to_string()),
            y: Some(y.to_string()),
            width: Some(width.to_string()),
            height: Some(height.to_string()),
            inkscape_label: label.map(String::from),
        });
        let page_id = self.document.add_node(page);
        self.document.append_child(nv, page_id);
        self.events.emit(EditorEvent::DocumentModified);
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
        self.document.remove_node(page_id);
        self.events.emit(EditorEvent::DocumentModified);
    }

    /// Resize a page.
    pub fn resize_page(&mut self, page_id: NodeId, width: f64, height: f64) {
        if let Some(node) = self.document.get_node_mut(page_id) {
            if let SvgElement::InkscapePage(attrs) = &mut node.element {
                attrs.width = Some(width.to_string());
                attrs.height = Some(height.to_string());
            }
        }
        self.events.emit(EditorEvent::DocumentModified);
    }

    /// Move a page to a new position.
    pub fn move_page(&mut self, page_id: NodeId, x: f64, y: f64) {
        if let Some(node) = self.document.get_node_mut(page_id) {
            if let SvgElement::InkscapePage(attrs) = &mut node.element {
                attrs.x = Some(x.to_string());
                attrs.y = Some(y.to_string());
            }
        }
        self.events.emit(EditorEvent::DocumentModified);
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
