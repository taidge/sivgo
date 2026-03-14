//! Menu bar component with full File, Edit, View, Layer, Object, Path,
//! Text, Filters, Extensions, and Help menus.
//!
//! Each menu item emits a `MenuAction` string through the `on_action` handler
//! so the parent can dispatch them to the editor engine.

use dioxus::prelude::*;

// ---------------------------------------------------------------------------
// Menu data model
// ---------------------------------------------------------------------------

/// A single menu item (or separator).
#[derive(Clone)]
enum MenuItem {
    /// A clickable item with action-id, display label, shortcut hint, and enabled flag.
    Action {
        action: &'static str,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
    },
    /// A visual separator line.
    Separator,
}

/// A top-level menu with its label and items.
struct Menu {
    label: &'static str,
    items: Vec<MenuItem>,
}

fn item(action: &'static str, label: &'static str, shortcut: &'static str) -> MenuItem {
    MenuItem::Action {
        action,
        label,
        shortcut,
        enabled: true,
    }
}

fn item_d(
    action: &'static str,
    label: &'static str,
    shortcut: &'static str,
    enabled: bool,
) -> MenuItem {
    MenuItem::Action {
        action,
        label,
        shortcut,
        enabled,
    }
}

fn sep() -> MenuItem {
    MenuItem::Separator
}

