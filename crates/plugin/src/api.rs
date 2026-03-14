use crate::actions::PluginAction;
use crate::manifest::PluginManifest;
use sivgo_dom::{NodeId, SvgDocument};

/// Read-only context that the host passes to every plugin callback.
///
/// Plugins use this to inspect the current document state and selection
/// without holding a mutable reference to the document.
pub struct PluginContext<'a> {
    /// The current SVG document.
    pub document: &'a SvgDocument,
    /// Node ids that are currently selected in the editor.
    pub selection: Vec<NodeId>,
}

/// The core extension point.
///
/// Every plugin (native Rust, future WASM, future Lua) ultimately
/// implements this trait. The manager invokes the lifecycle hooks at the
/// appropriate moments.
pub trait Plugin: std::fmt::Debug + Send {
    /// Return the manifest that describes this plugin.
    fn manifest(&self) -> &PluginManifest;

    /// Called once after the plugin is registered and loaded.
    fn on_load(&mut self) {
        // Default: no-op.
    }

    /// Called when the plugin is about to be unloaded.
    fn on_unload(&mut self) {
        // Default: no-op.
    }

    /// Called when the user explicitly activates the plugin
    /// (e.g. via a menu item or shortcut).
    fn on_activate(&mut self, ctx: &PluginContext) -> Vec<PluginAction>;

    /// Called after the document has changed.
    /// Only invoked for plugins that requested the `ReadDocument` or
    /// `ModifyDocument` permission.
    fn on_document_change(&mut self, _ctx: &PluginContext) -> Vec<PluginAction> {
        Vec::new()
    }

    /// Called when the selection set changes.
    fn on_selection_change(&mut self, _ctx: &PluginContext) -> Vec<PluginAction> {
        Vec::new()
    }
}
