//! Document Resources panel for listing all resources used in the SVG
//! document: gradients, patterns, fonts, images, filters, and markers.

use crate::{controls::*, Callback};
use glory::prelude::*;

/// Kind of SVG document resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceKind {
    Gradient,
    Pattern,
    Font,
    Image,
    Filter,
    Marker,
    Symbol,
    ClipPath,
    Mask,
    Template,
    Tutorial,
    ExtensionAsset,
}

impl ResourceKind {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Gradient => "Gradient",
            Self::Pattern => "Pattern",
            Self::Font => "Font",
            Self::Image => "Image",
            Self::Filter => "Filter",
            Self::Marker => "Marker",
            Self::Symbol => "Symbol",
            Self::ClipPath => "Clip Path",
            Self::Mask => "Mask",
            Self::Template => "Template",
            Self::Tutorial => "Tutorial",
            Self::ExtensionAsset => "Extension Asset",
        }
    }

    /// All resource kinds.
    pub fn all() -> &'static [ResourceKind] {
        &[
            Self::Gradient,
            Self::Pattern,
            Self::Font,
            Self::Image,
            Self::Filter,
            Self::Marker,
            Self::Symbol,
            Self::ClipPath,
            Self::Mask,
            Self::Template,
            Self::Tutorial,
            Self::ExtensionAsset,
        ]
    }
}

/// A single resource entry in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceEntry {
    /// The element id in the SVG.
    pub id: String,
    /// Kind of resource.
    pub kind: ResourceKind,
    /// Human-readable name or label.
    pub name: String,
    /// Number of elements referencing this resource.
    pub usage_count: usize,
}

impl ResourceEntry {
    /// Create a new resource entry.
    pub fn new(
        id: impl Into<String>,
        kind: ResourceKind,
        name: impl Into<String>,
        usage_count: usize,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            usage_count,
        }
    }

    /// Whether this resource is unused (zero references).
    pub fn is_unused(&self) -> bool {
        self.usage_count == 0
    }
}

/// Events emitted by the Resources panel.
#[derive(Debug, Clone)]
pub enum ResourcesAction {
    /// Select a resource for inspection.
    Select(String),
    /// Filter by resource kind (None = show all).
    SetKindFilter(Option<ResourceKind>),
    /// Delete an unused resource by id.
    DeleteUnused(String),
    /// Delete all unused resources.
    DeleteAllUnused,
}

/// A panel listing all resources (gradients, patterns, fonts, images, etc.)
/// used in the document, with usage counts and cleanup controls.
pub struct ResourcesPanel {
    pub resources: Vec<ResourceEntry>,
    pub kind_filter: Option<String>,
    pub selected_id: Option<String>,
    pub on_action: Callback<ResourcesAction>,
}

impl Widget for ResourcesPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let panel = panel(scope, "resources-panel", "Document Resources")?;
        let mut options = vec!["All"];
        options.extend(ResourceKind::all().iter().map(ResourceKind::label));
        let callback = self.on_action.clone();
        choice_field(
            &panel,
            "Resource kind",
            self.kind_filter.clone().unwrap_or_else(|| "All".into()),
            &options,
            move |v| {
                callback.call(ResourcesAction::SetKindFilter(
                    ResourceKind::all().iter().find(|k| k.label() == v).cloned(),
                ));
            },
        )?;
        let filter = self.kind_filter.as_deref().unwrap_or("All");
        let mut count = 0;
        for resource in &self.resources {
            if !filter.is_empty() && filter != "All" && filter != resource.kind.label() {
                continue;
            }
            count += 1;
            let callback = self.on_action.clone();
            let id = resource.id.clone();
            action_button(
                &panel,
                &format!(
                    "{}: {} #{} ({} uses)",
                    resource.kind.label(),
                    resource.name,
                    resource.id,
                    resource.usage_count
                ),
                self.selected_id.as_ref() == Some(&resource.id),
                move || callback.call(ResourcesAction::Select(id.clone())),
            )?;
            if resource.is_unused() {
                let callback = self.on_action.clone();
                let id = resource.id.clone();
                action_button(
                    &panel,
                    &format!("Delete unused {}", resource.name),
                    false,
                    move || callback.call(ResourcesAction::DeleteUnused(id.clone())),
                )?;
            }
        }
        if count == 0 {
            text().content("No resources found").show_in(&panel)?;
        }
        let unused = self.resources.iter().filter(|r| r.is_unused()).count();
        let callback = self.on_action.clone();
        sapid::button()
            .theme(crate::controls::editor_theme())
            .label(format!("Delete All Unused ({unused})"))
            .enabled(unused > 0)
            .on_press(move || callback.call(ResourcesAction::DeleteAllUnused))
            .show_in(&panel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_entry_new_and_unused() {
        let r = ResourceEntry::new("grad1", ResourceKind::Gradient, "Blue Gradient", 3);
        assert_eq!(r.id, "grad1");
        assert_eq!(r.kind, ResourceKind::Gradient);
        assert_eq!(r.name, "Blue Gradient");
        assert_eq!(r.usage_count, 3);
        assert!(!r.is_unused());

        let unused = ResourceEntry::new("pat1", ResourceKind::Pattern, "Old Pattern", 0);
        assert!(unused.is_unused());
    }

    #[test]
    fn resource_kind_labels() {
        assert_eq!(ResourceKind::Gradient.label(), "Gradient");
        assert_eq!(ResourceKind::ClipPath.label(), "Clip Path");
        assert_eq!(ResourceKind::Symbol.label(), "Symbol");
        assert_eq!(ResourceKind::Mask.label(), "Mask");
        assert_eq!(ResourceKind::all().len(), 12);
    }
}
