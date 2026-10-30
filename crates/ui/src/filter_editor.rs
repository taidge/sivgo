//! Filter Editor panel for visually composing SVG filter chains.
//!
//! Provides a visual editor to add, remove, reorder, and configure
//! SVG filter primitives (feGaussianBlur, feOffset, feColorMatrix, etc.).

use crate::controls::{action_button, panel, text_field};
use glory::prelude::*;

/// Kind of SVG filter primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterPrimitiveKind {
    GaussianBlur,
    Offset,
    ColorMatrix,
    Blend,
    Composite,
    Morphology,
    Turbulence,
    DisplacementMap,
    ConvolveMatrix,
    Flood,
    Merge,
}

impl FilterPrimitiveKind {
    /// Stable key shared with the core engine.
    pub fn key(&self) -> &'static str {
        match self {
            Self::GaussianBlur => "gaussian_blur",
            Self::Offset => "offset",
            Self::ColorMatrix => "color_matrix",
            Self::Blend => "blend",
            Self::Composite => "composite",
            Self::Morphology => "morphology",
            Self::Turbulence => "turbulence",
            Self::DisplacementMap => "displacement_map",
            Self::ConvolveMatrix => "convolve_matrix",
            Self::Flood => "flood",
            Self::Merge => "merge",
        }
    }

    /// Create a UI kind from a stable core key.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "gaussian_blur" => Some(Self::GaussianBlur),
            "offset" => Some(Self::Offset),
            "color_matrix" => Some(Self::ColorMatrix),
            "blend" => Some(Self::Blend),
            "composite" => Some(Self::Composite),
            "morphology" => Some(Self::Morphology),
            "turbulence" => Some(Self::Turbulence),
            "displacement_map" => Some(Self::DisplacementMap),
            "convolve_matrix" => Some(Self::ConvolveMatrix),
            "flood" => Some(Self::Flood),
            "merge" => Some(Self::Merge),
            _ => None,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::GaussianBlur => "Gaussian Blur",
            Self::Offset => "Offset",
            Self::ColorMatrix => "Color Matrix",
            Self::Blend => "Blend",
            Self::Composite => "Composite",
            Self::Morphology => "Morphology",
            Self::Turbulence => "Turbulence",
            Self::DisplacementMap => "Displacement Map",
            Self::ConvolveMatrix => "Convolve Matrix",
            Self::Flood => "Flood",
            Self::Merge => "Merge",
        }
    }

    /// All available filter primitive kinds.
    pub fn all() -> &'static [FilterPrimitiveKind] {
        &[
            Self::GaussianBlur,
            Self::Offset,
            Self::ColorMatrix,
            Self::Blend,
            Self::Composite,
            Self::Morphology,
            Self::Turbulence,
            Self::DisplacementMap,
            Self::ConvolveMatrix,
            Self::Flood,
            Self::Merge,
        ]
    }

    /// Default editable parameter rows for this primitive.
    pub fn default_params(&self) -> Vec<FilterParam> {
        fn param(name: &str, value: &str) -> FilterParam {
            FilterParam {
                name: name.to_string(),
                value: value.to_string(),
            }
        }

        match self {
            Self::GaussianBlur => vec![
                param("input", "SourceGraphic"),
                param("stdDeviationX", "3"),
                param("stdDeviationY", "3"),
                param("result", "blur"),
            ],
            Self::Offset => vec![
                param("input", "SourceGraphic"),
                param("dx", "4"),
                param("dy", "4"),
                param("result", "offset"),
            ],
            Self::ColorMatrix => vec![
                param("input", "SourceGraphic"),
                param("type", "saturate"),
                param("values", "0"),
                param("result", "color"),
            ],
            Self::Blend => vec![
                param("input", "SourceGraphic"),
                param("in2", "SourceGraphic"),
                param("mode", "normal"),
                param("result", "blend"),
            ],
            Self::Composite => vec![
                param("input", "SourceGraphic"),
                param("in2", "SourceGraphic"),
                param("operator", "over"),
                param("k1", "0"),
                param("k2", "1"),
                param("k3", "1"),
                param("k4", "0"),
                param("result", "composite"),
            ],
            Self::Morphology => vec![
                param("input", "SourceAlpha"),
                param("operator", "dilate"),
                param("radiusX", "2"),
                param("radiusY", "2"),
                param("result", "morphology"),
            ],
            Self::Turbulence => vec![
                param("type", "turbulence"),
                param("baseFrequencyX", "0.05"),
                param("baseFrequencyY", "0.05"),
                param("numOctaves", "2"),
                param("seed", "0"),
                param("result", "noise"),
            ],
            Self::DisplacementMap => vec![
                param("input", "SourceGraphic"),
                param("in2", "noise"),
                param("scale", "10"),
                param("xChannelSelector", "R"),
                param("yChannelSelector", "G"),
                param("result", "displaced"),
            ],
            Self::ConvolveMatrix => vec![
                param("input", "SourceGraphic"),
                param("orderX", "3"),
                param("orderY", "3"),
                param("kernelMatrix", "0 -1 0 -1 5 -1 0 -1 0"),
                param("divisor", "1"),
                param("bias", "0"),
                param("result", "convolved"),
            ],
            Self::Flood => vec![
                param("flood-color", "#000000"),
                param("flood-opacity", "0.5"),
                param("result", "flood"),
            ],
            Self::Merge => vec![param("inputs", "SourceGraphic"), param("result", "merge")],
        }
    }
}

