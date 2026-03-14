//! Preferences dialog with categorised settings sections:
//! Tools, Interface, I/O, Rendering, and Snapping.

use dioxus::prelude::*;

/// Top-level section within the preferences dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferencesSection {
    Tools,
    Interface,
    IO,
    Rendering,
    Snapping,
}

impl PreferencesSection {
    /// All available sections in display order.
    pub fn all() -> &'static [PreferencesSection] {
        &[
            Self::Tools,
            Self::Interface,
            Self::IO,
            Self::Rendering,
            Self::Snapping,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Interface => "Interface",
            Self::IO => "I/O",
            Self::Rendering => "Rendering",
            Self::Snapping => "Snapping",
        }
    }
}

/// Aggregate preferences state.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    pub section: PreferencesSection,
    // Tools
    pub default_shape_tool: String,
    pub bounding_box_display: bool,
    // Interface
    pub theme: String,
    pub icon_size: String,
    pub show_tooltips: bool,
    // I/O
    pub auto_save: bool,
    pub auto_save_interval_secs: u32,
    pub recent_files_count: u32,
    // Rendering
    pub anti_aliasing: bool,
    pub display_filter_quality: String,
    pub canvas_border: bool,
    // Snapping
    pub snap_to_nodes: bool,
    pub snap_to_paths: bool,
    pub snap_to_grid: bool,
    pub snap_distance: f64,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            section: PreferencesSection::Tools,
            default_shape_tool: "Rectangle".to_string(),
            bounding_box_display: true,
            theme: "Dark".to_string(),
            icon_size: "Medium".to_string(),
            show_tooltips: true,
            auto_save: true,
            auto_save_interval_secs: 300,
            recent_files_count: 10,
            anti_aliasing: true,
            display_filter_quality: "High".to_string(),
            canvas_border: true,
            snap_to_nodes: true,
            snap_to_paths: true,
            snap_to_grid: true,
            snap_distance: 10.0,
        }
    }
}

/// Events emitted by the Preferences dialog.
#[derive(Debug, Clone)]
pub enum PreferencesAction {
    SetSection(PreferencesSection),
    // Tools
    SetDefaultShapeTool(String),
    SetBoundingBoxDisplay(bool),
    // Interface
    SetTheme(String),
    SetIconSize(String),
    SetShowTooltips(bool),
    // I/O
    SetAutoSave(bool),
    SetAutoSaveInterval(u32),
    SetRecentFilesCount(u32),
    // Rendering
    SetAntiAliasing(bool),
    SetDisplayFilterQuality(String),
    SetCanvasBorder(bool),
    // Snapping
    SetSnapToNodes(bool),
    SetSnapToPaths(bool),
    SetSnapToGrid(bool),
    SetSnapDistance(f64),
    /// Close the dialog.
    Close,
}

