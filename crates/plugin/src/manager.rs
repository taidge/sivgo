use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Install a plugin from a directory containing `plugin.json` or
    /// `manifest.json`. The manifest is parsed and validated before the
    /// plugin is registered.
    pub fn install(&mut self, path: &str) -> Result<String, String> {
        let manifest_path = find_manifest_path(path)?;
        let json = fs::read_to_string(&manifest_path).map_err(|err| {
            format!(
                "failed to read manifest '{}': {err}",
                manifest_path.display()
            )
        })?;
        let manifest: PluginManifest = serde_json::from_str(&json).map_err(|err| {
            format!(
                "failed to parse manifest '{}': {err}",
                manifest_path.display()
            )
        })?;
        self.install_manifest(path, manifest)
    }

    /// Install a plugin from an already parsed manifest.
    pub fn install_manifest(
        &mut self,
        path: impl Into<String>,
        manifest: PluginManifest,
    ) -> Result<String, String> {
        manifest.validate()?;
        let path = path.into();
        let id = manifest.id.clone();

        if self.installed.iter().any(|p| p.manifest.id == id) {
            return Err(format!("plugin '{id}' is already installed"));
        }

        self.installed.push(InstalledPlugin {
            manifest,
            path,
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

fn find_manifest_path(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("path must not be empty".to_string());
    }
    let base = Path::new(path);
    let candidates = [base.join("plugin.json"), base.join("manifest.json")];
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("no plugin manifest found under '{path}'"))
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
    use crate::manifest::Permission;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Tester".to_string(),
            description: "A test plugin".to_string(),
            permissions: vec![Permission::ReadDocument],
        }
    }

    fn temp_plugin_dir(id: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sivgo-plugin-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let json = serde_json::to_string(&manifest(id)).unwrap();
        fs::write(dir.join("plugin.json"), json).unwrap();
        dir
    }

    #[test]
    fn registry_install_and_list() {
        let mut reg = PluginRegistry::new();
        let dir = temp_plugin_dir("ext.my_filter");
        let id = reg.install(dir.to_str().unwrap()).unwrap();
        assert_eq!(id, "ext.my_filter");
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get(&id).is_some());
        assert!(reg.is_enabled(&id));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn registry_uninstall() {
        let mut reg = PluginRegistry::new();
        let id = reg
            .install_manifest("/plugins/removable", manifest("ext.removable"))
            .unwrap();
        assert!(reg.uninstall(&id));
        assert!(reg.list().is_empty());
        assert!(!reg.is_enabled(&id));
        // Uninstalling again returns false.
        assert!(!reg.uninstall(&id));
    }

    #[test]
    fn registry_enable_disable() {
        let mut reg = PluginRegistry::new();
        let id = reg
            .install_manifest("/plugins/toggle", manifest("ext.toggle"))
            .unwrap();

        assert!(reg.is_enabled(&id));
        reg.disable(&id);
        assert!(!reg.is_enabled(&id));
        reg.enable(&id);
        assert!(reg.is_enabled(&id));
    }

    #[test]
    fn registry_duplicate_install_rejected() {
        let mut reg = PluginRegistry::new();
        reg.install_manifest("/plugins/dup", manifest("ext.dup"))
            .unwrap();
        let err = reg
            .install_manifest("/other/dup", manifest("ext.dup"))
            .unwrap_err();
        assert!(err.contains("already installed"));
    }

    #[test]
    fn registry_install_rejects_missing_or_invalid_manifest() {
        let mut reg = PluginRegistry::new();
        assert!(reg.install("").unwrap_err().contains("path"));
        assert!(reg
            .install("/definitely/missing")
            .unwrap_err()
            .contains("manifest"));

        let mut invalid = manifest("bad id");
        invalid.version = "1".to_string();
        let err = reg
            .install_manifest("/plugins/invalid", invalid)
            .unwrap_err();
        assert!(err.contains("id") || err.contains("semantic"));
    }
}
