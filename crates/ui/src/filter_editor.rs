//! Filter Editor panel for visually composing SVG filter chains.
//!
//! Provides a visual editor to add, remove, reorder, and configure
//! SVG filter primitives (feGaussianBlur, feOffset, feColorMatrix, etc.).

use dioxus::prelude::*;

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
#[component]
pub fn FilterEditorPanel(
    primitives: Vec<FilterPrimitive>,
    selected_id: Option<String>,
    on_action: EventHandler<FilterEditorAction>,
) -> Element {
    let all_kinds = FilterPrimitiveKind::all();

    rsx! {
        div { class: "filter-editor-panel",
            div { class: "panel-header", "Filter Editor" }

            // ---- Add primitive ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Add Filter" }
                div { class: "filter-add-buttons",
                    for kind in all_kinds.iter() {
                        {
                            let k = kind.clone();
                            rsx! {
                                button {
                                    class: "action-btn filter-add-btn",
                                    title: "Add {kind.label()}",
                                    onclick: move |_| on_action.call(FilterEditorAction::AddPrimitive(k.clone())),
                                    "{kind.label()}"
                                }
                            }
                        }
                    }
                }
            }

            // ---- Filter chain ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Filter Chain" }
                div { class: "filter-chain-list",
                    for (idx, prim) in primitives.iter().enumerate() {
                        {
                            let prim_id = prim.id.clone();
                            let prim_id_up = prim.id.clone();
                            let prim_id_down = prim.id.clone();
                            let prim_id_del = prim.id.clone();
                            let prim_id_toggle = prim.id.clone();
                            let is_selected = selected_id.as_deref() == Some(prim.id.as_str());
                            let is_first = idx == 0;
                            let is_last = idx == primitives.len() - 1;
                            rsx! {
                                div {
                                    key: "{prim.id}",
                                    class: if is_selected { "filter-prim-item selected" } else { "filter-prim-item" },
                                    onclick: move |_| on_action.call(FilterEditorAction::SelectPrimitive(prim_id.clone())),

                                    span {
                                        class: if prim.enabled { "filter-prim-label" } else { "filter-prim-label disabled" },
                                        "{prim.kind.label()}"
                                    }

                                    button {
                                        class: "filter-btn",
                                        title: if prim.enabled { "Disable" } else { "Enable" },
                                        onclick: move |_| on_action.call(FilterEditorAction::ToggleEnabled(prim_id_toggle.clone())),
                                        if prim.enabled { "On" } else { "Off" }
                                    }
                                    button {
                                        class: "filter-btn",
                                        title: "Move up",
                                        disabled: is_first,
                                        onclick: move |_| on_action.call(FilterEditorAction::MoveUp(prim_id_up.clone())),
                                        "Up"
                                    }
                                    button {
                                        class: "filter-btn",
                                        title: "Move down",
                                        disabled: is_last,
                                        onclick: move |_| on_action.call(FilterEditorAction::MoveDown(prim_id_down.clone())),
                                        "Dn"
                                    }
                                    button {
                                        class: "filter-btn delete",
                                        title: "Remove",
                                        onclick: move |_| on_action.call(FilterEditorAction::RemovePrimitive(prim_id_del.clone())),
                                        "X"
                                    }
                                }
                            }
                        }
                    }
                    if primitives.is_empty() {
                        div { class: "filter-chain-empty", "No filters" }
                    }
                }
            }

            // ---- Selected primitive parameters ----
            if let Some(ref sel_id) = selected_id {
                {
                    if let Some(selected) = primitives.iter().find(|p| &p.id == sel_id) {
                        let params = selected.params.clone();
                        let sel_prim_id = sel_id.clone();
                        rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Parameters: {selected.kind.label()}" }
                                for param in params.iter() {
                                    {
                                        let pid = sel_prim_id.clone();
                                        let pname = param.name.clone();
                                        let pval = param.value.clone();
                                        rsx! {
                                            div { class: "prop-row",
                                                label { class: "prop-label", "{pname}" }
                                                input {
                                                    r#type: "text",
                                                    class: "prop-input",
                                                    value: "{pval}",
                                                    onchange: move |evt: Event<FormData>| {
                                                        on_action.call(FilterEditorAction::SetParam(
                                                            pid.clone(),
                                                            pname.clone(),
                                                            evt.value().to_string(),
                                                        ));
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                                if params.is_empty() {
                                    div { class: "prop-empty", "No parameters" }
                                }
                            }
                        }
                    } else {
                        rsx! { }
                    }
                }
            }
        }
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
