//! `sivgo-plugin` -- Plugin / extension system for the Sivgo SVG editor.
//!
//! Provides:
//! - **Plugin trait** defining extension points ([`api::Plugin`])
//! - **PluginManager** for loading, unloading and dispatching plugins ([`manager`])
//! - **PluginRegistry** for installing / uninstalling plugin bundles ([`manager`])
//! - **PluginContext** giving plugins read-only access to the document ([`api::PluginContext`])
//! - **PluginManifest** for plugin metadata and permissions ([`manifest`])
//! - **PluginAction** enum describing what a plugin wants the editor to do ([`actions`])
//! - **Built-in plugins** as examples ([`builtin`])
//! - **WASM sandbox** abstraction for running untrusted WASM plugins ([`wasm_sandbox`])
//! - **Lua scripting engine** for lightweight expression evaluation ([`lua_engine`])
//! - **Plugin UI** primitives for building plugin dialogs ([`plugin_ui`])
//!
//! # Architecture
//!
//! Plugins are Rust trait objects (`Box<dyn Plugin>`) managed by `PluginManager`.
//! WASM and Lua plugin hosts bridge to the same `Plugin` trait through their
//! respective sandbox / engine abstractions.

pub mod actions;
pub mod api;
pub mod builtin;
pub mod lua_engine;
pub mod manager;
pub mod manifest;
pub mod plugin_ui;
pub mod wasm_sandbox;

