//! Text Properties panel with font family, size, weight, style,
//! line height, spacing, and alignment controls.

use crate::{controls::*, Callback};
use glory::prelude::*;

/// Events emitted by the text properties panel.
#[derive(Debug, Clone)]
pub enum TextAction {
    SetTextContent(String),
    SetFontFamily(String),
    SetFontSize(f64),
    SetFontWeight(String),
    SetFontStyle(String),
    SetLineHeight(f64),
    SetLetterSpacing(f64),
    SetWordSpacing(f64),
    SetTextAlign(String),
}

/// A panel for editing text-specific properties of selected text elements.
///
/// Provides controls for font family, size, weight, style, line height,
/// letter spacing, word spacing, and text alignment.
pub struct TextPanel {
    pub text_content: String,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: String,
    pub font_style: String,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub word_spacing: f64,
    pub text_align: String,
    pub has_text_selection: bool,
    pub on_action: Callback<TextAction>,
}

impl Widget for TextPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let panel = panel(scope, "text-panel", "Text")?;
        if !self.has_text_selection {
            return text().content("No text selected").show_in(&panel);
        }
        let callback = self.on_action.clone();
        text().content("Content").show_in(&panel)?;
        sapid::textarea()
            .theme(crate::controls::editor_theme())
            .name("Text content")
            .default_value(self.text_content.clone())
            .on_commit(move |v| callback.call(TextAction::SetTextContent(v)))
            .show_in(&panel)?;
        let callback = self.on_action.clone();
        text_field(&panel, "Font family", self.font_family.clone(), move |v| {
            callback.call(TextAction::SetFontFamily(v))
        })?;
        for (label, value, action) in [
            (
                "Font size",
                self.font_size,
                TextAction::SetFontSize as fn(f64) -> TextAction,
            ),
            ("Line height", self.line_height, TextAction::SetLineHeight),
            (
                "Letter spacing",
                self.letter_spacing,
                TextAction::SetLetterSpacing,
            ),
            (
                "Word spacing",
                self.word_spacing,
                TextAction::SetWordSpacing,
            ),
        ] {
            let callback = self.on_action.clone();
            number_field(&panel, label, value, move |v| callback.call(action(v)))?;
        }
        for (label, value, options, action) in [
            (
                "Font weight",
                self.font_weight.clone(),
                &[
                    "Normal", "Bold", "100", "200", "300", "400", "500", "600", "700", "800", "900",
                ][..],
                TextAction::SetFontWeight as fn(String) -> TextAction,
            ),
            (
                "Font style",
                self.font_style.clone(),
                &["Normal", "Italic", "Oblique"][..],
                TextAction::SetFontStyle,
            ),
            (
                "Text alignment",
                self.text_align.clone(),
                &["Start", "Middle", "End"][..],
                TextAction::SetTextAlign,
            ),
        ] {
            let callback = self.on_action.clone();
            choice_field(&panel, label, value, options, move |v| {
                callback.call(action(v))
            })?;
        }
        Ok(())
    }
}
