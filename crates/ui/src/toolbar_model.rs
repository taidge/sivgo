//! Toolbar layout data model (11.2.1-11.2.2).
//!
//! Defines toolbar items, groups, and a default toolbox configuration.
//! Uses `sivgo_core::ToolId` for tool identification.

use sivgo_core::ToolId;

/// A single item in the toolbar.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarItem {
    /// Unique identifier string.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Icon string (Unicode symbol or icon name).
    pub icon: String,
    /// Optional keyboard shortcut hint.
    pub shortcut: Option<String>,
    /// Whether this tool is currently active/selected.
    pub active: bool,
}

/// A group of related toolbar items.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarGroup {
    /// Group display name.
    pub name: String,
    /// Items in this group.
    pub items: Vec<ToolbarItem>,
}

impl ToolbarGroup {
    /// Find the first active item in the group, if any.
    pub fn active_item(&self) -> Option<&ToolbarItem> {
        self.items.iter().find(|item| item.active)
    }
}

/// Build a toolbar item from a `ToolId`.
fn make_item(id: ToolId, label: &str, icon: &str, shortcut: Option<&str>) -> ToolbarItem {
    ToolbarItem {
        id: format!("{:?}", id),
        label: label.to_string(),
        icon: icon.to_string(),
        shortcut: shortcut.map(String::from),
        active: false,
    }
}

/// Build the default toolbox layout matching the standard Sivgo editor.
pub fn default_toolbox() -> Vec<ToolbarGroup> {
    vec![
        ToolbarGroup {
            name: "Selection".to_string(),
            items: vec![
                make_item(
                    ToolId::Select,
                    "Select and Transform",
                    "\u{25ED}",
                    Some("S"),
                ),
                make_item(ToolId::Node, "Edit Nodes", "\u{25C7}", Some("N")),
            ],
        },
        ToolbarGroup {
            name: "Shapes".to_string(),
            items: vec![
                make_item(ToolId::Rect, "Rectangle", "\u{25AD}", Some("R")),
                make_item(ToolId::Ellipse, "Ellipse", "\u{25CB}", Some("E")),
                make_item(ToolId::Star, "Star / Polygon", "\u{2606}", Some("*")),
                make_item(ToolId::Spiral, "Spiral", "\u{0040}", Some("I")),
                make_item(ToolId::Box3d, "3D Box", "\u{2B1C}", Some("X")),
            ],
        },
        ToolbarGroup {
            name: "Drawing".to_string(),
            items: vec![
                make_item(ToolId::Pen, "Pen / Bezier", "\u{2712}", Some("B")),
                make_item(ToolId::Pencil, "Pencil / Freehand", "\u{270F}", Some("P")),
                make_item(ToolId::Calligraphy, "Calligraphy", "\u{1F58B}", Some("C")),
            ],
        },
        ToolbarGroup {
            name: "Text & Paint".to_string(),
            items: vec![
                make_item(ToolId::Text, "Text", "A", Some("T")),
                make_item(ToolId::Gradient, "Gradient", "\u{25B3}", Some("G")),
                make_item(ToolId::Mesh, "Mesh Gradient", "\u{2593}", Some("M")),
                make_item(
                    ToolId::Dropper,
                    "Color Picker / Dropper",
                    "\u{1F4A7}",
                    Some("D"),
                ),
                make_item(ToolId::BucketFill, "Bucket Fill", "\u{25A8}", Some("U")),
            ],
        },
        ToolbarGroup {
            name: "Modify".to_string(),
            items: vec![
                make_item(ToolId::Spray, "Spray", "\u{2055}", Some("A")),
                make_item(ToolId::Tweak, "Tweak / Sculpt", "\u{219D}", Some("W")),
                make_item(ToolId::Eraser, "Eraser", "\u{232B}", Some("Shift+E")),
            ],
        },
        ToolbarGroup {
            name: "Connectors & Layout".to_string(),
            items: vec![
                make_item(ToolId::Connector, "Connector", "\u{2194}", Some("O")),
                make_item(ToolId::Marker, "Marker", "\u{25C9}", Some("K")),
                make_item(ToolId::Pages, "Pages / Multi-page", "\u{2630}", None),
            ],
        },
        ToolbarGroup {
            name: "Measure & Zoom".to_string(),
            items: vec![
                make_item(ToolId::Measure, "Measure", "\u{2702}", Some("F5")),
                make_item(ToolId::Zoom, "Zoom", "\u{2315}", Some("Z")),
            ],
        },
    ]
}

/// Set the active state for a single tool by its id string, clearing all others.
pub fn set_active_tool(groups: &mut [ToolbarGroup], tool_id: &str) {
    for group in groups.iter_mut() {
        for item in group.items.iter_mut() {
            item.active = item.id == tool_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_toolbox_has_groups() {
        let groups = default_toolbox();
        assert_eq!(groups.len(), 7);
        assert_eq!(groups[0].name, "Selection");
        assert_eq!(groups[0].items.len(), 2);
    }

    #[test]
    fn set_active_tool_works() {
        let mut groups = default_toolbox();
        set_active_tool(&mut groups, "Select");
        let active = groups[0].active_item();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, "Select");
    }

    #[test]
    fn total_tool_count() {
        let groups = default_toolbox();
        let total: usize = groups.iter().map(|g| g.items.len()).sum();
        // Should match the 22 tools defined in ToolId (minus Lpe which isn't in toolbar)
        assert!(total >= 20);
    }
}
