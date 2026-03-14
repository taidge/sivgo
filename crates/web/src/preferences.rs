//! User preferences synchronisation.
//!
//! Stores per-user editor preferences in memory so they can be synced
//! across devices.  Supports full replacement and partial (merge) updates.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

/// The full set of user-configurable editor preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub theme: String,
    pub default_unit: String,
    pub grid_visible: bool,
    pub guides_visible: bool,
    pub snap_enabled: bool,
    pub autosave_interval: u64,
    pub recent_colors: Vec<String>,
    pub custom_shortcuts: Vec<(String, String)>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            default_unit: "px".to_string(),
            grid_visible: true,
            guides_visible: true,
            snap_enabled: true,
            autosave_interval: 30,
            recent_colors: Vec::new(),
            custom_shortcuts: Vec::new(),
        }
    }
}

/// Thread-safe, in-memory preferences store keyed by user id.
#[derive(Debug, Clone)]
pub struct PreferencesStore {
    prefs: Arc<RwLock<HashMap<String, UserPreferences>>>,
}

impl PreferencesStore {
    /// Create a new, empty preferences store.
    pub fn new() -> Self {
        Self {
            prefs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the preferences for a user, or `None` if not yet stored.
    pub fn get(&self, user_id: &str) -> Option<UserPreferences> {
        self.prefs.read().unwrap().get(user_id).cloned()
    }

    /// Set (replace) the full preferences for a user.
    pub fn set(&self, user_id: &str, prefs: UserPreferences) {
        self.prefs
            .write()
            .unwrap()
            .insert(user_id.to_string(), prefs);
    }

    /// Merge a partial JSON value into the existing preferences for a user.
    ///
    /// Only the keys present in `partial` are updated; the rest remain
    /// unchanged.  If the user has no stored preferences yet, the defaults
    /// are used as the base.
    pub fn merge(&self, user_id: &str, partial: serde_json::Value) {
        let mut store = self.prefs.write().unwrap();
        let current = store.get(user_id).cloned().unwrap_or_default();

        // Serialize current to JSON, merge, then deserialize back.
        let mut base = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
        if let (Some(base_obj), Some(partial_obj)) = (base.as_object_mut(), partial.as_object()) {
            for (key, value) in partial_obj {
                base_obj.insert(key.clone(), value.clone());
            }
        }

        if let Ok(merged) = serde_json::from_value::<UserPreferences>(base) {
            store.insert(user_id.to_string(), merged);
        }
    }
}

impl Default for PreferencesStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_unknown_user() {
        let store = PreferencesStore::new();
        assert!(store.get("unknown").is_none());
    }

    #[test]
    fn set_and_get_roundtrip() {
        let store = PreferencesStore::new();
        let prefs = UserPreferences {
            theme: "dark".to_string(),
            default_unit: "mm".to_string(),
            grid_visible: false,
            guides_visible: true,
            snap_enabled: false,
            autosave_interval: 60,
            recent_colors: vec!["#ff0000".to_string()],
            custom_shortcuts: vec![("Ctrl+S".to_string(), "save".to_string())],
        };
        store.set("user-1", prefs.clone());

        let got = store.get("user-1").expect("should exist");
        assert_eq!(got.theme, "dark");
        assert_eq!(got.default_unit, "mm");
        assert!(!got.grid_visible);
        assert_eq!(got.autosave_interval, 60);
        assert_eq!(got.recent_colors, vec!["#ff0000".to_string()]);
    }

    #[test]
    fn merge_updates_only_specified_fields() {
        let store = PreferencesStore::new();
        // Start with defaults.
        store.set("user-1", UserPreferences::default());

        let partial = serde_json::json!({
            "theme": "dark",
            "autosave_interval": 120
        });
        store.merge("user-1", partial);

        let got = store.get("user-1").unwrap();
        assert_eq!(got.theme, "dark");
        assert_eq!(got.autosave_interval, 120);
        // Unchanged fields keep their defaults.
        assert!(got.grid_visible);
        assert_eq!(got.default_unit, "px");
    }
}