/// A key-value parameter for a filter primitive.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterParam {
    pub name: String,
    pub value: String,
}

/// A single filter primitive in the filter chain.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterPrimitive {
    /// Unique id within the filter chain.
    pub id: String,
    /// The kind of filter primitive.
    pub kind: FilterPrimitiveKind,
    /// Parameters for this primitive.
    pub params: Vec<FilterParam>,
    /// Whether this primitive is enabled.
    pub enabled: bool,
}

impl FilterPrimitive {
    /// Create a new enabled filter primitive with default params.
    pub fn new(id: impl Into<String>, kind: FilterPrimitiveKind) -> Self {
        let params = kind.default_params();
        Self {
            id: id.into(),
            kind,
            params,
            enabled: true,
        }
    }
}

/// Events emitted by the Filter Editor panel.
#[derive(Debug, Clone)]
pub enum FilterEditorAction {
    /// Add a new filter primitive of the given kind.
    AddPrimitive(FilterPrimitiveKind),
    /// Remove a filter primitive by id.
    RemovePrimitive(String),
    /// Move a primitive up in the chain.
    MoveUp(String),
    /// Move a primitive down in the chain.
    MoveDown(String),
    /// Toggle enable/disable of a primitive.
    ToggleEnabled(String),
    /// Set a parameter value on a primitive.
    SetParam(String, String, String),
    /// Select a primitive for editing.
    SelectPrimitive(String),
}

/// A visual editor for composing SVG filter chains from individual
/// filter primitives with per-primitive parameter controls.
#[derive(Clone)]
pub struct FilterEditorPanel {
    pub primitives: Vec<FilterPrimitive>,
    pub selected_id: Option<String>,
    pub on_action: crate::Callback<FilterEditorAction>,
}