// Re-export primary types at the crate root.
pub use actions::PluginAction;
pub use api::{Plugin, PluginContext};
pub use builtin::{GridGeneratorPlugin, HelloPlugin};
pub use lua_engine::{eval_expression, LuaEngine, LuaHostCommand, LuaValue};
pub use manager::{InstalledPlugin, PluginManager, PluginRegistry};
pub use manifest::{Permission, PluginManifest, PluginPermissionSet};
pub use plugin_ui::{
    DialogElement, DialogResult, DialogValue, ExtensionMenuItem, ExtensionUiRegistry, PluginDialog,
};
pub use wasm_sandbox::{
    SandboxedWasmRuntime, WasmExport, WasmExportKind, WasmImport, WasmImportKind, WasmModule,
    WasmModuleInspection, WasmPermissions, WasmRuntime, WasmSandbox, WasmValue,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{SvgDocument, SvgElement, SvgSvgAttrs};

    /// Helper: create a minimal document with a root `<svg>`.
    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::new(800.0, LengthUnit::None)),
            height: Some(SvgLength::new(600.0, LengthUnit::None)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);
        doc
    }

    /// Helper: build a PluginContext from a doc reference.
    fn ctx(doc: &SvgDocument) -> PluginContext<'_> {
        PluginContext {
            document: doc,
            selection: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // 1. PluginManifest serialization / deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = PluginManifest {
            id: "test.serde".to_string(),
            name: "Serde Test".to_string(),
            version: "0.1.0".to_string(),
            author: "Tester".to_string(),
            description: "Round-trip test".to_string(),
            permissions: vec![Permission::ReadDocument, Permission::ModifyDocument],
        };

        let json = serde_json::to_string(&manifest).expect("serialize");
        let back: PluginManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.id, manifest.id);
        assert_eq!(back.name, manifest.name);
        assert_eq!(back.version, manifest.version);
        assert_eq!(back.author, manifest.author);
        assert_eq!(back.description, manifest.description);
        assert_eq!(back.permissions, manifest.permissions);
    }

    #[test]
    fn manifest_deserialize_from_json_literal() {
        let json = r#"{
            "id": "ext.foo",
            "name": "Foo",
            "version": "2.0.0",
            "author": "Alice",
            "description": "A foo plugin",
            "permissions": ["AccessFileSystem", "NetworkAccess"]
        }"#;
        let m: PluginManifest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m.permissions.len(), 2);
        assert_eq!(m.permissions[0], Permission::AccessFileSystem);
        assert_eq!(m.permissions[1], Permission::NetworkAccess);
    }

    // -----------------------------------------------------------------------
    // 2. PluginManager register / unregister
    // -----------------------------------------------------------------------

    #[test]
    fn manager_register_unregister() {
        let mut mgr = PluginManager::new();
        assert!(mgr.list().is_empty());

        mgr.register(Box::new(HelloPlugin::new()));
        assert_eq!(mgr.list().len(), 1);
        assert!(mgr.get("builtin.hello").is_some());

        let removed = mgr.unregister("builtin.hello");
        assert!(removed.is_some());
        assert!(mgr.list().is_empty());
        assert!(mgr.get("builtin.hello").is_none());
    }

    #[test]
    fn manager_unregister_unknown_returns_none() {
        let mut mgr = PluginManager::new();
        assert!(mgr.unregister("nonexistent").is_none());
    }

    // -----------------------------------------------------------------------
    // 3. PluginManager enable / disable
    // -----------------------------------------------------------------------

    #[test]
    fn manager_enable_disable() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(HelloPlugin::new()));

        // Enabled by default after registration.
        assert!(mgr.is_enabled("builtin.hello"));

        mgr.set_enabled("builtin.hello", false);
        assert!(!mgr.is_enabled("builtin.hello"));

        mgr.set_enabled("builtin.hello", true);
        assert!(mgr.is_enabled("builtin.hello"));
    }

    #[test]
    fn manager_is_enabled_unknown_returns_false() {
        let mgr = PluginManager::new();
        assert!(!mgr.is_enabled("nonexistent"));
    }

    // -----------------------------------------------------------------------
    // 4. HelloPlugin activation produces actions
    // -----------------------------------------------------------------------

    #[test]
    fn hello_plugin_produces_actions() {
        let mut plugin = HelloPlugin::new();
        let doc = make_doc();
        let c = ctx(&doc);

        let actions = plugin.on_activate(&c);
        assert_eq!(actions.len(), 2);

        // First action should be AddNode.
        match &actions[0] {
            PluginAction::AddNode { parent_id, element } => {
                assert_eq!(*parent_id, doc.root().unwrap());
                assert_eq!(element.tag_name(), "rect");
            }
            other => panic!("Expected AddNode, got {:?}", other),
        }

        // Second action should be ShowMessage.
        match &actions[1] {
            PluginAction::ShowMessage(msg) => {
                assert!(msg.contains("Hello"));
            }
            other => panic!("Expected ShowMessage, got {:?}", other),
        }
    }

    #[test]
    fn hello_plugin_no_root_returns_log() {
        let mut plugin = HelloPlugin::new();
        let doc = SvgDocument::new(); // no root set
        let c = ctx(&doc);

        let actions = plugin.on_activate(&c);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            PluginAction::Log(msg) => assert!(msg.contains("no root")),
            other => panic!("Expected Log, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // 5. GridGeneratorPlugin generates correct number of nodes
    // -----------------------------------------------------------------------

    #[test]
    fn grid_generator_produces_correct_count() {
        let rows = 3u32;
        let cols = 4u32;
        let mut plugin = GridGeneratorPlugin::new(rows, cols, 20.0, 5.0);
        let doc = make_doc();
        let c = ctx(&doc);

        let actions = plugin.on_activate(&c);
        // rows*cols AddNode actions + 1 ShowMessage
        let expected = (rows * cols) as usize + 1;
        assert_eq!(actions.len(), expected);

        // All but the last should be AddNode.
        for action in &actions[..actions.len() - 1] {
            match action {
                PluginAction::AddNode { .. } => {}
                other => panic!("Expected AddNode, got {:?}", other),
            }
        }

        // Last should be ShowMessage.
        match actions.last().unwrap() {
            PluginAction::ShowMessage(msg) => {
                assert!(msg.contains("12 rects"));
            }
            other => panic!("Expected ShowMessage, got {:?}", other),
        }
    }

    #[test]
    fn grid_generator_cell_positions() {
        let mut plugin = GridGeneratorPlugin::new(2, 2, 10.0, 5.0);
        let doc = make_doc();
        let c = ctx(&doc);

        let actions = plugin.on_activate(&c);

        // Verify positions of the 4 cells.
        let expected_positions = [
            (0.0, 0.0),   // row 0, col 0
            (15.0, 0.0),  // row 0, col 1  (10 + 5)
            (0.0, 15.0),  // row 1, col 0
            (15.0, 15.0), // row 1, col 1
        ];

        for (i, (ex, ey)) in expected_positions.iter().enumerate() {
            match &actions[i] {
                PluginAction::AddNode { element, .. } => match element {
                    SvgElement::Rect(attrs) => {
                        assert!(
                            (attrs.x.value - ex).abs() < 1e-10,
                            "cell {} x: expected {}, got {}",
                            i,
                            ex,
                            attrs.x.value
                        );
                        assert!(
                            (attrs.y.value - ey).abs() < 1e-10,
                            "cell {} y: expected {}, got {}",
                            i,
                            ey,
                            attrs.y.value
                        );
                    }
                    _ => panic!("Expected Rect element"),
                },
                other => panic!("Expected AddNode, got {:?}", other),
            }
        }
    }

    // -----------------------------------------------------------------------
    // 6. PluginManager activate dispatches correctly
    // -----------------------------------------------------------------------

    #[test]
    fn manager_activate_dispatches() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(HelloPlugin::new()));
        mgr.load("builtin.hello");

        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.activate("builtin.hello", &c);
        assert_eq!(actions.len(), 2); // AddNode + ShowMessage
    }

    #[test]
    fn manager_activate_disabled_returns_empty() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(HelloPlugin::new()));
        mgr.set_enabled("builtin.hello", false);

        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.activate("builtin.hello", &c);
        assert!(actions.is_empty());
    }

    #[test]
    fn manager_activate_unknown_returns_empty() {
        let mut mgr = PluginManager::new();
        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.activate("nonexistent", &c);
        assert!(actions.is_empty());
    }

    // -----------------------------------------------------------------------
    // 7. PluginManager notify_document_change calls all enabled plugins
    // -----------------------------------------------------------------------

    /// A tiny spy plugin that records how many times it was called.
    #[derive(Debug)]
    struct SpyPlugin {
        manifest: PluginManifest,
        activate_count: u32,
        doc_change_count: u32,
        selection_change_count: u32,
    }

    impl SpyPlugin {
        fn new(id: &str) -> Self {
            Self {
                manifest: PluginManifest {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: "0.0.1".to_string(),
                    author: "Test".to_string(),
                    description: "spy".to_string(),
                    permissions: vec![],
                },
                activate_count: 0,
                doc_change_count: 0,
                selection_change_count: 0,
            }
        }
    }

    impl Plugin for SpyPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn on_activate(&mut self, _ctx: &PluginContext) -> Vec<PluginAction> {
            self.activate_count += 1;
            vec![PluginAction::Log(format!(
                "spy {} activated ({})",
                self.manifest.id, self.activate_count
            ))]
        }

        fn on_document_change(&mut self, _ctx: &PluginContext) -> Vec<PluginAction> {
            self.doc_change_count += 1;
            vec![PluginAction::Log(format!(
                "spy {} doc_change ({})",
                self.manifest.id, self.doc_change_count
            ))]
        }

        fn on_selection_change(&mut self, _ctx: &PluginContext) -> Vec<PluginAction> {
            self.selection_change_count += 1;
            vec![PluginAction::Log(format!(
                "spy {} sel_change ({})",
                self.manifest.id, self.selection_change_count
            ))]
        }
    }

    #[test]
    fn manager_notify_document_change_all_enabled() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(SpyPlugin::new("spy.a")));
        mgr.register(Box::new(SpyPlugin::new("spy.b")));

        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.notify_document_change(&c);
        // Both spies should produce one Log action each.
        assert_eq!(actions.len(), 2);
        for a in &actions {
            match a {
                PluginAction::Log(msg) => assert!(msg.contains("doc_change")),
                other => panic!("Expected Log, got {:?}", other),
            }
        }
    }

    #[test]
    fn manager_notify_document_change_skips_disabled() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(SpyPlugin::new("spy.a")));
        mgr.register(Box::new(SpyPlugin::new("spy.b")));
        mgr.set_enabled("spy.b", false);

        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.notify_document_change(&c);
        // Only spy.a is enabled.
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn manager_notify_selection_change_all_enabled() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(SpyPlugin::new("spy.a")));
        mgr.register(Box::new(SpyPlugin::new("spy.b")));

        let doc = make_doc();
        let c = ctx(&doc);

        let actions = mgr.notify_selection_change(&c);
        assert_eq!(actions.len(), 2);
        for a in &actions {
            match a {
                PluginAction::Log(msg) => assert!(msg.contains("sel_change")),
                other => panic!("Expected Log, got {:?}", other),
            }
        }
    }

    #[test]
    fn manager_load_unload_lifecycle() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(HelloPlugin::new()));

        // These should not panic.
        mgr.load("builtin.hello");
        mgr.unload("builtin.hello");

        // Loading/unloading unknown id is a no-op.
        mgr.load("nonexistent");
        mgr.unload("nonexistent");
    }

    #[test]
    fn manager_default_trait() {
        let mgr = PluginManager::default();
        assert!(mgr.list().is_empty());
    }
}
