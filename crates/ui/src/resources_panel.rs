//! Document Resources panel for listing all resources used in the SVG
//! document: gradients, patterns, fonts, images, filters, and markers.

use dioxus::prelude::*;

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
#[component]
pub fn ResourcesPanel(
    resources: Vec<ResourceEntry>,
    kind_filter: Option<String>,
    selected_id: Option<String>,
    on_action: EventHandler<ResourcesAction>,
) -> Element {
    let filter_val = kind_filter.clone().unwrap_or_default();
    let kind_options = vec![
        "All",
        "Gradient",
        "Pattern",
        "Font",
        "Image",
        "Filter",
        "Marker",
        "Symbol",
        "Clip Path",
        "Mask",
        "Template",
        "Tutorial",
        "Extension Asset",
    ];

    // Client-side filtering
    let filtered: Vec<&ResourceEntry> = resources
        .iter()
        .filter(|r| filter_val.is_empty() || filter_val == "All" || r.kind.label() == filter_val)
        .collect();

    let unused_count = resources.iter().filter(|r| r.is_unused()).count();

    rsx! {
        div { class: "resources-panel",
            div { class: "panel-header", "Document Resources" }

            // ---- Kind filter ----
            div { class: "prop-row",
                label { class: "prop-label", "Filter" }
                select {
                    class: "prop-select",
                    value: "{filter_val}",
                    onchange: move |evt: Event<FormData>| {
                        let val = evt.value().to_string();
                        let kind = match val.as_str() {
                            "Gradient" => Some(ResourceKind::Gradient),
                            "Pattern" => Some(ResourceKind::Pattern),
                            "Font" => Some(ResourceKind::Font),
                            "Image" => Some(ResourceKind::Image),
                            "Filter" => Some(ResourceKind::Filter),
                            "Marker" => Some(ResourceKind::Marker),
                            "Symbol" => Some(ResourceKind::Symbol),
                            "Clip Path" => Some(ResourceKind::ClipPath),
                            "Mask" => Some(ResourceKind::Mask),
                            "Template" => Some(ResourceKind::Template),
                            "Tutorial" => Some(ResourceKind::Tutorial),
                            "Extension Asset" => Some(ResourceKind::ExtensionAsset),
                            _ => None,
                        };
                        on_action.call(ResourcesAction::SetKindFilter(kind));
                    },
                    for opt in kind_options.iter() {
                        {
                            let is_sel = filter_val == *opt || (filter_val.is_empty() && *opt == "All");
                            rsx! { option { value: "{opt}", selected: is_sel, "{opt}" } }
                        }
                    }
                }
            }

            // ---- Resource list ----
            div { class: "resources-list",
                div { class: "prop-section-header", "Resources ({filtered.len()})" }
                for res in filtered.iter() {
                    {
                        let rid = res.id.clone();
                        let rid_del = res.id.clone();
                        let is_selected = selected_id.as_deref() == Some(res.id.as_str());
                        let is_unused = res.is_unused();
                        rsx! {
                            div {
                                key: "{res.id}",
                                class: if is_selected { "resource-item selected" } else { "resource-item" },
                                onclick: move |_| on_action.call(ResourcesAction::Select(rid.clone())),

                                span { class: "resource-kind-badge", "{res.kind.label()}" }
                                span { class: "resource-name", "{res.name}" }
                                span { class: "resource-id", "#{res.id}" }
                                span {
                                    class: if is_unused { "resource-usage unused" } else { "resource-usage" },
                                    "{res.usage_count} uses"
                                }

                                if is_unused {
                                    button {
                                        class: "resource-btn delete",
                                        title: "Delete unused resource",
                                        onclick: move |_| on_action.call(ResourcesAction::DeleteUnused(rid_del.clone())),
                                        "X"
                                    }
                                }
                            }
                        }
                    }
                }
                if filtered.is_empty() {
                    div { class: "resources-empty", "No resources found" }
                }
            }

            // ---- Cleanup ----
            div { class: "dialog-actions",
                button {
                    class: "action-btn",
                    disabled: unused_count == 0,
                    onclick: move |_| on_action.call(ResourcesAction::DeleteAllUnused),
                    "Delete All Unused ({unused_count})"
                }
            }
        }
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
