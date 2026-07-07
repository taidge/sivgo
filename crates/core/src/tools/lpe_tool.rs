//! LPE Tool: interactively apply and edit Live Path Effects.
//!
//! This tool allows users to select a path element on the canvas and then
//! apply or edit Live Path Effects from the LPE registry. In *Idle* state
//! the tool waits for a mouse click. When a path is clicked, it transitions
//! to *Selecting* where the user can choose an LPE to apply. Once an LPE is
//! applied (or an existing LPE is selected for editing), the tool enters
//! *Editing* state where parameter adjustments can be made on canvas.

use sivgo_dom::{NodeId, SvgDocument, SvgElement};
use sivgo_math::Point;

use crate::commands::AddLpeCommand;
use crate::input::{Key, KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// Available Live Path Effects that can be applied via the LPE tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailableLpe {
    /// Fillet / Chamfer effect.
    FilletChamfer,
    /// Power Stroke effect.
    PowerStroke,
    /// Bend Path effect.
    BendPath,
    /// Pattern Along Path effect.
    PatternAlongPath,
    /// Spiro smooth curves.
    Spiro,
    /// Simplify path.
    Simplify,
    /// Roughen path.
    Roughen,
    /// Mirror Symmetry.
    MirrorSymmetry,
    /// Copy & Rotate.
    CopyRotate,
    /// B-Spline.
    BSpline,
    /// Sketch (hand-drawn look).
    Sketch,
    /// Tiling (pattern clones).
    Tiling,
    /// Offset path.
    Offset,
    /// Envelope deformation.
    Envelope,
    /// Perspective envelope.
    PerspectiveEnvelope,
    /// Lattice deformation.
    Lattice,
    /// Measure segments.
    MeasureSegments,
    /// Gears generator.
    Gears,
    /// Dashed stroke.
    DashedStroke,
    /// Extrude (3D-like extrusion).
    Extrude,
    /// Knot (crossing management).
    Knot,
}

impl AvailableLpe {
    /// Return the list of all available LPEs.
    pub fn all() -> &'static [AvailableLpe] {
        &[
            AvailableLpe::FilletChamfer,
            AvailableLpe::PowerStroke,
            AvailableLpe::BendPath,
            AvailableLpe::PatternAlongPath,
            AvailableLpe::Spiro,
            AvailableLpe::Simplify,
            AvailableLpe::Roughen,
            AvailableLpe::MirrorSymmetry,
            AvailableLpe::CopyRotate,
            AvailableLpe::BSpline,
            AvailableLpe::Sketch,
            AvailableLpe::Tiling,
            AvailableLpe::Offset,
            AvailableLpe::Envelope,
            AvailableLpe::PerspectiveEnvelope,
            AvailableLpe::Lattice,
            AvailableLpe::MeasureSegments,
            AvailableLpe::Gears,
            AvailableLpe::DashedStroke,
            AvailableLpe::Extrude,
            AvailableLpe::Knot,
        ]
    }

    /// The engine LPE kind string this effect maps to (matches
    /// `EditorEngine::normalize_lpe_kind`).
    pub fn kind(&self) -> &'static str {
        match self {
            AvailableLpe::FilletChamfer => "fillet",
            AvailableLpe::PowerStroke => "power_stroke",
            AvailableLpe::BendPath => "bend_path",
            AvailableLpe::PatternAlongPath => "pattern_along_path",
            AvailableLpe::Spiro => "spiro",
            AvailableLpe::Simplify => "simplify",
            AvailableLpe::Roughen => "roughen",
            AvailableLpe::MirrorSymmetry => "mirror",
            AvailableLpe::CopyRotate => "copy_rotate",
            AvailableLpe::BSpline => "bspline",
            AvailableLpe::Sketch => "sketch",
            AvailableLpe::Tiling => "tiling",
            AvailableLpe::Offset => "offset",
            AvailableLpe::Envelope => "envelope",
            AvailableLpe::PerspectiveEnvelope => "perspective_envelope",
            AvailableLpe::Lattice => "lattice",
            AvailableLpe::MeasureSegments => "measure",
            AvailableLpe::Gears => "gears",
            AvailableLpe::DashedStroke => "dashed_stroke",
            AvailableLpe::Extrude => "extrude",
            AvailableLpe::Knot => "knot",
        }
    }

    /// Human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            AvailableLpe::FilletChamfer => "Fillet / Chamfer",
            AvailableLpe::PowerStroke => "Power Stroke",
            AvailableLpe::BendPath => "Bend Path",
            AvailableLpe::PatternAlongPath => "Pattern Along Path",
            AvailableLpe::Spiro => "Spiro",
            AvailableLpe::Simplify => "Simplify",
            AvailableLpe::Roughen => "Roughen",
            AvailableLpe::MirrorSymmetry => "Mirror Symmetry",
            AvailableLpe::CopyRotate => "Copy & Rotate",
            AvailableLpe::BSpline => "B-Spline",
            AvailableLpe::Sketch => "Sketch",
            AvailableLpe::Tiling => "Tiling",
            AvailableLpe::Offset => "Offset",
            AvailableLpe::Envelope => "Envelope",
            AvailableLpe::PerspectiveEnvelope => "Perspective Envelope",
            AvailableLpe::Lattice => "Lattice",
            AvailableLpe::MeasureSegments => "Measure Segments",
            AvailableLpe::Gears => "Gears",
            AvailableLpe::DashedStroke => "Dashed Stroke",
            AvailableLpe::Extrude => "Extrude",
            AvailableLpe::Knot => "Knot",
        }
    }
}

