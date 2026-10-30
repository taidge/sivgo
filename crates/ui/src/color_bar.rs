//! Color indicator bar component showing quick fill/stroke color display
//! with click-to-edit functionality.

use crate::controls::{action_button, panel};
use glory::prelude::*;

/// A compact color indicator bar showing the current fill and stroke colors.
///
/// Displays two overlapping color swatches (fill in front, stroke behind)
/// and a "none" indicator. Clicking on the fill or stroke swatch opens
/// an edit interaction via the corresponding event handler.
#[derive(Clone)]
pub struct ColorBar {
    pub fill_color: String,
    pub stroke_color: String,
    pub fill_none: bool,
    pub stroke_none: bool,
    pub on_fill_click: crate::Callback<()>,
    pub on_stroke_click: crate::Callback<()>,
    pub on_set_none: crate::Callback<()>,
}

impl Widget for ColorBar {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "color-bar", "")?;
        for (name, color, none, callback) in [
            (
                "Fill",
                &self.fill_color,
                self.fill_none,
                self.on_fill_click.clone(),
            ),
            (
                "Stroke",
                &self.stroke_color,
                self.stroke_none,
                self.on_stroke_click.clone(),
            ),
        ] {
            let label = format!(
                "{name}: {}",
                if none || color.is_empty() {
                    "none"
                } else {
                    color
                }
            );
            let color = if none {
                sivgo_dom::Color::TRANSPARENT
            } else {
                sivgo_dom::Color::parse(color).unwrap_or(sivgo_dom::Color::TRANSPARENT)
            };
            sapid::color_swatch(glory::style::Color(color.r, color.g, color.b, color.a))
                .name(name)
                .side(24.0)
                .on_press(move || callback.call(()))
                .show_in(&scope)?;
            text().content(label).show_in(&scope)?;
        }
        let callback = self.on_set_none.clone();
        action_button(&scope, "No paint", false, move || callback.call(()))?;
        Ok(())
    }
}
