//! Fill & Stroke properties panel with paint type tabs, color picker,
//! stroke style controls, and marker selectors.

use crate::{controls::*, Callback};
use glory::prelude::*;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Describes the type of paint applied to fill or stroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintType {
    None,
    FlatColor,
    LinearGradient,
    RadialGradient,
    Pattern,
}

impl PaintType {
    pub fn label(&self) -> &'static str {
        match self {
            PaintType::None => "None",
            PaintType::FlatColor => "Flat Color",
            PaintType::LinearGradient => "Linear Gradient",
            PaintType::RadialGradient => "Radial Gradient",
            PaintType::Pattern => "Pattern",
        }
    }

    pub fn all() -> &'static [PaintType] {
        &[
            PaintType::None,
            PaintType::FlatColor,
            PaintType::LinearGradient,
            PaintType::RadialGradient,
            PaintType::Pattern,
        ]
    }
}

/// Line cap options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineCapOption {
    Butt,
    Round,
    Square,
}

impl LineCapOption {
    pub fn label(&self) -> &'static str {
        match self {
            LineCapOption::Butt => "Butt",
            LineCapOption::Round => "Round",
            LineCapOption::Square => "Square",
        }
    }

    pub fn all() -> &'static [LineCapOption] {
        &[
            LineCapOption::Butt,
            LineCapOption::Round,
            LineCapOption::Square,
        ]
    }
}

/// Line join options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineJoinOption {
    Miter,
    Round,
    Bevel,
}

impl LineJoinOption {
    pub fn label(&self) -> &'static str {
        match self {
            LineJoinOption::Miter => "Miter",
            LineJoinOption::Round => "Round",
            LineJoinOption::Bevel => "Bevel",
        }
    }

    pub fn all() -> &'static [LineJoinOption] {
        &[
            LineJoinOption::Miter,
            LineJoinOption::Round,
            LineJoinOption::Bevel,
        ]
    }
}

/// Events emitted by the properties panel.
#[derive(Debug, Clone)]
pub enum PropertiesAction {
    // Fill
    SetFillPaintType(PaintType),
    SetFillColor(String),
    SetFillOpacity(f64),

    // Stroke
    SetStrokePaintType(PaintType),
    SetStrokeColor(String),
    SetStrokeOpacity(f64),
    SetStrokeWidth(f64),
    SetLineCap(String),
    SetLineJoin(String),
    SetDashPattern(String),
    SetDashOffset(f64),

    // Markers
    SetMarkerStart(String),
    SetMarkerMid(String),
    SetMarkerEnd(String),
}

/// A panel displaying and editing the fill/stroke properties of the
/// currently selected element.
///
/// Features paint type tabs, hex color input, opacity and stroke style
/// controls, and marker selectors.
pub struct PropertiesPanel {
    pub fill: String,
    pub fill_paint_type: String,
    pub fill_opacity: f64,
    pub stroke: String,
    pub stroke_paint_type: String,
    pub stroke_opacity: f64,
    pub stroke_width: f64,
    pub line_cap: String,
    pub line_join: String,
    pub dash_pattern: String,
    pub dash_offset: f64,
    pub marker_start: String,
    pub marker_mid: String,
    pub marker_end: String,
    pub has_selection: bool,
    pub on_action: Callback<PropertiesAction>,
}

impl Widget for PropertiesPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let root = panel(scope, "properties-panel", "Fill & Stroke")?;
        if !self.has_selection {
            return text()
                .content("Select an object to edit its properties.")
                .show_in(&root);
        }
        for (is_fill, current, color, opacity) in [
            (true, &self.fill_paint_type, &self.fill, self.fill_opacity),
            (
                false,
                &self.stroke_paint_type,
                &self.stroke,
                self.stroke_opacity,
            ),
        ] {
            let name = if is_fill { "Fill" } else { "Stroke" };
            let section = panel(&root, "paint-section", name)?;
            let callback = self.on_action.clone();
            choice_field(
                &section,
                &format!("{name} paint"),
                current.clone(),
                &[
                    "None",
                    "Flat Color",
                    "Linear Gradient",
                    "Radial Gradient",
                    "Pattern",
                ],
                move |value| {
                    if let Some(paint) =
                        PaintType::all().iter().find(|paint| paint.label() == value)
                    {
                        callback.call(if is_fill {
                            PropertiesAction::SetFillPaintType(paint.clone())
                        } else {
                            PropertiesAction::SetStrokePaintType(paint.clone())
                        });
                    }
                },
            )?;
            let callback = self.on_action.clone();
            color_field(
                &section,
                &format!("{name} color"),
                color.clone(),
                move |value| {
                    callback.call(if is_fill {
                        PropertiesAction::SetFillColor(value)
                    } else {
                        PropertiesAction::SetStrokeColor(value)
                    });
                },
            )?;
            let callback = self.on_action.clone();
            number_field(
                &section,
                &format!("{name} opacity"),
                opacity,
                move |value| {
                    let value = value.clamp(0.0, 1.0);
                    callback.call(if is_fill {
                        PropertiesAction::SetFillOpacity(value)
                    } else {
                        PropertiesAction::SetStrokeOpacity(value)
                    });
                },
            )?;
        }
        let stroke = panel(&root, "stroke-section", "Stroke style")?;
        let callback = self.on_action.clone();
        number_field(&stroke, "Stroke width", self.stroke_width, move |value| {
            callback.call(PropertiesAction::SetStrokeWidth(value.max(0.0)));
        })?;
        let callback = self.on_action.clone();
        choice_field(
            &stroke,
            "Line cap",
            self.line_cap.clone(),
            &["Butt", "Round", "Square"],
            move |value| callback.call(PropertiesAction::SetLineCap(value)),
        )?;
        let callback = self.on_action.clone();
        choice_field(
            &stroke,
            "Line join",
            self.line_join.clone(),
            &["Miter", "Round", "Bevel"],
            move |value| callback.call(PropertiesAction::SetLineJoin(value)),
        )?;
        let callback = self.on_action.clone();
        text_field(
            &stroke,
            "Dash pattern",
            self.dash_pattern.clone(),
            move |value| callback.call(PropertiesAction::SetDashPattern(value)),
        )?;
        let callback = self.on_action.clone();
        number_field(&stroke, "Dash offset", self.dash_offset, move |value| {
            callback.call(PropertiesAction::SetDashOffset(value))
        })?;
        let markers = panel(&root, "marker-section", "Markers")?;
        for (label, value, action) in [
            (
                "Start marker",
                self.marker_start.clone(),
                PropertiesAction::SetMarkerStart as fn(String) -> PropertiesAction,
            ),
            (
                "Middle marker",
                self.marker_mid.clone(),
                PropertiesAction::SetMarkerMid,
            ),
            (
                "End marker",
                self.marker_end.clone(),
                PropertiesAction::SetMarkerEnd,
            ),
        ] {
            let callback = self.on_action.clone();
            text_field(&markers, label, value, move |value| {
                callback.call(action(value))
            })?;
        }
        Ok(())
    }
}