/// The internal state of the LPE tool.
#[derive(Debug)]
#[allow(dead_code)]
enum LpeToolState {
    /// Waiting for the user to click on a path element.
    Idle,
    /// A path has been clicked; the user can now select which LPE to apply.
    Selecting {
        /// The node ID of the clicked path.
        node_id: NodeId,
        /// Document-space position of the click.
        click_pos: Point,
    },
    /// An LPE has been applied or selected; the user can adjust parameters.
    Editing {
        /// The node ID of the path being edited.
        node_id: NodeId,
        /// Which LPE is being edited.
        lpe_type: AvailableLpe,
        /// The index in the LPE stack (for existing effects).
        stack_index: usize,
    },
}

/// Tool for interactively applying and editing Live Path Effects.
#[derive(Debug)]
pub struct LpeTool {
    state: LpeToolState,
}

impl LpeTool {
    /// Create a new LPE tool in idle state.
    pub fn new() -> Self {
        Self {
            state: LpeToolState::Idle,
        }
    }

    /// Check whether the given node is a path element.
    #[allow(dead_code)]
    fn is_path_element(doc: &SvgDocument, node_id: NodeId) -> bool {
        doc.get_node(node_id)
            .map(|n| matches!(n.element, SvgElement::Path(_)))
            .unwrap_or(false)
    }

    /// Return the current state summary (useful for UI).
    pub fn state_name(&self) -> &str {
        match &self.state {
            LpeToolState::Idle => "idle",
            LpeToolState::Selecting { .. } => "selecting",
            LpeToolState::Editing { .. } => "editing",
        }
    }

    /// When in Selecting state, return the node ID of the selected path.
    pub fn selected_node(&self) -> Option<NodeId> {
        match &self.state {
            LpeToolState::Selecting { node_id, .. } => Some(*node_id),
            LpeToolState::Editing { node_id, .. } => Some(*node_id),
            LpeToolState::Idle => None,
        }
    }

    /// Apply an LPE type to the currently selected path, transitioning to
    /// Editing state and emitting a command that actually attaches the effect
    /// to the document (undoable).
    ///
    /// Returns [`ToolResponse::ExecuteCommand`] when a path is selected, or
    /// [`ToolResponse::None`] when the tool is idle (nothing to apply to).
    pub fn apply_lpe(&mut self, lpe_type: AvailableLpe) -> ToolResponse {
        let node_id = match &self.state {
            LpeToolState::Selecting { node_id, .. } | LpeToolState::Editing { node_id, .. } => {
                *node_id
            }
            LpeToolState::Idle => return ToolResponse::None,
        };
        self.state = LpeToolState::Editing {
            node_id,
            lpe_type,
            stack_index: 0,
        };
        ToolResponse::ExecuteCommand(Box::new(AddLpeCommand::new(node_id, lpe_type.kind())))
    }
}

impl Default for LpeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for LpeTool {
    fn name(&self) -> &str {
        "LPE"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        // Hit-test for a path element.
        if let Some(hit_id) = hit_test_path_at(doc, doc_pt) {
            match &self.state {
                LpeToolState::Idle => {
                    self.state = LpeToolState::Selecting {
                        node_id: hit_id,
                        click_pos: doc_pt,
                    };
                    ToolResponse::Handled
                }
                LpeToolState::Selecting { .. } => {
                    // Clicked on a (different) path -- re-select.
                    self.state = LpeToolState::Selecting {
                        node_id: hit_id,
                        click_pos: doc_pt,
                    };
                    ToolResponse::Handled
                }
                LpeToolState::Editing { .. } => {
                    // Clicked on a path while editing -- switch to that path.
                    self.state = LpeToolState::Selecting {
                        node_id: hit_id,
                        click_pos: doc_pt,
                    };
                    ToolResponse::Handled
                }
            }
        } else {
            // Clicked on empty space -- deselect / go idle.
            self.state = LpeToolState::Idle;
            ToolResponse::Handled
        }
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        // LPE tool doesn't do drag operations (parameter editing happens via
        // the parameter panel or keyboard shortcuts).
        ToolResponse::None
    }

