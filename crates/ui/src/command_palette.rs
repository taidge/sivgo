//! Searchable command palette overlay.
//!
//! Shows a filterable list of editor commands with their keyboard shortcuts.
//! The user types a query and matching commands are ranked by a simple fuzzy
//! match. Selecting a command emits its action string via `on_command`.

use dioxus::prelude::*;

// ---------------------------------------------------------------------------
// Command descriptor
// ---------------------------------------------------------------------------

/// A single entry in the command palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteCommand {
    /// Machine-readable action id (e.g. "file.save", "edit.undo").
    pub action: &'static str,
    /// Human-readable label shown in the list.
    pub label: &'static str,
    /// Keyboard shortcut hint (display only).
    pub shortcut: &'static str,
    /// Category for grouping in the palette.
    pub category: &'static str,
}

/// Returns the full catalogue of commands available from the palette.
pub fn all_commands() -> Vec<PaletteCommand> {
    vec![
        // ---- File ----
        PaletteCommand {
            action: "file.new",
            label: "New Document",
            shortcut: "Ctrl+N",
            category: "File",
        },
        PaletteCommand {
            action: "file.open",
            label: "Open...",
            shortcut: "Ctrl+O",
            category: "File",
        },
        PaletteCommand {
            action: "file.save",
            label: "Save",
            shortcut: "Ctrl+S",
            category: "File",
        },
        PaletteCommand {
            action: "file.save_as",
            label: "Save As...",
            shortcut: "Ctrl+Shift+S",
            category: "File",
        },
        PaletteCommand {
            action: "file.export_png",
            label: "Export PNG",
            shortcut: "Ctrl+Shift+E",
            category: "File",
        },
        PaletteCommand {
            action: "file.export_svg",
            label: "Export Plain SVG",
            shortcut: "",
            category: "File",
        },
        PaletteCommand {
            action: "file.export_pages_svg",
            label: "Export Pages as SVG",
            shortcut: "",
            category: "File",
        },
        PaletteCommand {
            action: "file.export_pages_pdf",
            label: "Export Pages as PDF",
            shortcut: "",
            category: "File",
        },
        PaletteCommand {
            action: "file.import",
            label: "Import...",
            shortcut: "Ctrl+I",
            category: "File",
        },
        PaletteCommand {
            action: "file.print",
            label: "Print...",
            shortcut: "Ctrl+P",
            category: "File",
        },
        PaletteCommand {
            action: "file.close",
            label: "Close",
            shortcut: "Ctrl+W",
            category: "File",
        },
        PaletteCommand {
            action: "file.quit",
            label: "Quit",
            shortcut: "Ctrl+Q",
            category: "File",
        },
        // ---- Edit ----
        PaletteCommand {
            action: "edit.undo",
            label: "Undo",
            shortcut: "Ctrl+Z",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.redo",
            label: "Redo",
            shortcut: "Ctrl+Y",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.cut",
            label: "Cut",
            shortcut: "Ctrl+X",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.copy",
            label: "Copy",
            shortcut: "Ctrl+C",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.copy_bitmap",
            label: "Copy as Bitmap",
            shortcut: "",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.paste",
            label: "Paste",
            shortcut: "Ctrl+V",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.paste_in_place",
            label: "Paste in Place",
            shortcut: "Ctrl+Alt+V",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.delete",
            label: "Delete",
            shortcut: "Del",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.duplicate",
            label: "Duplicate",
            shortcut: "Ctrl+D",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.clone",
            label: "Create Clone",
            shortcut: "Alt+D",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.unlink_clone",
            label: "Unlink Clone",
            shortcut: "Alt+Shift+D",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.select_all",
            label: "Select All",
            shortcut: "Ctrl+A",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.select_all_layers",
            label: "Select All in All Layers",
            shortcut: "",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.select_none",
            label: "Deselect",
            shortcut: "Esc",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.find",
            label: "Find / Replace...",
            shortcut: "Ctrl+F",
            category: "Edit",
        },
        PaletteCommand {
            action: "edit.preferences",
            label: "Preferences...",
            shortcut: "",
            category: "Edit",
        },
        // ---- View ----
        PaletteCommand {
            action: "view.zoom_in",
            label: "Zoom In",
            shortcut: "+",
            category: "View",
        },
        PaletteCommand {
            action: "view.zoom_out",
            label: "Zoom Out",
            shortcut: "-",
            category: "View",
        },
        PaletteCommand {
            action: "view.zoom_fit",
            label: "Zoom to Fit",
            shortcut: "3",
            category: "View",
        },
        PaletteCommand {
            action: "view.zoom_selection",
            label: "Zoom to Selection",
            shortcut: "5",
            category: "View",
        },
        PaletteCommand {
            action: "view.zoom_100",
            label: "Zoom 100%",
            shortcut: "1",
            category: "View",
        },
        PaletteCommand {
            action: "view.toggle_grid",
            label: "Toggle Grid",
            shortcut: "#",
            category: "View",
        },
        PaletteCommand {
            action: "view.toggle_guides",
            label: "Toggle Guides",
            shortcut: "|",
            category: "View",
        },
        PaletteCommand {
            action: "view.toggle_rulers",
            label: "Toggle Rulers",
            shortcut: "Ctrl+R",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_normal",
            label: "Normal Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_outline",
            label: "Outline Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_no_filters",
            label: "No Filters Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_hairlines",
            label: "Visible Hairlines Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_print",
            label: "Print Preview Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.display_mode_proof",
            label: "Proof Colors Display",
            shortcut: "",
            category: "View",
        },
        PaletteCommand {
            action: "view.fullscreen",
            label: "Fullscreen",
            shortcut: "F11",
            category: "View",
        },
        // ---- Layer ----
        PaletteCommand {
            action: "layer.add",
            label: "Add Layer",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.delete",
            label: "Delete Layer",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.rename",
            label: "Rename Layer",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.raise",
            label: "Raise Layer",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.lower",
            label: "Lower Layer",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.to_top",
            label: "Layer to Top",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.to_bottom",
            label: "Layer to Bottom",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.toggle_visible",
            label: "Toggle Layer Visibility",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.toggle_lock",
            label: "Toggle Layer Lock",
            shortcut: "",
            category: "Layer",
        },
        PaletteCommand {
            action: "layer.move_selection_to_active",
            label: "Move Selection to Active Layer",
            shortcut: "",
            category: "Layer",
        },
        // ---- Object ----
        PaletteCommand {
            action: "object.group",
            label: "Group",
            shortcut: "Ctrl+G",
            category: "Object",
        },
        PaletteCommand {
            action: "object.ungroup",
            label: "Ungroup",
            shortcut: "Ctrl+Shift+G",
            category: "Object",
        },
        PaletteCommand {
            action: "object.raise",
            label: "Raise",
            shortcut: "PgUp",
            category: "Object",
        },
        PaletteCommand {
            action: "object.lower",
            label: "Lower",
            shortcut: "PgDn",
            category: "Object",
        },
        PaletteCommand {
            action: "object.to_front",
            label: "Raise to Top",
            shortcut: "Home",
            category: "Object",
        },
        PaletteCommand {
            action: "object.to_back",
            label: "Lower to Bottom",
            shortcut: "End",
            category: "Object",
        },
        PaletteCommand {
            action: "object.flip_h",
            label: "Flip Horizontal",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.flip_v",
            label: "Flip Vertical",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.rotate_90_cw",
            label: "Rotate 90\u{00B0} CW",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.rotate_90_ccw",
            label: "Rotate 90\u{00B0} CCW",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.align",
            label: "Align and Distribute...",
            shortcut: "Ctrl+Shift+A",
            category: "Object",
        },
        PaletteCommand {
            action: "object.transform",
            label: "Transform...",
            shortcut: "Ctrl+Shift+M",
            category: "Object",
        },
        PaletteCommand {
            action: "object.clip_set",
            label: "Set Clip",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.clip_release",
            label: "Release Clip",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.mask_set",
            label: "Set Mask",
            shortcut: "",
            category: "Object",
        },
        PaletteCommand {
            action: "object.mask_release",
            label: "Release Mask",
            shortcut: "",
            category: "Object",
        },
        // ---- Path ----
        PaletteCommand {
            action: "path.object_to_path",
            label: "Object to Path",
            shortcut: "Ctrl+Shift+C",
            category: "Path",
        },
        PaletteCommand {
            action: "path.stroke_to_path",
            label: "Stroke to Path",
            shortcut: "Ctrl+Alt+C",
            category: "Path",
        },
        PaletteCommand {
            action: "path.union",
            label: "Union",
            shortcut: "Ctrl++",
            category: "Path",
        },
        PaletteCommand {
            action: "path.difference",
            label: "Difference",
            shortcut: "Ctrl+-",
            category: "Path",
        },
        PaletteCommand {
            action: "path.intersection",
            label: "Intersection",
            shortcut: "Ctrl+*",
            category: "Path",
        },
        PaletteCommand {
            action: "path.exclusion",
            label: "Exclusion",
            shortcut: "Ctrl+^",
            category: "Path",
        },
        PaletteCommand {
            action: "path.division",
            label: "Division",
            shortcut: "Ctrl+/",
            category: "Path",
        },
        PaletteCommand {
            action: "path.cut_path",
            label: "Cut Path",
            shortcut: "Ctrl+Alt+/",
            category: "Path",
        },
        PaletteCommand {
            action: "path.combine",
            label: "Combine",
            shortcut: "Ctrl+K",
            category: "Path",
        },
        PaletteCommand {
            action: "path.break_apart",
            label: "Break Apart",
            shortcut: "Ctrl+Shift+K",
            category: "Path",
        },
        PaletteCommand {
            action: "path.inset",
            label: "Inset",
            shortcut: "Ctrl+(",
            category: "Path",
        },
        PaletteCommand {
            action: "path.outset",
            label: "Outset",
            shortcut: "Ctrl+)",
            category: "Path",
        },
        PaletteCommand {
            action: "path.dynamic_offset",
            label: "Dynamic Offset",
            shortcut: "Ctrl+J",
            category: "Path",
        },
        PaletteCommand {
            action: "path.linked_offset",
            label: "Linked Offset",
            shortcut: "Ctrl+Alt+J",
            category: "Path",
        },
        PaletteCommand {
            action: "path.simplify",
            label: "Simplify",
            shortcut: "Ctrl+L",
            category: "Path",
        },
        PaletteCommand {
            action: "path.reverse",
            label: "Reverse",
            shortcut: "",
            category: "Path",
        },
        // ---- Text ----
        PaletteCommand {
            action: "text.flow_into_frame",
            label: "Flow into Frame",
            shortcut: "Alt+W",
            category: "Text",
        },
        PaletteCommand {
            action: "text.unflow",
            label: "Unflow",
            shortcut: "",
            category: "Text",
        },
        PaletteCommand {
            action: "text.put_on_path",
            label: "Put on Path",
            shortcut: "",
            category: "Text",
        },
        PaletteCommand {
            action: "text.remove_from_path",
            label: "Remove from Path",
            shortcut: "",
            category: "Text",
        },
        PaletteCommand {
            action: "text.to_path",
            label: "Text to Path",
            shortcut: "",
            category: "Text",
        },
        PaletteCommand {
            action: "text.spell_check",
            label: "Check Spelling...",
            shortcut: "",
            category: "Text",
        },
        // ---- Filters ----
        PaletteCommand {
            action: "filter.blur",
            label: "Blur...",
            shortcut: "",
            category: "Filters",
        },
        PaletteCommand {
            action: "filter.drop_shadow",
            label: "Drop Shadow...",
            shortcut: "",
            category: "Filters",
        },
        PaletteCommand {
            action: "filter.editor",
            label: "Filter Editor...",
            shortcut: "",
            category: "Filters",
        },
        // ---- Extensions ----
        PaletteCommand {
            action: "ext.generate_barcode",
            label: "Generate Barcode...",
            shortcut: "",
            category: "Extensions",
        },
        PaletteCommand {
            action: "ext.color_randomize",
            label: "Randomize Colors",
            shortcut: "",
            category: "Extensions",
        },
        PaletteCommand {
            action: "ext.extension_mgr",
            label: "Extension Manager...",
            shortcut: "",
            category: "Extensions",
        },
        // ---- Help ----
        PaletteCommand {
            action: "help.about",
            label: "About Sivgo",
            shortcut: "",
            category: "Help",
        },
        PaletteCommand {
            action: "help.keys",
            label: "Keys and Mouse Reference",
            shortcut: "",
            category: "Help",
        },
        PaletteCommand {
            action: "help.manual",
            label: "Sivgo Manual",
            shortcut: "",
            category: "Help",
        },
    ]
}