/// Build the full set of menus, accepting flags that affect enabled state.
fn build_menus(can_undo: bool, can_redo: bool, has_selection: bool) -> Vec<Menu> {
    vec![
        // 11.3.1 -- File
        Menu {
            label: "File",
            items: vec![
                item("file.new", "New", "Ctrl+N"),
                item("file.new_from_template", "New from Template...", ""),
                sep(),
                item("file.open", "Open...", "Ctrl+O"),
                item("file.open_recent", "Open Recent", ""),
                sep(),
                item("file.save", "Save", "Ctrl+S"),
                item("file.save_as", "Save As...", "Ctrl+Shift+S"),
                item("file.save_copy", "Save a Copy...", "Ctrl+Alt+S"),
                sep(),
                item("file.import", "Import...", "Ctrl+I"),
                item("file.export_png", "Export PNG Image...", "Ctrl+Shift+E"),
                item("file.export_svg", "Export Plain SVG...", ""),
                sep(),
                item("file.print", "Print...", "Ctrl+P"),
                sep(),
                item(
                    "file.document_properties",
                    "Document Properties...",
                    "Ctrl+Shift+D",
                ),
                sep(),
                item("file.close", "Close", "Ctrl+W"),
                item("file.quit", "Quit", "Ctrl+Q"),
            ],
        },
        // 11.3.2 -- Edit
        Menu {
            label: "Edit",
            items: vec![
                item_d("edit.undo", "Undo", "Ctrl+Z", can_undo),
                item_d("edit.redo", "Redo", "Ctrl+Y", can_redo),
                item("edit.history", "Undo History...", ""),
                sep(),
                item_d("edit.cut", "Cut", "Ctrl+X", has_selection),
                item_d("edit.copy", "Copy", "Ctrl+C", has_selection),
                item("edit.paste", "Paste", "Ctrl+V"),
                item("edit.paste_in_place", "Paste in Place", "Ctrl+Alt+V"),
                item("edit.paste_style", "Paste Style", "Ctrl+Shift+V"),
                sep(),
                item_d("edit.delete", "Delete", "Del", has_selection),
                item_d("edit.duplicate", "Duplicate", "Ctrl+D", has_selection),
                item("edit.clone", "Create Clone", "Alt+D"),
                item("edit.unlink_clone", "Unlink Clone", "Alt+Shift+D"),
                sep(),
                item("edit.select_all", "Select All", "Ctrl+A"),
                item("edit.select_all_layers", "Select All in All Layers", ""),
                item("edit.select_same_style", "Select Same Style", ""),
                item("edit.invert_selection", "Invert Selection", "!"),
                item("edit.select_none", "Deselect", "Esc"),
                sep(),
                item("edit.find", "Find / Replace...", "Ctrl+F"),
                sep(),
                item("edit.preferences", "Preferences...", ""),
            ],
        },
        // 11.3.3 -- View
        Menu {
            label: "View",
            items: vec![
                item("view.zoom_in", "Zoom In", "+"),
                item("view.zoom_out", "Zoom Out", "-"),
                item("view.zoom_100", "Zoom 1:1", "1"),
                item("view.zoom_50", "Zoom 1:2", "2"),
                item("view.zoom_fit", "Zoom to Fit Page", "3"),
                item("view.zoom_fit_width", "Zoom to Fit Width", ""),
                item("view.zoom_selection", "Zoom to Selection", "5"),
                sep(),
                item("view.rotate_cw", "Rotate Canvas CW", ""),
                item("view.rotate_ccw", "Rotate Canvas CCW", ""),
                item("view.rotate_reset", "Reset Canvas Rotation", ""),
                sep(),
                item("view.toggle_grid", "Toggle Grid", "#"),
                item("view.toggle_guides", "Toggle Guides", "|"),
                item("view.toggle_rulers", "Toggle Rulers", "Ctrl+R"),
                item("view.toggle_snapping", "Toggle Snapping", "%"),
                sep(),
                item("view.display_mode_normal", "Normal Display", ""),
                item("view.display_mode_outline", "Outline Display", ""),
                item("view.display_mode_no_filters", "No Filters Display", ""),
                sep(),
                item("view.fullscreen", "Fullscreen", "F11"),
                item("view.toggle_palette", "Command Palette", "Ctrl+Shift+P"),
            ],
        },
        // 11.3.4 -- Layer
        Menu {
            label: "Layer",
            items: vec![
                item("layer.add", "Add Layer...", "Ctrl+Shift+N"),
                item("layer.rename", "Rename Layer...", ""),
                item("layer.delete", "Delete Current Layer", ""),
                sep(),
                item("layer.raise", "Raise Layer", ""),
                item("layer.lower", "Lower Layer", ""),
                item("layer.to_top", "Layer to Top", ""),
                item("layer.to_bottom", "Layer to Bottom", ""),
                sep(),
                item("layer.toggle_visible", "Toggle Visibility", ""),
                item("layer.toggle_lock", "Toggle Lock", ""),
                sep(),
                item("layer.merge_down", "Merge Down", ""),
                item("layer.flatten", "Flatten All Layers", ""),
            ],
        },
        // 11.3.5 -- Object
        Menu {
            label: "Object",
            items: vec![
                item_d("object.group", "Group", "Ctrl+G", has_selection),
                item_d("object.ungroup", "Ungroup", "Ctrl+Shift+G", has_selection),
                sep(),
                item_d("object.raise", "Raise", "PgUp", has_selection),
                item_d("object.lower", "Lower", "PgDn", has_selection),
                item_d("object.to_front", "Raise to Top", "Home", has_selection),
                item_d("object.to_back", "Lower to Bottom", "End", has_selection),
                sep(),
                item_d("object.flip_h", "Flip Horizontal", "", has_selection),
                item_d("object.flip_v", "Flip Vertical", "", has_selection),
                item_d(
                    "object.rotate_90_cw",
                    "Rotate 90\u{00B0} CW",
                    "",
                    has_selection,
                ),
                item_d(
                    "object.rotate_90_ccw",
                    "Rotate 90\u{00B0} CCW",
                    "",
                    has_selection,
                ),
                sep(),
                item("object.align", "Align and Distribute...", "Ctrl+Shift+A"),
                item("object.transform", "Transform...", "Ctrl+Shift+M"),
                sep(),
                item_d("object.clip_set", "Set Clip", "", has_selection),
                item_d("object.clip_release", "Release Clip", "", has_selection),
                item_d("object.mask_set", "Set Mask", "", has_selection),
                item_d("object.mask_release", "Release Mask", "", has_selection),
                sep(),
                item("object.object_properties", "Object Properties...", ""),
            ],
        },
        // 11.3.6 -- Path
        Menu {
            label: "Path",
            items: vec![
                item_d(
                    "path.object_to_path",
                    "Object to Path",
                    "Ctrl+Shift+C",
                    has_selection,
                ),
                item_d(
                    "path.stroke_to_path",
                    "Stroke to Path",
                    "Ctrl+Alt+C",
                    has_selection,
                ),
                sep(),
                item_d("path.union", "Union", "Ctrl++", has_selection),
                item_d("path.difference", "Difference", "Ctrl+-", has_selection),
                item_d("path.intersection", "Intersection", "Ctrl+*", has_selection),
                item_d("path.exclusion", "Exclusion", "Ctrl+^", has_selection),
                item_d("path.division", "Division", "Ctrl+/", has_selection),
                item_d("path.cut_path", "Cut Path", "Ctrl+Alt+/", has_selection),
                sep(),
                item_d("path.combine", "Combine", "Ctrl+K", has_selection),
                item_d(
                    "path.break_apart",
                    "Break Apart",
                    "Ctrl+Shift+K",
                    has_selection,
                ),
                sep(),
                item_d("path.inset", "Inset", "Ctrl+(", has_selection),
                item_d("path.outset", "Outset", "Ctrl+)", has_selection),
                item_d(
                    "path.dynamic_offset",
                    "Dynamic Offset",
                    "Ctrl+J",
                    has_selection,
                ),
                item_d(
                    "path.linked_offset",
                    "Linked Offset",
                    "Ctrl+Alt+J",
                    has_selection,
                ),
                sep(),
                item_d("path.simplify", "Simplify", "Ctrl+L", has_selection),
                item_d("path.reverse", "Reverse", "", has_selection),
            ],
        },
        // 11.3.7 -- Text
        Menu {
            label: "Text",
            items: vec![
                item("text.font_size_inc", "Increase Font Size", ""),
                item("text.font_size_dec", "Decrease Font Size", ""),
                sep(),
                item("text.bold", "Bold", "Ctrl+B"),
                item("text.italic", "Italic", "Ctrl+I"),
                item("text.underline", "Underline", "Ctrl+U"),
                sep(),
                item("text.flow_into_frame", "Flow into Frame", "Alt+W"),
                item("text.unflow", "Unflow", ""),
                item("text.put_on_path", "Put on Path", ""),
                item("text.remove_from_path", "Remove from Path", ""),
                sep(),
                item("text.spell_check", "Check Spelling...", ""),
                item("text.text_properties", "Text and Font...", "Ctrl+Shift+T"),
            ],
        },
        // 11.3.8 -- Filters
        Menu {
            label: "Filters",
            items: vec![
                item("filter.editor", "Filter Editor...", ""),
                sep(),
                item("filter.blur", "Blur...", ""),
                item("filter.drop_shadow", "Drop Shadow...", ""),
                item("filter.bevel", "Bevel...", ""),
                item("filter.glow", "Glow...", ""),
                sep(),
                item("filter.color_matrix", "Color Matrix...", ""),
                item("filter.desaturate", "Desaturate", ""),
                item("filter.invert", "Invert", ""),
                sep(),
                item("filter.morphology", "Morphology...", ""),
                item("filter.distort", "Distort...", ""),
                item("filter.turbulence", "Turbulence...", ""),
            ],
        },
        // 11.3.9 -- Extensions
        Menu {
            label: "Extensions",
            items: vec![
                item("ext.extension_mgr", "Extension Manager...", ""),
                sep(),
                item("ext.generate_barcode", "Generate Barcode...", ""),
                item("ext.color_randomize", "Randomize Colors", ""),
                item("ext.export_layers", "Export Layers...", ""),
                item("ext.flatten_beziers", "Flatten Beziers...", ""),
                item("ext.interpolate", "Interpolate...", ""),
                item("ext.motion_path", "Motion Path...", ""),
            ],
        },
        // 11.3.10 -- Help
        Menu {
            label: "Help",
            items: vec![
                item("help.manual", "Sivgo Manual", ""),
                item("help.keys", "Keys and Mouse Reference", ""),
                item("help.tutorials", "Tutorials", ""),
                sep(),
                item("help.report_bug", "Report a Bug...", ""),
                item("help.about", "About Sivgo", ""),
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// The top menu bar with dropdown menus for every editor category.
///
/// Clicking a menu item emits its action id (e.g. `"file.save"`, `"edit.undo"`)
/// through the `on_action` handler. The parent is responsible for mapping the
/// action string to actual editor operations.
///
/// This component also supports the legacy per-action `EventHandler` props for
/// backward compatibility via a thin wrapper; however new code should prefer
/// `on_action`.
#[component]
pub fn MenuBar(
    on_action: EventHandler<String>,
    #[props(default = false)] can_undo: bool,
    #[props(default = false)] can_redo: bool,
    #[props(default = false)] has_selection: bool,
) -> Element {
    let mut open_menu = use_signal(|| Option::<usize>::None);
    let menus = build_menus(can_undo, can_redo, has_selection);

    rsx! {
        div { class: "menu-bar",
            for (idx, menu) in menus.iter().enumerate() {
                div {
                    class: "menu-dropdown",
                    // Toggle open/close on the top-level label
                    button {
                        class: if *open_menu.read() == Some(idx) { "menu-top-btn active" } else { "menu-top-btn" },
                        onclick: move |_| {
                            let current = *open_menu.read();
                            if current == Some(idx) {
                                open_menu.set(None);
                            } else {
                                open_menu.set(Some(idx));
                            }
                        },
                        onmouseenter: move |_| {
                            // If any menu is open, hovering another switches to it.
                            if open_menu.read().is_some() {
                                open_menu.set(Some(idx));
                            }
                        },
                        "{menu.label}"
                    }

                    // Dropdown panel
                    if *open_menu.read() == Some(idx) {
                        div { class: "menu-dropdown-panel",
                            for mi in menu.items.iter() {
                                match mi {
                                    MenuItem::Separator => rsx! {
                                        div { class: "menu-sep" }
                                    },
                                    MenuItem::Action { action, label, shortcut, enabled } => {
                                        let action_str = action.to_string();
                                        let is_enabled = *enabled;
                                        rsx! {
                                            button {
                                                class: "menu-item",
                                                disabled: !is_enabled,
                                                onclick: {
                                                    let action_owned = action_str.clone();
                                                    move |_| {
                                                        on_action.call(action_owned.clone());
                                                        open_menu.set(None);
                                                    }
                                                },
                                                span { class: "menu-item-label", "{label}" }
                                                if !shortcut.is_empty() {
                                                    span { class: "menu-item-shortcut", "{shortcut}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
