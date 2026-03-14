use crate::actions::PluginAction;
use crate::api::{Plugin, PluginContext};
use crate::manifest::{Permission, PluginManifest};
use sivgo_dom::types::{LengthUnit, SvgLength};
use sivgo_dom::{RectAttrs, SvgElement};

/// A simple example plugin that adds a rounded rectangle to the document root.
#[derive(Debug)]
pub struct HelloPlugin {
    manifest: PluginManifest,
}

impl HelloPlugin {
    pub fn new() -> Self {
        Self {
            manifest: PluginManifest {
                id: "builtin.hello".to_string(),
                name: "Hello World".to_string(),
                version: "1.0.0".to_string(),
                author: "Sivgo Team".to_string(),
                description: "Adds a hello world rectangle to the document".to_string(),
                permissions: vec![Permission::ModifyDocument],
            },
        }
    }
}

impl Default for HelloPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelloPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn on_activate(&mut self, ctx: &PluginContext) -> Vec<PluginAction> {
        if let Some(root) = ctx.document.root() {
            vec![
                PluginAction::AddNode {
                    parent_id: root,
                    element: SvgElement::Rect(RectAttrs {
                        x: SvgLength::new(50.0, LengthUnit::None),
                        y: SvgLength::new(50.0, LengthUnit::None),
                        width: SvgLength::new(200.0, LengthUnit::None),
                        height: SvgLength::new(100.0, LengthUnit::None),
                        rx: SvgLength::new(10.0, LengthUnit::None),
                        ry: SvgLength::new(10.0, LengthUnit::None),
                    }),
                },
                PluginAction::ShowMessage("Hello from plugin!".to_string()),
            ]
        } else {
            vec![PluginAction::Log(
                "HelloPlugin: no root node in document".to_string(),
            )]
        }
    }
}
