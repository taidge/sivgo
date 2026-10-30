//! Transform panel with move, scale, rotate, and skew controls.

use crate::{controls::*, Callback};
use glory::prelude::*;

/// Events emitted by the transform panel.
#[derive(Debug, Clone)]
pub enum TransformAction {
    SetX(f64),
    SetY(f64),
    SetWidth(f64),
    SetHeight(f64),
    SetScalePercent(f64, f64),
    ToggleLockAspect,
    TogglePercentMode,
    SetRotation(f64),
    SetSkewX(f64),
    SetSkewY(f64),
}

/// A panel for controlling position, scale, rotation, and skew of selected
/// objects.
///
/// Displays numeric inputs for X/Y position, W/H dimensions (with optional
/// percentage mode and aspect-ratio lock), rotation angle, and skew values.
pub struct TransformPanel {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub lock_aspect: bool,
    pub percent_mode: bool,
    pub has_selection: bool,
    pub on_action: Callback<TransformAction>,
}

impl Widget for TransformPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let panel = panel(scope, "transform-panel", "Transform")?;
        if !self.has_selection {
            return text().content("No selection").show_in(&panel);
        }
        for (label, value, action) in [
            (
                "X",
                self.x,
                TransformAction::SetX as fn(f64) -> TransformAction,
            ),
            ("Y", self.y, TransformAction::SetY),
            ("Rotation", self.rotation, TransformAction::SetRotation),
            ("Horizontal skew", self.skew_x, TransformAction::SetSkewX),
            ("Vertical skew", self.skew_y, TransformAction::SetSkewY),
        ] {
            let callback = self.on_action.clone();
            number_field(&panel, label, value, move |v| callback.call(action(v)))?;
        }
        for (label, value, horizontal) in
            [("Width", self.width, true), ("Height", self.height, false)]
        {
            let callback = self.on_action.clone();
            let percent = self.percent_mode;
            number_field(
                &panel,
                if percent {
                    if horizontal {
                        "Width %"
                    } else {
                        "Height %"
                    }
                } else {
                    label
                },
                value,
                move |v| {
                    if v < 0.0 {
                        return;
                    }
                    callback.call(if percent {
                        if horizontal {
                            TransformAction::SetScalePercent(v, 0.0)
                        } else {
                            TransformAction::SetScalePercent(0.0, v)
                        }
                    } else if horizontal {
                        TransformAction::SetWidth(v)
                    } else {
                        TransformAction::SetHeight(v)
                    });
                },
            )?;
        }
        let callback = self.on_action.clone();
        action_button(&panel, "Lock aspect ratio", self.lock_aspect, move || {
            callback.call(TransformAction::ToggleLockAspect)
        })?;
        let callback = self.on_action.clone();
        action_button(&panel, "Percentage mode", self.percent_mode, move || {
            callback.call(TransformAction::TogglePercentMode)
        })
    }
}