impl Widget for FilterEditorPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "filter-editor", "Filter Editor")?;
        let selected_kind = scope.cage(
            FilterPrimitiveKind::all()
                .first()
                .map(|kind| kind.key().to_owned()),
        );
        crate::controls::choice_options(
            &scope,
            "Filter primitive",
            selected_kind.get().unwrap_or_default(),
            FilterPrimitiveKind::all()
                .iter()
                .map(|kind| (kind.key().to_owned(), kind.label().to_owned()))
                .collect(),
            move |value| selected_kind.revise(|current| *current = Some(value)),
        )?;
        let callback = self.on_action.clone();
        action_button(&scope, "Add primitive", false, move || {
            if let Some(kind) = selected_kind
                .get()
                .as_deref()
                .and_then(FilterPrimitiveKind::from_key)
            {
                callback.call(FilterEditorAction::AddPrimitive(kind));
            }
        })?;
        for (index, entry) in self.primitives.iter().enumerate() {
            let scope = panel(
                &scope,
                if self.selected_id.as_ref() == Some(&entry.id) {
                    "effect-card selected"
                } else {
                    "effect-card"
                },
                entry.kind.label(),
            )?;
            for (label, action) in [
                (
                    "Edit parameters",
                    FilterEditorAction::SelectPrimitive(entry.id.clone()),
                ),
                (
                    if entry.enabled { "Disable" } else { "Enable" },
                    FilterEditorAction::ToggleEnabled(entry.id.clone()),
                ),
                (
                    "Remove",
                    FilterEditorAction::RemovePrimitive(entry.id.clone()),
                ),
            ] {
                let callback = self.on_action.clone();
                action_button(&scope, label, false, move || callback.call(action.clone()))?;
            }
            let id = entry.id.clone();
            let callback = self.on_action.clone();
            sapid::button()
                .theme(crate::controls::editor_theme())
                .label("Move up")
                .enabled(index > 0)
                .on_press(move || callback.call(FilterEditorAction::MoveUp(id.clone())))
                .show_in(&scope)?;
            let id = entry.id.clone();
            let callback = self.on_action.clone();
            sapid::button()
                .theme(crate::controls::editor_theme())
                .label("Move down")
                .enabled(index + 1 < self.primitives.len())
                .on_press(move || callback.call(FilterEditorAction::MoveDown(id.clone())))
                .show_in(&scope)?;
            if self.selected_id.as_ref() == Some(&entry.id) {
                for param in &entry.params {
                    let callback = self.on_action.clone();
                    let id = entry.id.clone();
                    let name = param.name.clone();
                    text_field(&scope, &param.name, param.value.clone(), move |value| {
                        callback.call(FilterEditorAction::SetParam(
                            id.clone(),
                            name.clone(),
                            value,
                        ))
                    })?;
                }
                if entry.params.is_empty() {
                    text().content("No parameters").show_in(&scope)?;
                }
            }
        }
        if self.primitives.is_empty() {
            text()
                .content("No filter primitives applied")
                .show_in(&scope)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_primitive_new() {
        let prim = FilterPrimitive::new("f1", FilterPrimitiveKind::GaussianBlur);
        assert_eq!(prim.id, "f1");
        assert_eq!(prim.kind, FilterPrimitiveKind::GaussianBlur);
        assert_eq!(prim.params[1].name, "stdDeviationX");
        assert!(prim.enabled);
    }

    #[test]
    fn filter_primitive_kind_labels() {
        assert_eq!(FilterPrimitiveKind::GaussianBlur.label(), "Gaussian Blur");
        assert_eq!(FilterPrimitiveKind::ColorMatrix.label(), "Color Matrix");
        assert_eq!(FilterPrimitiveKind::Turbulence.label(), "Turbulence");
        assert_eq!(FilterPrimitiveKind::all().len(), 11);
    }

    #[test]
    fn filter_primitive_kind_keys_and_default_params() {
        assert_eq!(FilterPrimitiveKind::GaussianBlur.key(), "gaussian_blur");
        assert_eq!(
            FilterPrimitiveKind::from_key("displacement_map"),
            Some(FilterPrimitiveKind::DisplacementMap)
        );
        assert_eq!(
            FilterPrimitiveKind::Flood.default_params()[0].name,
            "flood-color"
        );
        assert_eq!(FilterPrimitiveKind::Composite.default_params().len(), 8);
    }
}
