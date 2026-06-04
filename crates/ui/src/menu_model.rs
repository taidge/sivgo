//! Menu bar data model (11.3.1-11.3.10).
//!
//! Defines a hierarchical menu structure with actions, shortcuts, separators,
//! and a builder for the full Inkscape-style menu layout.

/// A single menu item in a hierarchical menu tree.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    /// Display label for the item.
    pub label: String,
    /// Action identifier (e.g. `"file.save"`).  `None` for parent-only items.
    pub action: Option<String>,
    /// Keyboard shortcut hint (e.g. `"Ctrl+S"`).
    pub shortcut: Option<String>,
    /// Sub-menu items.  Empty for leaf items.
    pub children: Vec<MenuItem>,
    /// Whether this item is a visual separator line.
    pub separator: bool,
    /// Whether the item is currently enabled.
    pub enabled: bool,
    /// Optional checked state for toggle items (e.g. "Show Grid").
    pub checked: Option<bool>,
}

impl MenuItem {
    /// Create a clickable menu item with an action.
    pub fn action(
        label: impl Into<String>,
        action: impl Into<String>,
        shortcut: impl Into<Option<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            action: Some(action.into()),
            shortcut: shortcut.into(),
            children: Vec::new(),
            separator: false,
            enabled: true,
            checked: None,
        }
    }

    /// Create a separator line.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            action: None,
            shortcut: None,
            children: Vec::new(),
            separator: true,
            enabled: false,
            checked: None,
        }
    }

    /// Create a sub-menu parent item.
    pub fn submenu(label: impl Into<String>, children: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            action: None,
            shortcut: None,
            children,
            separator: false,
            enabled: true,
            checked: None,
        }
    }

    /// Create a menu item with a checked state.
    pub fn toggle(
        label: impl Into<String>,
        action: impl Into<String>,
        shortcut: impl Into<Option<String>>,
        checked: bool,
    ) -> Self {
        Self {
            label: label.into(),
            action: Some(action.into()),
            shortcut: shortcut.into(),
            children: Vec::new(),
            separator: false,
            enabled: true,
            checked: Some(checked),
        }
    }

    /// Builder: set the enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Return the total number of non-separator items in this tree (including self).
    pub fn count_actions(&self) -> usize {
        let self_count = if self.action.is_some() { 1 } else { 0 };
        self_count
            + self
                .children
                .iter()
                .map(|c| c.count_actions())
                .sum::<usize>()
    }
}

// Shorthand helpers.
fn item(label: &str, action: &str, shortcut: &str) -> MenuItem {
    MenuItem::action(
        label,
        action,
        if shortcut.is_empty() {
            None
        } else {
            Some(shortcut.to_string())
        },
    )
    .with_enabled(is_action_wired(action))
}

fn item_d(label: &str, action: &str, shortcut: &str, enabled: bool) -> MenuItem {
    item(label, action, shortcut).with_enabled(enabled && is_action_wired(action))
}

fn sep() -> MenuItem {
    MenuItem::separator()
}

/// Whether a menu action is currently connected to a real desktop operation.
///
/// Unsupported Inkscape-parity actions remain visible in the menu model, but
/// disabled, so the UI does not advertise inert commands as usable.
pub fn is_action_wired(action: &str) -> bool {
    matches!(
        action,
        "file.new"
            | "file.open"
            | "file.save"
            | "file.save_as"
            | "file.save_copy"
            | "file.export_png"
            | "file.export_svg"
            | "file.export_pages_svg"
            | "file.export_pages_pdf"
            | "file.print"
            | "file.document_properties"
            | "file.close"
            | "file.quit"
            | "edit.undo"
            | "edit.redo"
            | "edit.cut"
            | "edit.copy"
            | "edit.copy_bitmap"
            | "edit.paste"
            | "edit.paste_in_place"
            | "edit.paste_style"
            | "edit.delete"
            | "edit.duplicate"
            | "edit.clone"
            | "edit.unlink_clone"
            | "edit.select_all"
            | "edit.select_all_layers"
            | "edit.invert_selection"
            | "edit.select_none"
            | "edit.preferences"
            | "view.zoom_in"
            | "view.zoom_out"
            | "view.zoom_100"
            | "view.zoom_50"
            | "view.zoom_fit"
            | "view.zoom_selection"
            | "view.toggle_grid"
            | "view.toggle_guides"
            | "view.toggle_snapping"
            | "view.display_mode_normal"
            | "view.display_mode_outline"
            | "view.display_mode_no_filters"
            | "view.display_mode_hairlines"
            | "view.display_mode_print"
            | "view.display_mode_proof"
            | "view.fullscreen"
            | "layer.add"
            | "layer.delete"
            | "layer.raise"
            | "layer.lower"
            | "layer.to_top"
            | "layer.to_bottom"
            | "layer.toggle_visible"
            | "layer.toggle_lock"
            | "layer.move_selection_to_active"
            | "layer.merge_down"
            | "layer.flatten"
            | "object.group"
            | "object.ungroup"
            | "object.raise"
            | "object.lower"
            | "object.to_front"
            | "object.to_back"
            | "object.flip_h"
            | "object.flip_v"
            | "object.rotate_90_cw"
            | "object.rotate_90_ccw"
            | "object.align"
            | "object.transform"
            | "object.clip_set"
            | "object.clip_release"
            | "object.mask_set"
            | "object.mask_release"
            | "object.object_properties"
            | "path.object_to_path"
            | "path.stroke_to_path"
            | "path.union"
            | "path.difference"
            | "path.intersection"
            | "path.exclusion"
            | "path.division"
            | "path.cut_path"
            | "path.combine"
            | "path.break_apart"
            | "path.simplify"
            | "path.reverse"
            | "path.inset"
            | "path.outset"
            | "path.dynamic_offset"
            | "path.linked_offset"
            | "text.font_size_inc"
            | "text.font_size_dec"
            | "text.bold"
            | "text.italic"
            | "text.to_path"
            | "text.text_properties"
            | "help.about"
    )
}

