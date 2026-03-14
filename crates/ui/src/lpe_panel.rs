//! Live Path Effects (LPE) panel for adding, editing, reordering,
//! and removing path effects applied to selected objects.

use dioxus::prelude::*;

/// Kind of Live Path Effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LpeKind {
    BendPath,
    PatternAlongPath,
    StitchSubPaths,
    Offset,
    Simplify,
    PowerStroke,
    Roughen,
    Mirror,
    BooleanOperation,
    Measure,
}

impl LpeKind {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BendPath => "Bend Path",
            Self::PatternAlongPath => "Pattern Along Path",
            Self::StitchSubPaths => "Stitch Sub-Paths",
            Self::Offset => "Offset",
            Self::Simplify => "Simplify",
            Self::PowerStroke => "Power Stroke",
            Self::Roughen => "Roughen",
            Self::Mirror => "Mirror",
            Self::BooleanOperation => "Boolean Operation",
            Self::Measure => "Measure",
        }
    }

    /// All available LPE kinds.
    pub fn all() -> &'static [LpeKind] {
        &[
            Self::BendPath,
            Self::PatternAlongPath,
            Self::StitchSubPaths,
            Self::Offset,
            Self::Simplify,
            Self::PowerStroke,
            Self::Roughen,
            Self::Mirror,
            Self::BooleanOperation,
            Self::Measure,
        ]
    }
}

/// A key-value parameter for an LPE.
#[derive(Debug, Clone, PartialEq)]
pub struct LpeParam {
    pub name: String,
    pub value: String,
}

/// A single LPE applied to the selected path.
#[derive(Debug, Clone, PartialEq)]
pub struct LpeEntry {
    /// Unique identifier.
    pub id: String,
    /// The kind of LPE.
    pub kind: LpeKind,
    /// Parameters for this LPE.
    pub params: Vec<LpeParam>,
    /// Whether this LPE is enabled.
    pub enabled: bool,
}

impl LpeEntry {
    /// Create a new enabled LPE entry with no parameters.
    pub fn new(id: impl Into<String>, kind: LpeKind) -> Self {
        Self {
            id: id.into(),
            kind,
            params: Vec::new(),
            enabled: true,
        }
    }
}

/// Events emitted by the LPE panel.
#[derive(Debug, Clone)]
pub enum LpeAction {
    /// Add a new LPE of the given kind.
    Add(LpeKind),
    /// Remove an LPE by id.
    Remove(String),
    /// Move an LPE up in the stack.
    MoveUp(String),
    /// Move an LPE down in the stack.
    MoveDown(String),
    /// Toggle enable/disable of an LPE.
    ToggleEnabled(String),
    /// Set a parameter value on an LPE.
    SetParam(String, String, String),
    /// Select an LPE for parameter editing.
    Select(String),
}

/// A panel for managing Live Path Effects on selected path objects.
#[component]
pub fn LpePanel(
    effects: Vec<LpeEntry>,
    selected_id: Option<String>,
    has_selection: bool,
    on_action: EventHandler<LpeAction>,
) -> Element {
    if !has_selection {
        return rsx! {
            div { class: "lpe-panel",
                div { class: "panel-header", "Live Path Effects" }
                div { class: "prop-empty", "No path selected" }
            }
        };
    }

    let all_kinds = LpeKind::all();

    rsx! {
        div { class: "lpe-panel",
            div { class: "panel-header", "Live Path Effects" }

            // ---- Add LPE ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Add Effect" }
                div { class: "lpe-add-buttons",
                    for kind in all_kinds.iter() {
                        {
                            let k = kind.clone();
                            rsx! {
                                button {
                                    class: "action-btn lpe-add-btn",
                                    title: "Add {kind.label()}",
                                    onclick: move |_| on_action.call(LpeAction::Add(k.clone())),
                                    "{kind.label()}"
                                }
                            }
                        }
                    }
                }
            }

            // ---- Effect stack ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Effect Stack" }
                div { class: "lpe-list",
                    for (idx, effect) in effects.iter().enumerate() {
                        {
                            let eid = effect.id.clone();
                            let eid_up = effect.id.clone();
                            let eid_down = effect.id.clone();
                            let eid_del = effect.id.clone();
                            let eid_toggle = effect.id.clone();
                            let is_selected = selected_id.as_deref() == Some(effect.id.as_str());
                            let is_first = idx == 0;
                            let is_last = idx == effects.len() - 1;
                            rsx! {
                                div {
                                    class: if is_selected { "lpe-item selected" } else { "lpe-item" },
                                    onclick: move |_| on_action.call(LpeAction::Select(eid.clone())),

                                    span {
                                        class: if effect.enabled { "lpe-label" } else { "lpe-label disabled" },
                                        "{effect.kind.label()}"
                                    }

                                    button {
                                        class: "lpe-btn",
                                        title: if effect.enabled { "Disable" } else { "Enable" },
                                        onclick: move |_| on_action.call(LpeAction::ToggleEnabled(eid_toggle.clone())),
                                        if effect.enabled { "On" } else { "Off" }
                                    }
                                    button {
                                        class: "lpe-btn",
                                        title: "Move up",
                                        disabled: is_first,
                                        onclick: move |_| on_action.call(LpeAction::MoveUp(eid_up.clone())),
                                        "Up"
                                    }
                                    button {
                                        class: "lpe-btn",
                                        title: "Move down",
                                        disabled: is_last,
                                        onclick: move |_| on_action.call(LpeAction::MoveDown(eid_down.clone())),
                                        "Dn"
                                    }
                                    button {
                                        class: "lpe-btn delete",
                                        title: "Remove",
                                        onclick: move |_| on_action.call(LpeAction::Remove(eid_del.clone())),
                                        "X"
                                    }
                                }
                            }
                        }
                    }
                    if effects.is_empty() {
                        div { class: "lpe-list-empty", "No effects applied" }
                    }
                }
            }

            // ---- Selected LPE parameters ----
            if let Some(ref sel_id) = selected_id {
                {
                    if let Some(selected) = effects.iter().find(|e| &e.id == sel_id) {
                        let params = selected.params.clone();
                        let sel_eid = sel_id.clone();
                        rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Parameters: {selected.kind.label()}" }
                                for param in params.iter() {
                                    {
                                        let eid = sel_eid.clone();
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
                                                        on_action.call(LpeAction::SetParam(
                                                            eid.clone(),
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
    fn lpe_entry_new() {
        let lpe = LpeEntry::new("lpe1", LpeKind::BendPath);
        assert_eq!(lpe.id, "lpe1");
        assert_eq!(lpe.kind, LpeKind::BendPath);
        assert!(lpe.params.is_empty());
        assert!(lpe.enabled);
    }

    #[test]
    fn lpe_kind_labels_and_count() {
        assert_eq!(LpeKind::BendPath.label(), "Bend Path");
        assert_eq!(LpeKind::PowerStroke.label(), "Power Stroke");
        assert_eq!(LpeKind::BooleanOperation.label(), "Boolean Operation");
        assert_eq!(LpeKind::all().len(), 10);
    }
}
