//! Dynamic tool-controls bar that renders context-sensitive options for the active tool.
//!
//! Each tool type has its own set of controls (e.g., the Select tool shows X/Y/W/H
//! spin-boxes, the Pen tool shows smoothing options, etc.).

use crate::{controls::*, Callback};
use glory::prelude::*;
use sivgo_core::ToolId;

// ---------------------------------------------------------------------------
// Action enum emitted by tool controls
// ---------------------------------------------------------------------------

/// Actions that tool-controls can emit to the parent.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolControlAction {
    // -- Select tool --
    SetX(f64),
    SetY(f64),
    SetWidth(f64),
    SetHeight(f64),

    // -- Node tool --
    SetNodeType(NodeType),
    SetSegmentType(SegmentType),

    // -- Shape tools (Rect, Ellipse, Star, Spiral, 3D Box) --
    SetShapeWidth(f64),
    SetShapeHeight(f64),
    SetCornerRadius(f64),
    SetStarCorners(u32),
    SetStarRatio(f64),
    SetSpiralTurns(f64),

    // -- Pen / Pencil --
    SetSmoothing(f64),
    SetShape(PenShape),

    // -- Text --
    SetFontFamily(String),
    SetFontSize(f64),
    ToggleBold,
    ToggleItalic,

    // -- Gradient --
    SetGradientType(GradientKind),
    AddStop,
    RemoveStop,

    // -- Snapping --
    ToggleSnapToGrid,
    ToggleSnapToGuides,
    ToggleSnapToObjects,
    ToggleSnapToPaths,
}

/// Node type for the Node editing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Cusp,
    Smooth,
    Symmetric,
    Auto,
}

/// Path-segment type for the Node editing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Line,
    Curve,
}

/// Pen stroke shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenShape {
    None,
    Triangle,
    FromClipboard,
    BendFromClipboard,
}

/// Gradient variant selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

/// A horizontal bar that shows different controls depending on the active tool.
///
/// The parent supplies the current [`ToolId`] and initial values. Every user
/// interaction emits a [`ToolControlAction`] through `on_action`.
pub struct ToolControls {
    pub active_tool: ToolId,
    pub on_action: Callback<ToolControlAction>,
    pub sel_x: f64,
    pub sel_y: f64,
    pub sel_w: f64,
    pub sel_h: f64,
    pub snap_grid: bool,
    pub snap_guides: bool,
    pub snap_objects: bool,
    pub snap_paths: bool,
}

impl Widget for ToolControls {
    fn build(&mut self, scope: &Scope) -> Result {
        use ToolControlAction as A;
        type NumericControl = (&'static str, f64, fn(f64) -> A);
        let panel = panel(scope, "tool-controls", "")?;
        let mut numbers: Vec<NumericControl> = Vec::new();
        let mut actions: Vec<(&str, A, bool)> = Vec::new();
        match self.active_tool {
            ToolId::Select => numbers.extend([
                ("X", self.sel_x, A::SetX as fn(f64) -> A),
                ("Y", self.sel_y, A::SetY),
                ("Width", self.sel_w, A::SetWidth),
                ("Height", self.sel_h, A::SetHeight),
            ]),
            ToolId::Node => actions.extend([
                ("Cusp", A::SetNodeType(NodeType::Cusp), false),
                ("Smooth", A::SetNodeType(NodeType::Smooth), false),
                ("Symmetric", A::SetNodeType(NodeType::Symmetric), false),
                ("Auto smooth", A::SetNodeType(NodeType::Auto), false),
                ("Line segment", A::SetSegmentType(SegmentType::Line), false),
                (
                    "Curve segment",
                    A::SetSegmentType(SegmentType::Curve),
                    false,
                ),
            ]),
            ToolId::Rect | ToolId::Box3d | ToolId::Ellipse => {
                numbers.extend([
                    ("Shape width", self.sel_w, A::SetShapeWidth as fn(f64) -> A),
                    ("Shape height", self.sel_h, A::SetShapeHeight),
                ]);
                if self.active_tool == ToolId::Rect {
                    numbers.push(("Corner radius", 0.0, A::SetCornerRadius));
                }
            }
            ToolId::Star => {
                let callback = self.on_action.clone();
                compact_number_field(&panel, "Star corners", 5.0, move |v| {
                    callback.call(A::SetStarCorners(v.round().clamp(3.0, 100.0) as u32))
                })?;
                numbers.push(("Spoke ratio", 0.5, A::SetStarRatio));
            }
            ToolId::Spiral => numbers.push(("Spiral turns", 3.0, A::SetSpiralTurns)),
            ToolId::Pen | ToolId::Pencil => {
                numbers.push(("Smoothing", 50.0, A::SetSmoothing));
                actions.extend([
                    ("No shape", A::SetShape(PenShape::None), false),
                    ("Triangle", A::SetShape(PenShape::Triangle), false),
                    (
                        "From clipboard",
                        A::SetShape(PenShape::FromClipboard),
                        false,
                    ),
                    (
                        "Bend from clipboard",
                        A::SetShape(PenShape::BendFromClipboard),
                        false,
                    ),
                ]);
            }
            ToolId::Text => {
                let callback = self.on_action.clone();
                text_field(&panel, "Font family", "sans-serif".into(), move |v| {
                    callback.call(A::SetFontFamily(v))
                })?;
                numbers.push(("Font size", 16.0, A::SetFontSize));
                actions.extend([
                    ("Bold", A::ToggleBold, false),
                    ("Italic", A::ToggleItalic, false),
                ]);
            }
            ToolId::Gradient => actions.extend([
                (
                    "Linear gradient",
                    A::SetGradientType(GradientKind::Linear),
                    false,
                ),
                (
                    "Radial gradient",
                    A::SetGradientType(GradientKind::Radial),
                    false,
                ),
                ("Add stop", A::AddStop, false),
                ("Remove stop", A::RemoveStop, false),
            ]),
            _ => {}
        }
        for (label, value, action) in numbers {
            let callback = self.on_action.clone();
            compact_number_field(&panel, label, value, move |v| callback.call(action(v)))?;
        }
        actions.extend([
            ("Snap to grid", A::ToggleSnapToGrid, self.snap_grid),
            ("Snap to guides", A::ToggleSnapToGuides, self.snap_guides),
            ("Snap to objects", A::ToggleSnapToObjects, self.snap_objects),
            ("Snap to paths", A::ToggleSnapToPaths, self.snap_paths),
        ]);
        for (label, action, active) in actions {
            let callback = self.on_action.clone();
            action_button(&panel, label, active, move || callback.call(action.clone()))?;
        }
        Ok(())
    }
}