// ---------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------

/// Simple case-insensitive subsequence match returning a score.
///
/// Returns `None` if the query does not match. A lower score is better.
fn fuzzy_score(query: &str, text: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    let query_lower = query.to_ascii_lowercase();
    let text_lower = text.to_ascii_lowercase();

    let mut qi = 0;
    let query_bytes = query_lower.as_bytes();
    let mut penalty: usize = 0;
    let mut gap = 0usize;

    for ch in text_lower.bytes() {
        if qi < query_bytes.len() && ch == query_bytes[qi] {
            penalty += gap;
            gap = 0;
            qi += 1;
        } else {
            gap += 1;
        }
    }

    if qi == query_bytes.len() {
        Some(penalty)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A searchable command palette overlay.
///
/// When `visible` is true, the palette renders on top of the editor.
/// Typing in the search box filters the command list using fuzzy matching.
/// Clicking a command (or pressing Enter on the highlighted one) fires
/// `on_command` with the action string. `on_close` is called when the
/// palette should be dismissed (Escape key or clicking outside).
#[component]
pub fn CommandPalette(
    visible: bool,
    on_command: EventHandler<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut query = use_signal(|| String::new());

    // Recompute the filtered + sorted command list only when the query signal
    // changes. The closure reads `query`, so the memo is correctly reactive.
    // NOTE: this hook must be called unconditionally (before any early return)
    // to keep the hook order stable across renders.
    let filtered = use_memo(move || {
        let q = query.read().clone();
        let commands = all_commands();
        let mut scored: Vec<(usize, PaletteCommand)> = commands
            .into_iter()
            .filter_map(|cmd| {
                // Match against label and action.
                let score_label = fuzzy_score(&q, cmd.label);
                let score_action = fuzzy_score(&q, cmd.action);
                let best = match (score_label, score_action) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                best.map(|s| (s, cmd))
            })
            .collect::<Vec<_>>();
        scored.sort_by_key(|(score, _)| *score);
        scored.into_iter().map(|(_, cmd)| cmd).collect::<Vec<_>>()
    });

    if !visible {
        return rsx! {};
    }

    let q = query.read().clone();

    rsx! {
        div {
            class: "command-palette-overlay",
            onclick: move |_| on_close.call(()),

            div {
                class: "command-palette",
                // stop click from propagating to overlay
                onclick: move |evt| evt.stop_propagation(),

                input {
                    class: "cp-search",
                    r#type: "text",
                    placeholder: "Type a command...",
                    autofocus: true,
                    value: "{q}",
                    oninput: move |evt: Event<FormData>| {
                        query.set(evt.value().to_string());
                    },
                    onkeydown: move |evt: Event<KeyboardData>| {
                        if evt.key() == Key::Escape {
                            on_close.call(());
                        }
                    },
                }

                div { class: "cp-results",
                    if filtered.read().is_empty() {
                        div { class: "cp-empty", "No matching commands" }
                    }
                    for cmd in filtered.read().iter() {
                        button {
                            key: "{cmd.action}",
                            class: "cp-item",
                            onclick: {
                                let action_owned = cmd.action.to_string();
                                move |_| on_command.call(action_owned.clone())
                            },
                            span { class: "cp-category", "[{cmd.category}] " }
                            span { class: "cp-label", "{cmd.label}" }
                            if !cmd.shortcut.is_empty() {
                                span { class: "cp-shortcut", "{cmd.shortcut}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_offset_commands_are_exposed() {
        let commands = all_commands();

        assert!(commands.iter().any(|cmd| cmd.action == "path.inset"));
        assert!(commands.iter().any(|cmd| cmd.action == "path.outset"));
        assert!(commands
            .iter()
            .any(|cmd| cmd.action == "path.dynamic_offset"));
        assert!(commands
            .iter()
            .any(|cmd| cmd.action == "path.linked_offset"));
    }

    #[test]
    fn display_mode_commands_are_exposed() {
        let commands = all_commands();

        for action in [
            "view.display_mode_normal",
            "view.display_mode_outline",
            "view.display_mode_no_filters",
            "view.display_mode_hairlines",
            "view.display_mode_print",
            "view.display_mode_proof",
        ] {
            assert!(
                commands.iter().any(|cmd| cmd.action == action),
                "missing {action}"
            );
        }
    }

    #[test]
    fn layer_aware_commands_are_exposed() {
        let commands = all_commands();

        assert!(commands
            .iter()
            .any(|cmd| cmd.action == "edit.select_all_layers"));
        assert!(commands
            .iter()
            .any(|cmd| cmd.action == "layer.move_selection_to_active"));
    }
}
