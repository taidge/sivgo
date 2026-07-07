//! Desktop-side integration of the `sivgo-plugin` extension system.
//!
//! Builds a [`PluginManager`] preloaded with the built-in plugins and exposes
//! thin helpers an extensions menu can call to list and activate plugins. This
//! is the wiring that makes the plugin subsystem reachable from the running
//! app (previously no crate depended on `sivgo-plugin` at all).

use sivgo_core::EditorEngine;
use sivgo_plugin::{GridGeneratorPlugin, HelloPlugin, PluginAction, PluginContext, PluginManager};

/// Build a [`PluginManager`] with the built-in plugins registered and loaded.
pub fn init_plugin_manager() -> PluginManager {
    let mut manager = PluginManager::new();
    manager.register(Box::new(HelloPlugin::new()));
    manager.register(Box::new(GridGeneratorPlugin::new(4, 4, 40.0, 8.0)));
    log::info!(
        "Plugin system initialized with {} built-in plugin(s)",
        manager.list().len()
    );
    manager
}

/// `(id, display name)` pairs for populating an extensions menu.
pub fn available_plugins(manager: &PluginManager) -> Vec<(String, String)> {
    manager
        .list()
        .iter()
        .map(|p| {
            let m = p.manifest();
            (m.id.clone(), m.name.clone())
        })
        .collect()
}

/// Activate the plugin with the given id against the current editor state,
/// returning the [`PluginAction`]s it requests (which the caller applies).
pub fn activate_plugin(
    manager: &mut PluginManager,
    id: &str,
    engine: &EditorEngine,
) -> Vec<PluginAction> {
    let ctx = PluginContext {
        document: &engine.document,
        selection: engine.selection.selected().to_vec(),
    };
    manager.activate(id, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_loaded_and_listable() {
        let manager = init_plugin_manager();
        let plugins = available_plugins(&manager);
        assert!(plugins.len() >= 2, "expected the two built-in plugins");
        assert!(plugins.iter().any(|(id, _)| id == "builtin.hello"));
    }

    #[test]
    fn activating_a_plugin_returns_actions() {
        let mut manager = init_plugin_manager();
        let engine = EditorEngine::new(800.0, 600.0);
        let actions = activate_plugin(&mut manager, "builtin.hello", &engine);
        assert!(!actions.is_empty(), "hello plugin should emit an action");
    }
}