    fn on_mouse_up(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        // Escape returns to idle.
        if event.key == Key::Escape {
            self.state = LpeToolState::Idle;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.state = LpeToolState::Idle;
    }

    fn on_deactivate(&mut self) {
        self.state = LpeToolState::Idle;
    }

    fn cursor(&self) -> &str {
        match &self.state {
            LpeToolState::Idle => "crosshair",
            LpeToolState::Selecting { .. } => "pointer",
            LpeToolState::Editing { .. } => "pointer",
        }
    }
}

// ---------------------------------------------------------------------------
// Hit-testing helper (path elements only)
// ---------------------------------------------------------------------------

/// Hit-test that only matches `<path>` elements. Skips locked/hidden objects.
pub(crate) fn hit_test_path_at(doc: &SvgDocument, point: Point) -> Option<NodeId> {
    let root = doc.root()?;
    let all = doc.traverse_pre_order(root);

    let mut result = None;
    for id in all {
        let node = match doc.get_node(id) {
            Some(n) => n,
            None => continue,
        };

        if node.locked || node.hidden {
            continue;
        }

        // Only match Path elements.
        if let SvgElement::Path(ref attrs) = node.element {
            let bb = attrs.d.bounding_box();
            if !bb.is_empty() && bb.contains_point(point) {
                result = Some(id);
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lpe_tool_name() {
        let tool = LpeTool::new();
        assert_eq!(tool.name(), "LPE");
    }

    #[test]
    fn test_lpe_tool_default_state() {
        let tool = LpeTool::new();
        assert_eq!(tool.state_name(), "idle");
        assert!(tool.selected_node().is_none());
    }

    #[test]
    fn test_lpe_tool_cursor_idle() {
        let tool = LpeTool::new();
        assert_eq!(tool.cursor(), "crosshair");
    }

    #[test]
    fn test_lpe_tool_on_activate() {
        let mut tool = LpeTool::new();
        tool.on_activate();
        assert_eq!(tool.state_name(), "idle");
    }

    #[test]
    fn test_lpe_tool_on_deactivate() {
        let mut tool = LpeTool::new();
        tool.on_deactivate();
        assert_eq!(tool.state_name(), "idle");
    }

    #[test]
    fn test_apply_lpe_fails_when_idle() {
        let mut tool = LpeTool::new();
        assert!(matches!(
            tool.apply_lpe(AvailableLpe::FilletChamfer),
            ToolResponse::None
        ));
    }

    #[test]
    fn test_apply_lpe_emits_command_when_selecting() {
        let mut tool = LpeTool::new();
        // Force the tool into Selecting state on a synthetic node.
        tool.state = LpeToolState::Selecting {
            node_id: NodeId::new(),
            click_pos: Point::new(0.0, 0.0),
        };
        let resp = tool.apply_lpe(AvailableLpe::Spiro);
        assert!(matches!(resp, ToolResponse::ExecuteCommand(_)));
        assert_eq!(tool.state_name(), "editing");
    }

    #[test]
    fn test_available_lpe_all() {
        let all = AvailableLpe::all();
        assert_eq!(all.len(), 21);
    }

    #[test]
    fn test_available_lpe_display_names() {
        assert_eq!(
            AvailableLpe::FilletChamfer.display_name(),
            "Fillet / Chamfer"
        );
        assert_eq!(AvailableLpe::BSpline.display_name(), "B-Spline");
        assert_eq!(AvailableLpe::Sketch.display_name(), "Sketch");
    }

    #[test]
    fn test_lpe_tool_escape_to_idle() {
        use crate::input::Modifiers;

        let mut tool = LpeTool::new();
        let doc = SvgDocument::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let event = KeyEvent {
            key: Key::Escape,
            modifiers: Modifiers::none(),
        };

        let resp = tool.on_key_down(event, &doc, &sel, &vp);
        assert!(matches!(resp, ToolResponse::Handled));
        assert_eq!(tool.state_name(), "idle");
    }

    #[test]
    fn test_lpe_tool_click_empty_space() {
        let mut tool = LpeTool::new();
        let doc = SvgDocument::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let event = MouseEvent::left(400.0, 300.0);

        let resp = tool.on_mouse_down(event, &doc, &sel, &vp);
        assert!(matches!(resp, ToolResponse::Handled));
        assert_eq!(tool.state_name(), "idle");
    }

    #[test]
    fn test_lpe_tool_right_click_ignored() {
        use crate::input::Modifiers;

        let mut tool = LpeTool::new();
        let doc = SvgDocument::new();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);

        let event = MouseEvent {
            x: 400.0,
            y: 300.0,
            button: MouseButton::Right,
            modifiers: Modifiers::none(),
        };

        let resp = tool.on_mouse_down(event, &doc, &sel, &vp);
        assert!(matches!(resp, ToolResponse::None));
    }
}