/// Build the full default menu bar structure for the Sivgo editor.
///
/// The `can_undo`, `can_redo`, and `has_selection` flags control the
/// enabled state of relevant menu items.
pub fn default_menu_bar() -> Vec<MenuItem> {
    default_menu_bar_with_state(false, false, false)
}

/// Build the full default menu bar with specific editor state flags.
pub fn default_menu_bar_with_state(
    can_undo: bool,
    can_redo: bool,
    has_selection: bool,
) -> Vec<MenuItem> {
    vec![
        // 11.3.1 -- File
        MenuItem::submenu(
            "File",
            vec![
                item("New", "file.new", "Ctrl+N"),
                item("New from Template...", "file.new_from_template", ""),
                sep(),
                item("Open...", "file.open", "Ctrl+O"),
                item("Open Recent", "file.open_recent", ""),
                sep(),
                item("Save", "file.save", "Ctrl+S"),
                item("Save As...", "file.save_as", "Ctrl+Shift+S"),
                item("Save a Copy...", "file.save_copy", "Ctrl+Alt+S"),
                sep(),
                item("Import...", "file.import", "Ctrl+I"),
                item("Export PNG Image...", "file.export_png", "Ctrl+Shift+E"),
                item("Export Plain SVG...", "file.export_svg", ""),
                item("Export Pages as SVG...", "file.export_pages_svg", ""),
                item("Export Pages as PDF...", "file.export_pages_pdf", ""),
                sep(),
                item("Print...", "file.print", "Ctrl+P"),
                sep(),
                item(
                    "Document Properties...",
                    "file.document_properties",
                    "Ctrl+Shift+D",
                ),
                sep(),
                item("Close", "file.close", "Ctrl+W"),
                item("Quit", "file.quit", "Ctrl+Q"),
            ],
        ),
        // 11.3.2 -- Edit
        MenuItem::submenu(
            "Edit",
            vec![
                item_d("Undo", "edit.undo", "Ctrl+Z", can_undo),
                item_d("Redo", "edit.redo", "Ctrl+Y", can_redo),
                item("Undo History...", "edit.history", ""),
                sep(),
                item_d("Cut", "edit.cut", "Ctrl+X", has_selection),
                item_d("Copy", "edit.copy", "Ctrl+C", has_selection),
                item("Copy as Bitmap", "edit.copy_bitmap", ""),
                item("Paste", "edit.paste", "Ctrl+V"),
                item("Paste in Place", "edit.paste_in_place", "Ctrl+Alt+V"),
                item("Paste Style", "edit.paste_style", "Ctrl+Shift+V"),
                sep(),
                item_d("Delete", "edit.delete", "Del", has_selection),
                item_d("Duplicate", "edit.duplicate", "Ctrl+D", has_selection),
                item("Create Clone", "edit.clone", "Alt+D"),
                item("Unlink Clone", "edit.unlink_clone", "Alt+Shift+D"),
                sep(),
                item("Select All", "edit.select_all", "Ctrl+A"),
                item("Select All in All Layers", "edit.select_all_layers", ""),
                item("Select Same Style", "edit.select_same_style", ""),
                item("Invert Selection", "edit.invert_selection", "!"),
                item("Deselect", "edit.select_none", "Esc"),
                sep(),
                item("Find / Replace...", "edit.find", "Ctrl+F"),
                sep(),
                item("Preferences...", "edit.preferences", ""),
            ],
        ),
        // 11.3.3 -- View
        MenuItem::submenu(
            "View",
            vec![
                item("Zoom In", "view.zoom_in", "+"),
                item("Zoom Out", "view.zoom_out", "-"),
                item("Zoom 1:1", "view.zoom_100", "1"),
                item("Zoom 1:2", "view.zoom_50", "2"),
                item("Zoom to Fit Page", "view.zoom_fit", "3"),
                item("Zoom to Fit Width", "view.zoom_fit_width", ""),
                item("Zoom to Selection", "view.zoom_selection", "5"),
                sep(),
                item("Rotate Canvas CW", "view.rotate_cw", ""),
                item("Rotate Canvas CCW", "view.rotate_ccw", ""),
                item("Reset Canvas Rotation", "view.rotate_reset", ""),
                sep(),
                item("Toggle Grid", "view.toggle_grid", "#"),
                item("Toggle Guides", "view.toggle_guides", "|"),
                item("Toggle Rulers", "view.toggle_rulers", "Ctrl+R"),
                item("Toggle Snapping", "view.toggle_snapping", "%"),
                sep(),
                item("Normal Display", "view.display_mode_normal", ""),
                item("Outline Display", "view.display_mode_outline", ""),
                item("No Filters Display", "view.display_mode_no_filters", ""),
                item(
                    "Visible Hairlines Display",
                    "view.display_mode_hairlines",
                    "",
                ),
                item("Print Preview Display", "view.display_mode_print", ""),
                item("Proof Colors Display", "view.display_mode_proof", ""),
                sep(),
                item("Fullscreen", "view.fullscreen", "F11"),
                item("Command Palette", "view.toggle_palette", "Ctrl+Shift+P"),
            ],
        ),
        // 11.3.4 -- Layer
        MenuItem::submenu(
            "Layer",
            vec![
                item("Add Layer...", "layer.add", "Ctrl+Shift+N"),
                item("Rename Layer...", "layer.rename", ""),
                item("Delete Current Layer", "layer.delete", ""),
                sep(),
                item("Raise Layer", "layer.raise", ""),
                item("Lower Layer", "layer.lower", ""),
                item("Layer to Top", "layer.to_top", ""),
                item("Layer to Bottom", "layer.to_bottom", ""),
                sep(),
                item("Toggle Visibility", "layer.toggle_visible", ""),
                item("Toggle Lock", "layer.toggle_lock", ""),
                item_d(
                    "Move Selection to Active Layer",
                    "layer.move_selection_to_active",
                    "",
                    has_selection,
                ),
                sep(),
                item("Merge Down", "layer.merge_down", ""),
                item("Flatten All Layers", "layer.flatten", ""),
            ],
        ),
        // 11.3.5 -- Object
        MenuItem::submenu(
            "Object",
            vec![
                item_d("Group", "object.group", "Ctrl+G", has_selection),
                item_d("Ungroup", "object.ungroup", "Ctrl+Shift+G", has_selection),
                sep(),
                item_d("Raise", "object.raise", "PgUp", has_selection),
                item_d("Lower", "object.lower", "PgDn", has_selection),
                item_d("Raise to Top", "object.to_front", "Home", has_selection),
                item_d("Lower to Bottom", "object.to_back", "End", has_selection),
                sep(),
                item_d("Flip Horizontal", "object.flip_h", "", has_selection),
                item_d("Flip Vertical", "object.flip_v", "", has_selection),
                item_d(
                    "Rotate 90\u{00B0} CW",
                    "object.rotate_90_cw",
                    "",
                    has_selection,
                ),
                item_d(
                    "Rotate 90\u{00B0} CCW",
                    "object.rotate_90_ccw",
                    "",
                    has_selection,
                ),
                sep(),
                item("Align and Distribute...", "object.align", "Ctrl+Shift+A"),
                item("Transform...", "object.transform", "Ctrl+Shift+M"),
                sep(),
                item_d("Set Clip", "object.clip_set", "", has_selection),
                item_d("Release Clip", "object.clip_release", "", has_selection),
                item_d("Set Mask", "object.mask_set", "", has_selection),
                item_d("Release Mask", "object.mask_release", "", has_selection),
                sep(),
                item("Object Properties...", "object.object_properties", ""),
            ],
        ),
        // 11.3.6 -- Path
        MenuItem::submenu(
            "Path",
            vec![
                item_d(
                    "Object to Path",
                    "path.object_to_path",
                    "Ctrl+Shift+C",
                    has_selection,
                ),
                item_d(
                    "Stroke to Path",
                    "path.stroke_to_path",
                    "Ctrl+Alt+C",
                    has_selection,
                ),
                sep(),
                item_d("Union", "path.union", "Ctrl++", has_selection),
                item_d("Difference", "path.difference", "Ctrl+-", has_selection),
                item_d("Intersection", "path.intersection", "Ctrl+*", has_selection),
                item_d("Exclusion", "path.exclusion", "Ctrl+^", has_selection),
                item_d("Division", "path.division", "Ctrl+/", has_selection),
                item_d("Cut Path", "path.cut_path", "Ctrl+Alt+/", has_selection),
                sep(),
                item_d("Combine", "path.combine", "Ctrl+K", has_selection),
                item_d(
                    "Break Apart",
                    "path.break_apart",
                    "Ctrl+Shift+K",
                    has_selection,
                ),
                sep(),
                item_d("Inset", "path.inset", "Ctrl+(", has_selection),
                item_d("Outset", "path.outset", "Ctrl+)", has_selection),
                item_d(
                    "Dynamic Offset",
                    "path.dynamic_offset",
                    "Ctrl+J",
                    has_selection,
                ),
                item_d(
                    "Linked Offset",
                    "path.linked_offset",
                    "Ctrl+Alt+J",
                    has_selection,
                ),
                sep(),
                item_d("Simplify", "path.simplify", "Ctrl+L", has_selection),
                item_d("Reverse", "path.reverse", "", has_selection),
            ],
        ),
        // 11.3.7 -- Text
        MenuItem::submenu(
            "Text",
            vec![
                item("Increase Font Size", "text.font_size_inc", ""),
                item("Decrease Font Size", "text.font_size_dec", ""),
                sep(),
                item("Bold", "text.bold", "Ctrl+B"),
                item("Italic", "text.italic", "Ctrl+I"),
                item("Underline", "text.underline", "Ctrl+U"),
                sep(),
                item("Flow into Frame", "text.flow_into_frame", "Alt+W"),
                item("Unflow", "text.unflow", ""),
                item("Put on Path", "text.put_on_path", ""),
                item("Remove from Path", "text.remove_from_path", ""),
                item_d("Text to Path", "text.to_path", "", has_selection),
                sep(),
                item("Check Spelling...", "text.spell_check", ""),
                item("Text and Font...", "text.text_properties", "Ctrl+Shift+T"),
            ],
        ),
        // 11.3.8 -- Filters
        MenuItem::submenu(
            "Filters",
            vec![
                item("Filter Editor...", "filter.editor", ""),
                sep(),
                item("Blur...", "filter.blur", ""),
                item("Drop Shadow...", "filter.drop_shadow", ""),
                item("Bevel...", "filter.bevel", ""),
                item("Glow...", "filter.glow", ""),
                sep(),
                item("Color Matrix...", "filter.color_matrix", ""),
                item("Desaturate", "filter.desaturate", ""),
                item("Invert", "filter.invert", ""),
                sep(),
                item("Morphology...", "filter.morphology", ""),
                item("Distort...", "filter.distort", ""),
                item("Turbulence...", "filter.turbulence", ""),
            ],
        ),
        // 11.3.9 -- Extensions
        MenuItem::submenu(
            "Extensions",
            vec![
                item("Extension Manager...", "ext.extension_mgr", ""),
                sep(),
                item("Generate Barcode...", "ext.generate_barcode", ""),
                item("Randomize Colors", "ext.color_randomize", ""),
                item("Export Layers...", "ext.export_layers", ""),
                item("Flatten Beziers...", "ext.flatten_beziers", ""),
                item("Interpolate...", "ext.interpolate", ""),
                item("Motion Path...", "ext.motion_path", ""),
            ],
        ),
        // 11.3.10 -- Help
        MenuItem::submenu(
            "Help",
            vec![
                item("Sivgo Manual", "help.manual", ""),
                item("Keys and Mouse Reference", "help.keys", ""),
                item("Tutorials", "help.tutorials", ""),
                sep(),
                item("Report a Bug...", "help.report_bug", ""),
                item("About Sivgo", "help.about", ""),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_menu_has_10_top_level_entries() {
        let menu = default_menu_bar();
        assert_eq!(menu.len(), 10);
        assert_eq!(menu[0].label, "File");
        assert_eq!(menu[9].label, "Help");
    }

    #[test]
    fn file_menu_has_actions() {
        let menu = default_menu_bar();
        let file = &menu[0];
        let action_count = file.count_actions();
        // File menu has about 14 actionable items (excluding separators and the parent)
        assert!(action_count >= 10, "File menu has {} actions", action_count);
    }

    #[test]
    fn disabled_items_when_no_selection() {
        let menu = default_menu_bar_with_state(false, false, false);
        let edit = &menu[1];
        // Find the "Cut" item -- it should be disabled.
        let cut = edit.children.iter().find(|i| i.label == "Cut");
        assert!(cut.is_some());
        assert!(!cut.unwrap().enabled);
    }

    #[test]
    fn copy_as_bitmap_is_wired_without_selection() {
        let menu = default_menu_bar_with_state(false, false, false);
        let edit = &menu[1];
        let copy_bitmap = edit
            .children
            .iter()
            .find(|i| i.action.as_deref() == Some("edit.copy_bitmap"));
        assert!(copy_bitmap.is_some());
        assert!(copy_bitmap.unwrap().enabled);
        assert!(is_action_wired("edit.copy_bitmap"));
    }

    #[test]
    fn dynamic_and_linked_offset_are_wired_when_selection_exists() {
        let menu = default_menu_bar_with_state(false, false, true);
        let path = menu.iter().find(|item| item.label == "Path").unwrap();

        let dynamic = path
            .children
            .iter()
            .find(|item| item.action.as_deref() == Some("path.dynamic_offset"))
            .unwrap();
        let linked = path
            .children
            .iter()
            .find(|item| item.action.as_deref() == Some("path.linked_offset"))
            .unwrap();

        assert!(dynamic.enabled);
        assert!(linked.enabled);
        assert!(is_action_wired("path.dynamic_offset"));
        assert!(is_action_wired("path.linked_offset"));
    }

    #[test]
    fn all_display_mode_actions_are_wired() {
        let menu = default_menu_bar();
        let view = menu.iter().find(|item| item.label == "View").unwrap();
        for action in [
            "view.display_mode_normal",
            "view.display_mode_outline",
            "view.display_mode_no_filters",
            "view.display_mode_hairlines",
            "view.display_mode_print",
            "view.display_mode_proof",
        ] {
            let item = view
                .children
                .iter()
                .find(|item| item.action.as_deref() == Some(action))
                .unwrap_or_else(|| panic!("missing {action}"));
            assert!(item.enabled, "{action} should be enabled");
            assert!(is_action_wired(action));
        }
    }

    #[test]
    fn layer_aware_selection_actions_are_wired() {
        let menu = default_menu_bar_with_state(false, false, true);
        let edit = menu.iter().find(|item| item.label == "Edit").unwrap();
        let layer = menu.iter().find(|item| item.label == "Layer").unwrap();

        let select_all_layers = edit
            .children
            .iter()
            .find(|item| item.action.as_deref() == Some("edit.select_all_layers"))
            .unwrap();
        let move_to_active = layer
            .children
            .iter()
            .find(|item| item.action.as_deref() == Some("layer.move_selection_to_active"))
            .unwrap();

        assert!(select_all_layers.enabled);
        assert!(move_to_active.enabled);
        assert!(is_action_wired("edit.select_all_layers"));
        assert!(is_action_wired("layer.move_selection_to_active"));
    }

    #[test]
    fn menu_covers_file_text_view_and_layer_interactions() {
        let menu = default_menu_bar_with_state(true, true, true);
        let find_action = |action: &str| {
            fn walk<'a>(items: &'a [MenuItem], action: &str) -> Option<&'a MenuItem> {
                for item in items {
                    if item.action.as_deref() == Some(action) {
                        return Some(item);
                    }
                    if let Some(found) = walk(&item.children, action) {
                        return Some(found);
                    }
                }
                None
            }
            walk(&menu, action).unwrap_or_else(|| panic!("missing menu action {action}"))
        };

        for action in [
            "file.open",
            "file.save",
            "file.export_png",
            "file.export_pages_svg",
            "file.export_pages_pdf",
            "text.font_size_inc",
            "text.font_size_dec",
            "text.bold",
            "text.italic",
            "text.text_properties",
            "text.to_path",
            "view.zoom_in",
            "view.zoom_out",
            "view.zoom_fit",
            "view.zoom_selection",
            "layer.add",
            "layer.move_selection_to_active",
        ] {
            let item = find_action(action);
            assert!(item.enabled, "{action} should be enabled");
            assert!(is_action_wired(action), "{action} should be wired");
        }
    }
}
