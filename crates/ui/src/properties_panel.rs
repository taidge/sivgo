//! Fill & Stroke properties panel with paint type tabs, color picker,
//! stroke style controls, and marker selectors.

use dioxus::prelude::*;

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
#[component]
pub fn PropertiesPanel(
    // Fill state
    fill: String,
    fill_paint_type: String,
    fill_opacity: f64,
    // Stroke state
    stroke: String,
    stroke_paint_type: String,
    stroke_opacity: f64,
    stroke_width: f64,
    line_cap: String,
    line_join: String,
    dash_pattern: String,
    dash_offset: f64,
    // Markers
    marker_start: String,
    marker_mid: String,
    marker_end: String,
    // Selection
    has_selection: bool,
    on_action: EventHandler<PropertiesAction>,
) -> Element {
    let fill_preview_style = format!("background: {fill};");
    let stroke_preview_style = format!("background: {stroke};");

    if !has_selection {
        return rsx! {
            div { class: "properties-panel",
                div { class: "panel-header",
                    div { class: "panel-title-wrap",
                        span { class: "panel-kicker", "Inspector" }
                        h3 { class: "panel-title", "Fill & Stroke" }
                        p { class: "panel-meta", "Color, opacity, stroke profile, and marker controls for the active object." }
                    }
                    div { class: "panel-swatch-pair",
                        span { class: "panel-swatch", style: fill_preview_style.clone() }
                        span { class: "panel-swatch", style: stroke_preview_style.clone() }
                    }
                }
                div { class: "prop-empty",
                    span { class: "empty-title", "Select an object to inspect it" }
                    span { "The fill, stroke, and marker controls appear here as soon as the canvas has an active selection." }
                }
            }
        };
    }

    let fill_color = fill.clone();
    let stroke_color = stroke.clone();
    let cap_value = line_cap.clone();
    let join_value = line_join.clone();
    let dash_value = dash_pattern.clone();

    rsx! {
        div { class: "properties-panel",
            div { class: "panel-header",
                div { class: "panel-title-wrap",
                    span { class: "panel-kicker", "Inspector" }
                    h3 { class: "panel-title", "Fill & Stroke" }
                    p { class: "panel-meta", "Fine-tune paint, stroke weight, line endings, and SVG markers in one place." }
                }
                div { class: "panel-swatch-pair",
                    span { class: "panel-swatch", style: fill_preview_style }
                    span { class: "panel-swatch", style: stroke_preview_style }
                }
            }

            // ---- Fill Section ----
            div { class: "prop-section",
                div { class: "prop-section-header-row",
                    div { class: "prop-section-header", "Fill" }
                    span { class: "panel-badge", "Primary paint" }
                }

                // Paint type tabs
                div { class: "paint-type-tabs",
                    for pt in PaintType::all().iter() {
                        {
                            let pt_label = pt.label();
                            let is_active = fill_paint_type == pt_label;
                            let pt_label_owned = pt_label.to_string();
                            rsx! {
                                button {
                                    class: if is_active { "paint-tab active" } else { "paint-tab" },
                                    onclick: move |_| {
                                        let action = match pt_label {
                                            "None" => PropertiesAction::SetFillPaintType(PaintType::None),
                                            "Linear Gradient" => PropertiesAction::SetFillPaintType(PaintType::LinearGradient),
                                            "Radial Gradient" => PropertiesAction::SetFillPaintType(PaintType::RadialGradient),
                                            "Pattern" => PropertiesAction::SetFillPaintType(PaintType::Pattern),
                                            _ => PropertiesAction::SetFillPaintType(PaintType::FlatColor),
                                        };
                                        on_action.call(action);
                                    },
                                    "{pt_label_owned}"
                                }
                            }
                        }
                    }
                }

                // Color picker + hex input (shown when paint type is Flat Color)
                if fill_paint_type == "Flat Color" {
                    div { class: "prop-color-row",
                        input {
                            r#type: "color",
                            class: "prop-color",
                            value: "{fill_color}",
                            onchange: move |evt: Event<FormData>| {
                                on_action.call(PropertiesAction::SetFillColor(evt.value().to_string()));
                            },
                        }
                        input {
                            r#type: "text",
                            class: "prop-hex-input",
                            placeholder: "#rrggbb",
                            value: "{fill}",
                            onchange: move |evt: Event<FormData>| {
                                on_action.call(PropertiesAction::SetFillColor(evt.value().to_string()));
                            },
                        }
                    }
                }

                // Fill opacity
                div { class: "prop-row",
                    label { class: "prop-label", "Opacity" }
                    input {
                        r#type: "range",
                        class: "prop-slider",
                        min: "0",
                        max: "1",
                        step: "0.01",
                        value: "{fill_opacity}",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(PropertiesAction::SetFillOpacity(val));
                            }
                        },
                    }
                    span { class: "prop-value", "{(fill_opacity * 100.0) as u32}%" }
                }
            }

            // ---- Stroke Section ----
            div { class: "prop-section",
                div { class: "prop-section-header-row",
                    div { class: "prop-section-header", "Stroke" }
                    span { class: "panel-badge", "Edge profile" }
                }

                // Paint type tabs
                div { class: "paint-type-tabs",
                    for pt in PaintType::all().iter() {
                        {
                            let pt_label = pt.label();
                            let is_active = stroke_paint_type == pt_label;
                            let pt_label_owned = pt_label.to_string();
                            rsx! {
                                button {
                                    class: if is_active { "paint-tab active" } else { "paint-tab" },
                                    onclick: move |_| {
                                        let action = match pt_label {
                                            "None" => PropertiesAction::SetStrokePaintType(PaintType::None),
                                            "Linear Gradient" => PropertiesAction::SetStrokePaintType(PaintType::LinearGradient),
                                            "Radial Gradient" => PropertiesAction::SetStrokePaintType(PaintType::RadialGradient),
                                            "Pattern" => PropertiesAction::SetStrokePaintType(PaintType::Pattern),
                                            _ => PropertiesAction::SetStrokePaintType(PaintType::FlatColor),
                                        };
                                        on_action.call(action);
                                    },
                                    "{pt_label_owned}"
                                }
                            }
                        }
                    }
                }

                // Stroke color picker + hex input
                if stroke_paint_type == "Flat Color" {
                    div { class: "prop-color-row",
                        input {
                            r#type: "color",
                            class: "prop-color",
                            value: "{stroke_color}",
                            onchange: move |evt: Event<FormData>| {
                                on_action.call(PropertiesAction::SetStrokeColor(evt.value().to_string()));
                            },
                        }
                        input {
                            r#type: "text",
                            class: "prop-hex-input",
                            placeholder: "#rrggbb",
                            value: "{stroke}",
                            onchange: move |evt: Event<FormData>| {
                                on_action.call(PropertiesAction::SetStrokeColor(evt.value().to_string()));
                            },
                        }
                    }
                }

                // Stroke opacity
                div { class: "prop-row",
                    label { class: "prop-label", "Opacity" }
                    input {
                        r#type: "range",
                        class: "prop-slider",
                        min: "0",
                        max: "1",
                        step: "0.01",
                        value: "{stroke_opacity}",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(PropertiesAction::SetStrokeOpacity(val));
                            }
                        },
                    }
                    span { class: "prop-value", "{(stroke_opacity * 100.0) as u32}%" }
                }

                // Stroke width
                div { class: "prop-row",
                    label { class: "prop-label", "Width" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{stroke_width}",
                        min: "0",
                        max: "1000",
                        step: "0.5",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(PropertiesAction::SetStrokeWidth(val));
                            }
                        },
                    }
                }

                // Line cap
                div { class: "prop-row",
                    label { class: "prop-label", "Line Cap" }
                    select {
                        class: "prop-select",
                        value: "{cap_value}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetLineCap(evt.value().to_string()));
                        },
                        for cap in LineCapOption::all().iter() {
                            {
                                let cap_label = cap.label();
                                let is_sel = cap_value == cap_label;
                                rsx! {
                                    option {
                                        value: "{cap_label}",
                                        selected: is_sel,
                                        "{cap_label}"
                                    }
                                }
                            }
                        }
                    }
                }

                // Line join
                div { class: "prop-row",
                    label { class: "prop-label", "Line Join" }
                    select {
                        class: "prop-select",
                        value: "{join_value}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetLineJoin(evt.value().to_string()));
                        },
                        for join in LineJoinOption::all().iter() {
                            {
                                let join_label = join.label();
                                let is_sel = join_value == join_label;
                                rsx! {
                                    option {
                                        value: "{join_label}",
                                        selected: is_sel,
                                        "{join_label}"
                                    }
                                }
                            }
                        }
                    }
                }

                // Dash pattern
                div { class: "prop-row",
                    label { class: "prop-label", "Dash" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "e.g. 5,3,2",
                        value: "{dash_value}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetDashPattern(evt.value().to_string()));
                        },
                    }
                }

                // Dash offset
                div { class: "prop-row",
                    label { class: "prop-label", "Offset" }
                    input {
                        r#type: "number",
                        class: "prop-input",
                        value: "{dash_offset}",
                        step: "0.5",
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                on_action.call(PropertiesAction::SetDashOffset(val));
                            }
                        },
                    }
                }
            }

            // ---- Markers Section ----
            div { class: "prop-section",
                div { class: "prop-section-header-row",
                    div { class: "prop-section-header", "Markers" }
                    span { class: "panel-badge", "Directional cues" }
                }

                div { class: "prop-row",
                    label { class: "prop-label", "Start" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "url(#marker-id)",
                        value: "{marker_start}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetMarkerStart(evt.value().to_string()));
                        },
                    }
                }

                div { class: "prop-row",
                    label { class: "prop-label", "Mid" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "url(#marker-id)",
                        value: "{marker_mid}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetMarkerMid(evt.value().to_string()));
                        },
                    }
                }

                div { class: "prop-row",
                    label { class: "prop-label", "End" }
                    input {
                        r#type: "text",
                        class: "prop-input",
                        placeholder: "url(#marker-id)",
                        value: "{marker_end}",
                        onchange: move |evt: Event<FormData>| {
                            on_action.call(PropertiesAction::SetMarkerEnd(evt.value().to_string()));
                        },
                    }
                }
            }
        }
    }
}
