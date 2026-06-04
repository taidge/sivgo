//! Plugin registry for discovering and managing editor plugins.
//!
//! Maintains a catalogue of available plugins and tracks which plugins
//! each user has installed.  All state is held in memory.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata for a plugin in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// Thread-safe plugin registry.
///
/// Contains a global catalogue of available plugins and per-user
/// installation records.
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<Vec<PluginInfo>>>,
    installations: Arc<RwLock<HashMap<Uuid, HashSet<String>>>>,
}

impl PluginRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(Vec::new())),
            installations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a registry pre-populated with the given plugins.
    pub fn with_plugins(plugins: Vec<PluginInfo>) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(plugins)),
            installations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// List all plugins in the catalogue.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins.read().unwrap().clone()
    }

    /// Register a new plugin in the catalogue.
    pub fn register(&self, plugin: PluginInfo) {
        self.plugins.write().unwrap().push(plugin);
    }

    /// Install a plugin for a user. Returns `true` if the plugin exists and
    /// was not already installed for this user.
    pub fn install(&self, user_id: Uuid, plugin_id: &str) -> bool {
        // Verify the plugin exists.
        let exists = self
            .plugins
            .read()
            .unwrap()
            .iter()
            .any(|p| p.id == plugin_id);
        if !exists {
            return false;
        }

        let mut installs = self.installations.write().unwrap();
        let user_plugins = installs.entry(user_id).or_default();
        user_plugins.insert(plugin_id.to_string())
    }

    /// Uninstall a plugin for a user. Returns `true` if it was installed.
    pub fn uninstall(&self, user_id: Uuid, plugin_id: &str) -> bool {
        let mut installs = self.installations.write().unwrap();
        if let Some(user_plugins) = installs.get_mut(&user_id) {
            user_plugins.remove(plugin_id)
        } else {
            false
        }
    }

    /// List the plugins installed by a specific user.
    pub fn user_plugins(&self, user_id: Uuid) -> Vec<PluginInfo> {
        let installs = self.installations.read().unwrap();
        let installed_ids = match installs.get(&user_id) {
            Some(ids) => ids.clone(),
            None => return Vec::new(),
        };
        // Need to drop the installations lock before acquiring plugins lock.
        drop(installs);

        self.plugins
            .read()
            .unwrap()
            .iter()
            .filter(|p| installed_ids.contains(&p.id))
            .cloned()
            .collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plugins() -> Vec<PluginInfo> {
        vec![
            PluginInfo {
                id: "plugin-a".to_string(),
                name: "Plugin A".to_string(),
                version: "1.0.0".to_string(),
                description: "First plugin".to_string(),
                author: "Alice".to_string(),
            },
            PluginInfo {
                id: "plugin-b".to_string(),
                name: "Plugin B".to_string(),
                version: "2.1.0".to_string(),
                description: "Second plugin".to_string(),
                author: "Bob".to_string(),
            },
        ]
    }

    #[test]
    fn list_returns_all_plugins() {
        let registry = PluginRegistry::with_plugins(sample_plugins());
        let plugins = registry.list();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].id, "plugin-a");
        assert_eq!(plugins[1].id, "plugin-b");
    }

    #[test]
    fn install_and_user_plugins() {
        let registry = PluginRegistry::with_plugins(sample_plugins());
        let user_id = Uuid::new_v4();
        assert!(registry.install(user_id, "plugin-a"));
        // Installing again returns false.
        assert!(!registry.install(user_id, "plugin-a"));

        let user_plugins = registry.user_plugins(user_id);
        assert_eq!(user_plugins.len(), 1);
        assert_eq!(user_plugins[0].id, "plugin-a");
    }

    #[test]
    fn install_nonexistent_plugin_returns_false() {
        let registry = PluginRegistry::with_plugins(sample_plugins());
        assert!(!registry.install(Uuid::new_v4(), "plugin-nonexistent"));
    }

    #[test]
    fn uninstall_removes_plugin() {
        let registry = PluginRegistry::with_plugins(sample_plugins());
        let user_id = Uuid::new_v4();
        registry.install(user_id, "plugin-b");
        assert!(registry.uninstall(user_id, "plugin-b"));
        // Uninstalling again returns false.
        assert!(!registry.uninstall(user_id, "plugin-b"));

        let user_plugins = registry.user_plugins(user_id);
        assert!(user_plugins.is_empty());
    }
}
