//! Live Path Effects (LPE) panel for adding, editing, reordering,
//! and removing path effects applied to selected objects.

use crate::controls::{action_button, panel, text_field};
use glory::prelude::*;

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
#[derive(Clone)]
pub struct LpePanel {
    pub effects: Vec<LpeEntry>,
    pub selected_id: Option<String>,
    pub has_selection: bool,
    pub on_action: crate::Callback<LpeAction>,
}

impl Widget for LpePanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "lpe-panel", "Live Path Effects")?;
        if !self.has_selection {
            text()
                .content("Select a path to edit its live effects.")
                .show_in(&scope)?;
            return Ok(());
        }
        let selected_kind = scope.cage(LpeKind::all().first().map(|kind| kind.key().to_owned()));
        crate::controls::choice_options(
            &scope,
            "Effect",
            selected_kind.get().unwrap_or_default(),
            LpeKind::all()
                .iter()
                .map(|kind| (kind.key().to_owned(), kind.label().to_owned()))
                .collect(),
            move |value| selected_kind.revise(|current| *current = Some(value)),
        )?;
        let callback = self.on_action.clone();
        action_button(&scope, "Add effect", false, move || {
            if let Some(kind) = selected_kind.get().as_deref().and_then(LpeKind::from_key) {
                callback.call(LpeAction::Add(kind));
            }
        })?;
        for (index, entry) in self.effects.iter().enumerate() {
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
                ("Edit parameters", LpeAction::Select(entry.id.clone())),
                (
                    if entry.enabled { "Disable" } else { "Enable" },
                    LpeAction::ToggleEnabled(entry.id.clone()),
                ),
                ("Remove", LpeAction::Remove(entry.id.clone())),
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
                .on_press(move || callback.call(LpeAction::MoveUp(id.clone())))
                .show_in(&scope)?;
            let id = entry.id.clone();
            let callback = self.on_action.clone();
            sapid::button()
                .theme(crate::controls::editor_theme())
                .label("Move down")
                .enabled(index + 1 < self.effects.len())
                .on_press(move || callback.call(LpeAction::MoveDown(id.clone())))
                .show_in(&scope)?;
            if self.selected_id.as_ref() == Some(&entry.id) {
                for param in &entry.params {
                    let callback = self.on_action.clone();
                    let id = entry.id.clone();
                    let name = param.name.clone();
                    text_field(&scope, &param.name, param.value.clone(), move |value| {
                        callback.call(LpeAction::SetParam(id.clone(), name.clone(), value))
                    })?;
                }
                if entry.params.is_empty() {
                    text().content("No parameters").show_in(&scope)?;
                }
            }
        }
        if self.effects.is_empty() {
            text().content("No effects applied").show_in(&scope)?;
        }
        Ok(())
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
