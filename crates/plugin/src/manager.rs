use std::collections::{HashMap, HashSet};

use crate::actions::PluginAction;
use crate::api::{Plugin, PluginContext};
use crate::manifest::PluginManifest;

/// Central registry that owns all plugins and dispatches lifecycle hooks.
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
    enabled: HashMap<String, bool>,
}

impl PluginManager {
    /// Create an empty plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            enabled: HashMap::new(),
        }
    }

    /// Register a plugin. The plugin is enabled by default but `on_load` is
    /// **not** called automatically -- use [`load`](Self::load) after registration.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let id = plugin.manifest().id.clone();
        self.enabled.insert(id.clone(), true);
        self.plugins.insert(id, plugin);
    }

    /// Unregister and return a plugin. Returns `None` if the id is unknown.
    pub fn unregister(&mut self, id: &str) -> Option<Box<dyn Plugin>> {
        self.enabled.remove(id);
        self.plugins.remove(id)
    }

    /// Enable or disable a plugin.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if self.plugins.contains_key(id) {
            self.enabled.insert(id.to_string(), enabled);
        }
    }

    /// Check whether a plugin is currently enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }

    /// Look up a plugin by id (immutable).
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins.get(id).map(|p| p.as_ref())
    }

    /// Look up a plugin by id (mutable).
    pub fn get_mut<'a>(&'a mut self, id: &str) -> Option<&'a mut (dyn Plugin + 'static)> {
        self.plugins.get_mut(id).map(|p| p.as_mut())
    }

    /// Return a list of all registered plugins.
    pub fn list(&self) -> Vec<&dyn Plugin> {
        self.plugins.values().map(|p| p.as_ref()).collect()
    }

    /// Activate a single plugin by id. Returns the actions it produced,
    /// or an empty vec if the plugin is not found or disabled.
    pub fn activate(&mut self, id: &str, ctx: &PluginContext) -> Vec<PluginAction> {
        if !self.is_enabled(id) {
            return Vec::new();
        }
        match self.plugins.get_mut(id) {
            Some(plugin) => plugin.on_activate(ctx),
            None => Vec::new(),
        }
    }

    /// Notify every enabled plugin that the document has changed.
    /// Collects and returns all resulting actions.
    pub fn notify_document_change(&mut self, ctx: &PluginContext) -> Vec<PluginAction> {
        let enabled_ids: Vec<String> = self
            .plugins
            .keys()
            .filter(|id| self.is_enabled(id))
            .cloned()
            .collect();

        let mut actions = Vec::new();
        for id in &enabled_ids {
            if let Some(plugin) = self.plugins.get_mut(id) {
                actions.extend(plugin.on_document_change(ctx));
            }
        }
        actions
    }

    /// Notify every enabled plugin that the selection has changed.
    /// Collects and returns all resulting actions.
    pub fn notify_selection_change(&mut self, ctx: &PluginContext) -> Vec<PluginAction> {
        let enabled_ids: Vec<String> = self
            .plugins
            .keys()
            .filter(|id| self.is_enabled(id))
            .cloned()
            .collect();

        let mut actions = Vec::new();
        for id in &enabled_ids {
            if let Some(plugin) = self.plugins.get_mut(id) {
                actions.extend(plugin.on_selection_change(ctx));
            }
        }
        actions
    }

    /// Call `on_load` on a registered plugin.
    pub fn load(&mut self, id: &str) {
        if let Some(plugin) = self.plugins.get_mut(id) {
            log::info!("Loading plugin: {}", id);
            plugin.on_load();
        }
    }

    /// Call `on_unload` on a registered plugin.
    pub fn unload(&mut self, id: &str) {
        if let Some(plugin) = self.plugins.get_mut(id) {
            log::info!("Unloading plugin: {}", id);
            plugin.on_unload();
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PluginRegistry -- manifest-based install / uninstall lifecycle
// ---------------------------------------------------------------------------

/// Record for a plugin that has been *installed* on disk (as opposed to
/// merely registered at runtime).
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    /// Parsed manifest describing the plugin.
    pub manifest: PluginManifest,
    /// Filesystem path where the plugin is installed.
    pub path: String,
    /// Whether the plugin's code has been loaded into memory.
    pub loaded: bool,
}

/// Higher-level manager that tracks installed plugins by path and
/// controls their enabled / disabled state.
///
/// This complements [`PluginManager`] (which manages runtime trait
/// objects) by handling the *installation* side of the lifecycle: where
/// plugin bundles live on disk, which ones the user has enabled, etc.
#[derive(Debug)]
pub struct PluginRegistry {
    installed: Vec<InstalledPlugin>,
    enabled: HashSet<String>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            installed: Vec::new(),
            enabled: HashSet::new(),
        }
    }

    /// Install a plugin from `path`.
    ///
    /// In a full implementation this would read and validate a manifest
    /// file from the given path.  Here we synthesise a manifest from the
    /// path's leaf directory name so that the API is exercisable without
    /// real filesystem access.
    ///
    /// Returns the plugin id on success.
    pub fn install(&mut self, path: &str) -> Result<String, String> {
        // Derive a deterministic id from the path.
        let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);

        if leaf.is_empty() {
            return Err("path must not be empty".to_string());
        }

        let id = format!("installed.{leaf}");

        // Reject duplicates.
        if self.installed.iter().any(|p| p.manifest.id == id) {
            return Err(format!("plugin '{id}' is already installed"));
        }

        let manifest = PluginManifest {
            id: id.clone(),
            name: leaf.to_string(),
            version: "0.0.0".to_string(),
            author: String::new(),
            description: format!("Plugin installed from {path}"),
            permissions: Vec::new(),
        };

        self.installed.push(InstalledPlugin {
            manifest,
            path: path.to_string(),
            loaded: false,
        });

        self.enabled.insert(id.clone());
        Ok(id)
    }

    /// Uninstall a plugin by id. Returns `true` if it was found and
    /// removed.
    pub fn uninstall(&mut self, id: &str) -> bool {
        let before = self.installed.len();
        self.installed.retain(|p| p.manifest.id != id);
        self.enabled.remove(id);
        self.installed.len() < before
    }

    /// Mark a plugin as enabled.
    pub fn enable(&mut self, id: &str) {
        if self.installed.iter().any(|p| p.manifest.id == id) {
            self.enabled.insert(id.to_string());
        }
    }

    /// Mark a plugin as disabled.
    pub fn disable(&mut self, id: &str) {
        self.enabled.remove(id);
    }

    /// Check whether a plugin is currently enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(id)
    }

    /// Return a slice of all installed plugins.
    pub fn list(&self) -> &[InstalledPlugin] {
        &self.installed
    }

    /// Look up an installed plugin by id.
    pub fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.installed.iter().find(|p| p.manifest.id == id)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_install_and_list() {
        let mut reg = PluginRegistry::new();
        let id = reg.install("/plugins/my_filter").unwrap();
        assert_eq!(id, "installed.my_filter");
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get(&id).is_some());
        assert!(reg.is_enabled(&id));
    }

    #[test]
    fn registry_uninstall() {
        let mut reg = PluginRegistry::new();
        let id = reg.install("/plugins/removable").unwrap();
        assert!(reg.uninstall(&id));
        assert!(reg.list().is_empty());
        assert!(!reg.is_enabled(&id));
        // Uninstalling again returns false.
        assert!(!reg.uninstall(&id));
    }

    #[test]
    fn registry_enable_disable() {
        let mut reg = PluginRegistry::new();
        let id = reg.install("/plugins/toggle").unwrap();

        assert!(reg.is_enabled(&id));
        reg.disable(&id);
        assert!(!reg.is_enabled(&id));
        reg.enable(&id);
        assert!(reg.is_enabled(&id));
    }

    #[test]
    fn registry_duplicate_install_rejected() {
        let mut reg = PluginRegistry::new();
        reg.install("/plugins/dup").unwrap();
        let err = reg.install("/other/dup").unwrap_err();
        assert!(err.contains("already installed"));
    }
}