/// A dialog for editing application-wide preferences organised into
/// sections: Tools, Interface, I/O, Rendering, and Snapping.
#[component]
pub fn PreferencesDialog(
    state: PreferencesState,
    on_action: EventHandler<PreferencesAction>,
) -> Element {
    let sections = PreferencesSection::all();
    let themes = vec!["Dark", "Light", "System"];
    let icon_sizes = vec!["Small", "Medium", "Large"];
    let filter_qualities = vec!["Low", "Medium", "High"];

    let theme_val = state.theme.clone();
    let icon_val = state.icon_size.clone();
    let filter_val = state.display_filter_quality.clone();
    let shape_val = state.default_shape_tool.clone();

    rsx! {
        div { class: "preferences-dialog",
            div { class: "panel-header",
                span { "Preferences" }
                button {
                    class: "dialog-close-btn",
                    title: "Close",
                    onclick: move |_| on_action.call(PreferencesAction::Close),
                    "X"
                }
            }

            div { class: "preferences-layout",
                // Section sidebar
                div { class: "preferences-sidebar",
                    for sec in sections.iter() {
                        {
                            let is_active = state.section == *sec;
                            let sec_clone = sec.clone();
                            rsx! {
                                button {
                                    class: if is_active { "pref-section-btn active" } else { "pref-section-btn" },
                                    onclick: move |_| on_action.call(PreferencesAction::SetSection(sec_clone.clone())),
                                    "{sec.label()}"
                                }
                            }
                        }
                    }
                }

                // Section content
                div { class: "preferences-content",
                    match state.section {
                        PreferencesSection::Tools => rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Tools" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Default shape" }
                                    input {
                                        r#type: "text",
                                        class: "prop-input",
                                        value: "{shape_val}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetDefaultShapeTool(evt.value().to_string()));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Bounding box" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.bounding_box_display,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetBoundingBoxDisplay(evt.value() == "true"));
                                        },
                                    }
                                }
                            }
                        },
                        PreferencesSection::Interface => rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Interface" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Theme" }
                                    select {
                                        class: "prop-select",
                                        value: "{theme_val}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetTheme(evt.value().to_string()));
                                        },
                                        for t in themes.iter() {
                                            {
                                                let is_sel = theme_val == *t;
                                                rsx! { option { value: "{t}", selected: is_sel, "{t}" } }
                                            }
                                        }
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Icon size" }
                                    select {
                                        class: "prop-select",
                                        value: "{icon_val}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetIconSize(evt.value().to_string()));
                                        },
                                        for s in icon_sizes.iter() {
                                            {
                                                let is_sel = icon_val == *s;
                                                rsx! { option { value: "{s}", selected: is_sel, "{s}" } }
                                            }
                                        }
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Tooltips" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.show_tooltips,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetShowTooltips(evt.value() == "true"));
                                        },
                                    }
                                }
                            }
                        },
                        PreferencesSection::IO => rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "I/O" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Auto-save" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.auto_save,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetAutoSave(evt.value() == "true"));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Interval (s)" }
                                    input {
                                        r#type: "number",
                                        class: "prop-input",
                                        value: "{state.auto_save_interval_secs}",
                                        min: "10",
                                        step: "10",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<u32>() {
                                                on_action.call(PreferencesAction::SetAutoSaveInterval(val));
                                            }
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Recent files" }
                                    input {
                                        r#type: "number",
                                        class: "prop-input",
                                        value: "{state.recent_files_count}",
                                        min: "0",
                                        max: "50",
                                        step: "1",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<u32>() {
                                                on_action.call(PreferencesAction::SetRecentFilesCount(val));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        PreferencesSection::Rendering => rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Rendering" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Anti-aliasing" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.anti_aliasing,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetAntiAliasing(evt.value() == "true"));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Filter quality" }
                                    select {
                                        class: "prop-select",
                                        value: "{filter_val}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetDisplayFilterQuality(evt.value().to_string()));
                                        },
                                        for q in filter_qualities.iter() {
                                            {
                                                let is_sel = filter_val == *q;
                                                rsx! { option { value: "{q}", selected: is_sel, "{q}" } }
                                            }
                                        }
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Canvas border" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.canvas_border,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetCanvasBorder(evt.value() == "true"));
                                        },
                                    }
                                }
                            }
                        },
                        PreferencesSection::Snapping => rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Snapping" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Snap to nodes" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.snap_to_nodes,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetSnapToNodes(evt.value() == "true"));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Snap to paths" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.snap_to_paths,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetSnapToPaths(evt.value() == "true"));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Snap to grid" }
                                    input {
                                        r#type: "checkbox",
                                        class: "prop-checkbox",
                                        checked: state.snap_to_grid,
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(PreferencesAction::SetSnapToGrid(evt.value() == "true"));
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Distance" }
                                    input {
                                        r#type: "number",
                                        class: "prop-input",
                                        value: "{state.snap_distance}",
                                        min: "1",
                                        step: "1",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<f64>() {
                                                on_action.call(PreferencesAction::SetSnapDistance(val));
                                            }
                                        },
                                    }
                                    span { class: "prop-unit", "px" }
                                }
                            }
                        },
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
    fn preferences_state_default() {
        let state = PreferencesState::default();
        assert_eq!(state.section, PreferencesSection::Tools);
        assert!(state.auto_save);
        assert_eq!(state.auto_save_interval_secs, 300);
        assert!(state.anti_aliasing);
        assert!(state.snap_to_nodes);
        assert!((state.snap_distance - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn section_labels() {
        assert_eq!(PreferencesSection::Tools.label(), "Tools");
        assert_eq!(PreferencesSection::IO.label(), "I/O");
        assert_eq!(PreferencesSection::Snapping.label(), "Snapping");
        assert_eq!(PreferencesSection::all().len(), 5);
    }
}
