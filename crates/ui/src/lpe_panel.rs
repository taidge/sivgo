//! Live Path Effects (LPE) panel for adding, editing, reordering,
//! and removing path effects applied to selected objects.

use dioxus::prelude::*;

/// Kind of Live Path Effect.
///
/// Holds the engine's canonical key rather than a fixed enum. The panel used
/// to model only a subset, and any effect outside that subset was dropped from
/// the list the panel was given — invisible to the user and impossible to
/// remove once applied. Taking the key straight from the engine keeps the two
/// lists from drifting apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LpeKind {
    key: String,
}

impl LpeKind {
    /// Stable key shared with the core engine.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Wrap a key the engine recognises.
    ///
    /// Returns `None` for an unknown key so a malformed document cannot put an
    /// unusable row in the panel.
    pub fn from_key(key: &str) -> Option<Self> {
        // Aliases such as "dash" resolve to the canonical "dashed_stroke", so
        // two spellings of the same effect compare equal.
        let label = sivgo_core::EditorEngine::lpe_kind_label(key)?;
        let canonical = sivgo_core::EditorEngine::available_lpe_kinds()
            .iter()
            .find(|(_, l)| *l == label)
            .map(|(k, _)| *k)?;
        Some(Self {
            key: canonical.to_string(),
        })
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        sivgo_core::EditorEngine::lpe_kind_label(&self.key).unwrap_or("Unknown Effect")
    }

    /// Every effect the engine can apply.
    pub fn all() -> Vec<LpeKind> {
        sivgo_core::EditorEngine::available_lpe_kinds()
            .iter()
            .map(|(key, _)| LpeKind {
                key: (*key).to_string(),
            })
            .collect()
    }

    /// Default editable parameter rows for this effect kind.
    pub fn default_params(&self) -> Vec<LpeParam> {
        sivgo_core::EditorEngine::lpe_kind_default_params(&self.key)
            .into_iter()
            .map(|(name, value)| LpeParam { name, value })
            .collect()
    }
}

/// A single editable parameter of an effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LpeParam {
    /// Parameter name as the engine knows it.
    pub name: String,
    /// Current value, formatted for display and editing.
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
    /// Create a new enabled LPE entry with that kind's default parameters.
    pub fn new(id: impl Into<String>, kind: LpeKind) -> Self {
        let params = kind.default_params();
        Self {
            id: id.into(),
            kind,
            params,
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
                                    key: "{effect.id}",
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

    fn kind(key: &str) -> LpeKind {
        LpeKind::from_key(key).unwrap_or_else(|| panic!("{key} should be a known effect"))
    }

    #[test]
    fn lpe_entry_new() {
        let lpe = LpeEntry::new("lpe1", kind("bend_path"));
        assert_eq!(lpe.id, "lpe1");
        assert_eq!(lpe.kind.key(), "bend_path");
        assert!(lpe.enabled);
        assert!(
            !lpe.params.is_empty(),
            "an effect should start with its default parameters"
        );
    }

    #[test]
    fn labels_come_from_the_engine() {
        assert_eq!(kind("bend_path").label(), "Bend Path");
        assert_eq!(kind("power_stroke").label(), "Power Stroke");
        assert_eq!(kind("von_koch").label(), "Von Koch Fractal");
    }

    #[test]
    fn the_panel_offers_every_effect_the_engine_supports() {
        // The panel used to model only ten of them and silently drop the rest.
        let offered = LpeKind::all();
        assert_eq!(
            offered.len(),
            sivgo_core::EditorEngine::available_lpe_kinds().len()
        );
        assert!(offered.len() >= 20, "expected the full effect list");

        for k in &offered {
            assert_eq!(
                LpeKind::from_key(k.key()).as_ref(),
                Some(k),
                "{} should round-trip through its key",
                k.key()
            );
            assert_ne!(k.label(), "Unknown Effect", "{} needs a label", k.key());
        }
    }

    #[test]
    fn aliases_resolve_to_the_canonical_key() {
        assert_eq!(kind("dash").key(), "dashed_stroke");
        assert_eq!(kind("Mirror Symmetry").key(), "mirror");
        assert_eq!(kind("chamfer").key(), "fillet");
    }

    #[test]
    fn an_unknown_key_is_refused() {
        assert!(LpeKind::from_key("not-an-effect").is_none());
    }

    #[test]
    fn default_params_match_what_the_engine_will_store() {
        for k in LpeKind::all() {
            let from_panel: Vec<String> = k.default_params().into_iter().map(|p| p.name).collect();
            let from_engine: Vec<String> =
                sivgo_core::EditorEngine::lpe_kind_default_params(k.key())
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();
            assert_eq!(from_panel, from_engine, "parameters differ for {}", k.key());
        }
    }
}
