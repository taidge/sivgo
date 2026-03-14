//! Keyboard shortcut reference dialog (11.6.7).
//!
//! Provides a searchable, categorised list of all default keyboard shortcuts.

/// A single keyboard shortcut entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    /// The action name (e.g. "New Document").
    pub action: String,
    /// Human-readable shortcut display string (e.g. "Ctrl+N").
    pub shortcut_display: String,
    /// Brief description of what the shortcut does.
    pub description: String,
}

/// A group of related shortcuts.
#[derive(Debug, Clone)]
pub struct ShortcutCategory {
    /// Category name (e.g. "File", "Edit").
    pub name: String,
    /// Shortcuts in this category.
    pub shortcuts: Vec<ShortcutEntry>,
}

/// The keyboard shortcut reference panel state.
#[derive(Debug, Clone)]
pub struct ShortcutReferencePanel {
    /// Shortcut categories with their entries.
    pub categories: Vec<ShortcutCategory>,
    /// Current search filter text.
    pub search_filter: String,
}

impl ShortcutReferencePanel {
    /// Build the reference panel populated with all default shortcuts grouped
    /// by category.
    pub fn build_reference() -> Self {
        let categories = vec![
            ShortcutCategory {
                name: "File".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "New Document".to_string(),
                        shortcut_display: "Ctrl+N".to_string(),
                        description: "Create a new empty document".to_string(),
                    },
                    ShortcutEntry {
                        action: "Open".to_string(),
                        shortcut_display: "Ctrl+O".to_string(),
                        description: "Open an existing SVG file".to_string(),
                    },
                    ShortcutEntry {
                        action: "Save".to_string(),
                        shortcut_display: "Ctrl+S".to_string(),
                        description: "Save the current document".to_string(),
                    },
                    ShortcutEntry {
                        action: "Save As".to_string(),
                        shortcut_display: "Ctrl+Shift+S".to_string(),
                        description: "Save the document with a new name".to_string(),
                    },
                    ShortcutEntry {
                        action: "Export PNG".to_string(),
                        shortcut_display: "Ctrl+Shift+E".to_string(),
                        description: "Export the document as a PNG image".to_string(),
                    },
                    ShortcutEntry {
                        action: "Print".to_string(),
                        shortcut_display: "Ctrl+P".to_string(),
                        description: "Print the document".to_string(),
                    },
                    ShortcutEntry {
                        action: "Close".to_string(),
                        shortcut_display: "Ctrl+W".to_string(),
                        description: "Close the current document".to_string(),
                    },
                    ShortcutEntry {
                        action: "Quit".to_string(),
                        shortcut_display: "Ctrl+Q".to_string(),
                        description: "Exit the application".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Edit".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Undo".to_string(),
                        shortcut_display: "Ctrl+Z".to_string(),
                        description: "Undo the last action".to_string(),
                    },
                    ShortcutEntry {
                        action: "Redo".to_string(),
                        shortcut_display: "Ctrl+Y".to_string(),
                        description: "Redo the last undone action".to_string(),
                    },
                    ShortcutEntry {
                        action: "Cut".to_string(),
                        shortcut_display: "Ctrl+X".to_string(),
                        description: "Cut selected objects to clipboard".to_string(),
                    },
                    ShortcutEntry {
                        action: "Copy".to_string(),
                        shortcut_display: "Ctrl+C".to_string(),
                        description: "Copy selected objects to clipboard".to_string(),
                    },
                    ShortcutEntry {
                        action: "Paste".to_string(),
                        shortcut_display: "Ctrl+V".to_string(),
                        description: "Paste objects from clipboard".to_string(),
                    },
                    ShortcutEntry {
                        action: "Paste In Place".to_string(),
                        shortcut_display: "Ctrl+Alt+V".to_string(),
                        description: "Paste at the original position".to_string(),
                    },
                    ShortcutEntry {
                        action: "Delete".to_string(),
                        shortcut_display: "Delete".to_string(),
                        description: "Delete selected objects".to_string(),
                    },
                    ShortcutEntry {
                        action: "Duplicate".to_string(),
                        shortcut_display: "Ctrl+D".to_string(),
                        description: "Duplicate selected objects in place".to_string(),
                    },
                    ShortcutEntry {
                        action: "Clone".to_string(),
                        shortcut_display: "Alt+D".to_string(),
                        description: "Create a clone of the selected object".to_string(),
                    },
                    ShortcutEntry {
                        action: "Select All".to_string(),
                        shortcut_display: "Ctrl+A".to_string(),
                        description: "Select all objects on the canvas".to_string(),
                    },
                    ShortcutEntry {
                        action: "Deselect".to_string(),
                        shortcut_display: "Escape".to_string(),
                        description: "Deselect all objects".to_string(),
                    },
                    ShortcutEntry {
                        action: "Find/Replace".to_string(),
                        shortcut_display: "Ctrl+F".to_string(),
                        description: "Open find and replace dialog".to_string(),
                    },
                    ShortcutEntry {
                        action: "Preferences".to_string(),
                        shortcut_display: "Ctrl+Shift+P".to_string(),
                        description: "Open preferences dialog".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "View".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Zoom In".to_string(),
                        shortcut_display: "+".to_string(),
                        description: "Zoom in on the canvas".to_string(),
                    },
                    ShortcutEntry {
                        action: "Zoom Out".to_string(),
                        shortcut_display: "-".to_string(),
                        description: "Zoom out on the canvas".to_string(),
                    },
                    ShortcutEntry {
                        action: "Zoom to Fit".to_string(),
                        shortcut_display: "3".to_string(),
                        description: "Fit the drawing in the viewport".to_string(),
                    },
                    ShortcutEntry {
                        action: "Zoom to Selection".to_string(),
                        shortcut_display: "5".to_string(),
                        description: "Zoom to fit the selected objects".to_string(),
                    },
                    ShortcutEntry {
                        action: "Zoom 100%".to_string(),
                        shortcut_display: "1".to_string(),
                        description: "Set zoom to 100%".to_string(),
                    },
                    ShortcutEntry {
                        action: "Toggle Grid".to_string(),
                        shortcut_display: "#".to_string(),
                        description: "Show or hide the grid".to_string(),
                    },
                    ShortcutEntry {
                        action: "Toggle Guides".to_string(),
                        shortcut_display: "|".to_string(),
                        description: "Show or hide guides".to_string(),
                    },
                    ShortcutEntry {
                        action: "Toggle Rulers".to_string(),
                        shortcut_display: "Ctrl+R".to_string(),
                        description: "Show or hide rulers".to_string(),
                    },
                    ShortcutEntry {
                        action: "Fullscreen".to_string(),
                        shortcut_display: "F11".to_string(),
                        description: "Toggle fullscreen mode".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Object".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Group".to_string(),
                        shortcut_display: "Ctrl+G".to_string(),
                        description: "Group selected objects".to_string(),
                    },
                    ShortcutEntry {
                        action: "Ungroup".to_string(),
                        shortcut_display: "Ctrl+Shift+G".to_string(),
                        description: "Ungroup selected group".to_string(),
                    },
                    ShortcutEntry {
                        action: "Raise".to_string(),
                        shortcut_display: "Page Up".to_string(),
                        description: "Raise selected object one step".to_string(),
                    },
                    ShortcutEntry {
                        action: "Lower".to_string(),
                        shortcut_display: "Page Down".to_string(),
                        description: "Lower selected object one step".to_string(),
                    },
                    ShortcutEntry {
                        action: "Raise to Top".to_string(),
                        shortcut_display: "Home".to_string(),
                        description: "Raise selected object to top of stack".to_string(),
                    },
                    ShortcutEntry {
                        action: "Lower to Bottom".to_string(),
                        shortcut_display: "End".to_string(),
                        description: "Lower selected object to bottom of stack".to_string(),
                    },
                    ShortcutEntry {
                        action: "Rotate 90 CW".to_string(),
                        shortcut_display: "Ctrl+]".to_string(),
                        description: "Rotate selection 90 degrees clockwise".to_string(),
                    },
                    ShortcutEntry {
                        action: "Rotate 90 CCW".to_string(),
                        shortcut_display: "Ctrl+[".to_string(),
                        description: "Rotate selection 90 degrees counter-clockwise".to_string(),
                    },
                    ShortcutEntry {
                        action: "Flip Horizontal".to_string(),
                        shortcut_display: "Ctrl+Shift+H".to_string(),
                        description: "Flip selection horizontally".to_string(),
                    },
                    ShortcutEntry {
                        action: "Flip Vertical".to_string(),
                        shortcut_display: "Ctrl+Shift+V".to_string(),
                        description: "Flip selection vertically".to_string(),
                    },
                    ShortcutEntry {
                        action: "Object Properties".to_string(),
                        shortcut_display: "Ctrl+Shift+O".to_string(),
                        description: "Open object properties dialog".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Path".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Object to Path".to_string(),
                        shortcut_display: "Ctrl+Shift+C".to_string(),
                        description: "Convert selected object to a path".to_string(),
                    },
                    ShortcutEntry {
                        action: "Stroke to Path".to_string(),
                        shortcut_display: "Ctrl+Alt+C".to_string(),
                        description: "Convert the stroke to a filled path".to_string(),
                    },
                    ShortcutEntry {
                        action: "Union".to_string(),
                        shortcut_display: "Ctrl++".to_string(),
                        description: "Boolean union of selected paths".to_string(),
                    },
                    ShortcutEntry {
                        action: "Difference".to_string(),
                        shortcut_display: "Ctrl+-".to_string(),
                        description: "Boolean difference of selected paths".to_string(),
                    },
                    ShortcutEntry {
                        action: "Intersection".to_string(),
                        shortcut_display: "Ctrl+*".to_string(),
                        description: "Boolean intersection of selected paths".to_string(),
                    },
                    ShortcutEntry {
                        action: "Exclusion".to_string(),
                        shortcut_display: "Ctrl+^".to_string(),
                        description: "Boolean XOR of selected paths".to_string(),
                    },
                    ShortcutEntry {
                        action: "Combine".to_string(),
                        shortcut_display: "Ctrl+K".to_string(),
                        description: "Combine selected paths into one compound path".to_string(),
                    },
                    ShortcutEntry {
                        action: "Break Apart".to_string(),
                        shortcut_display: "Ctrl+Shift+K".to_string(),
                        description: "Break a compound path into separate paths".to_string(),
                    },
                    ShortcutEntry {
                        action: "Simplify".to_string(),
                        shortcut_display: "Ctrl+L".to_string(),
                        description: "Simplify selected path by reducing nodes".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Text".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Bold".to_string(),
                        shortcut_display: "Ctrl+B".to_string(),
                        description: "Toggle bold on selected text".to_string(),
                    },
                    ShortcutEntry {
                        action: "Italic".to_string(),
                        shortcut_display: "Ctrl+I".to_string(),
                        description: "Toggle italic on selected text".to_string(),
                    },
                    ShortcutEntry {
                        action: "Increase Font Size".to_string(),
                        shortcut_display: "Ctrl+Shift+>".to_string(),
                        description: "Increase the font size of selected text".to_string(),
                    },
                    ShortcutEntry {
                        action: "Decrease Font Size".to_string(),
                        shortcut_display: "Ctrl+Shift+<".to_string(),
                        description: "Decrease the font size of selected text".to_string(),
                    },
                    ShortcutEntry {
                        action: "Align Left".to_string(),
                        shortcut_display: "Ctrl+Shift+L".to_string(),
                        description: "Align text to the left".to_string(),
                    },
                    ShortcutEntry {
                        action: "Align Center".to_string(),
                        shortcut_display: "Ctrl+Shift+C".to_string(),
                        description: "Center-align text".to_string(),
                    },
                    ShortcutEntry {
                        action: "Align Right".to_string(),
                        shortcut_display: "Ctrl+Shift+R".to_string(),
                        description: "Align text to the right".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Tools".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Selector".to_string(),
                        shortcut_display: "S".to_string(),
                        description: "Switch to the selection/pointer tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Node Tool".to_string(),
                        shortcut_display: "N".to_string(),
                        description: "Switch to the node editing tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Rectangle Tool".to_string(),
                        shortcut_display: "R".to_string(),
                        description: "Switch to the rectangle drawing tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Ellipse Tool".to_string(),
                        shortcut_display: "E".to_string(),
                        description: "Switch to the ellipse drawing tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Star Tool".to_string(),
                        shortcut_display: "*".to_string(),
                        description: "Switch to the star/polygon tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Pen Tool".to_string(),
                        shortcut_display: "B".to_string(),
                        description: "Switch to the bezier pen tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Pencil Tool".to_string(),
                        shortcut_display: "P".to_string(),
                        description: "Switch to the freehand pencil tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Text Tool".to_string(),
                        shortcut_display: "T".to_string(),
                        description: "Switch to the text tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Gradient Tool".to_string(),
                        shortcut_display: "G".to_string(),
                        description: "Switch to the gradient editing tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Dropper Tool".to_string(),
                        shortcut_display: "D".to_string(),
                        description: "Switch to the colour picker tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Zoom Tool".to_string(),
                        shortcut_display: "Z".to_string(),
                        description: "Switch to the zoom tool".to_string(),
                    },
                    ShortcutEntry {
                        action: "Measure Tool".to_string(),
                        shortcut_display: "M".to_string(),
                        description: "Switch to the measure tool".to_string(),
                    },
                ],
            },
            ShortcutCategory {
                name: "Navigation".to_string(),
                shortcuts: vec![
                    ShortcutEntry {
                        action: "Scroll Up".to_string(),
                        shortcut_display: "Ctrl+Up".to_string(),
                        description: "Scroll the canvas up".to_string(),
                    },
                    ShortcutEntry {
                        action: "Scroll Down".to_string(),
                        shortcut_display: "Ctrl+Down".to_string(),
                        description: "Scroll the canvas down".to_string(),
                    },
                    ShortcutEntry {
                        action: "Scroll Left".to_string(),
                        shortcut_display: "Ctrl+Left".to_string(),
                        description: "Scroll the canvas left".to_string(),
                    },
                    ShortcutEntry {
                        action: "Scroll Right".to_string(),
                        shortcut_display: "Ctrl+Right".to_string(),
                        description: "Scroll the canvas right".to_string(),
                    },
                    ShortcutEntry {
                        action: "Next Page".to_string(),
                        shortcut_display: "Ctrl+Page Down".to_string(),
                        description: "Go to the next page".to_string(),
                    },
                    ShortcutEntry {
                        action: "Previous Page".to_string(),
                        shortcut_display: "Ctrl+Page Up".to_string(),
                        description: "Go to the previous page".to_string(),
                    },
                    ShortcutEntry {
                        action: "Tab to Next Object".to_string(),
                        shortcut_display: "Tab".to_string(),
                        description: "Select the next object in document order".to_string(),
                    },
                    ShortcutEntry {
                        action: "Tab to Previous Object".to_string(),
                        shortcut_display: "Shift+Tab".to_string(),
                        description: "Select the previous object in document order".to_string(),
                    },
                ],
            },
        ];

        Self {
            categories,
            search_filter: String::new(),
        }
    }

    /// Search all shortcuts by matching `query` (case-insensitive) against
    /// action names and descriptions. Returns references to matching entries.
    pub fn search<'a>(&'a self, query: &str) -> Vec<&'a ShortcutEntry> {
        if query.is_empty() {
            return self
                .categories
                .iter()
                .flat_map(|c| c.shortcuts.iter())
                .collect();
        }

        let q = query.to_lowercase();
        self.categories
            .iter()
            .flat_map(|c| c.shortcuts.iter())
            .filter(|entry| {
                entry.action.to_lowercase().contains(&q)
                    || entry.description.to_lowercase().contains(&q)
                    || entry.shortcut_display.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Update the search filter and return matching entries.
    pub fn set_filter(&mut self, query: &str) -> Vec<&ShortcutEntry> {
        self.search_filter = query.to_string();
        self.search(query)
    }

    /// Get all shortcut entries across all categories.
    pub fn all_shortcuts(&self) -> Vec<&ShortcutEntry> {
        self.categories
            .iter()
            .flat_map(|c| c.shortcuts.iter())
            .collect()
    }

    /// Get the number of categories.
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }

    /// Get the total number of shortcut entries.
    pub fn shortcut_count(&self) -> usize {
        self.categories.iter().map(|c| c.shortcuts.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_reference_has_all_categories() {
        let panel = ShortcutReferencePanel::build_reference();
        let names: Vec<&str> = panel.categories.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"File"));
        assert!(names.contains(&"Edit"));
        assert!(names.contains(&"View"));
        assert!(names.contains(&"Object"));
        assert!(names.contains(&"Path"));
        assert!(names.contains(&"Text"));
        assert!(names.contains(&"Tools"));
        assert!(names.contains(&"Navigation"));
        assert_eq!(panel.category_count(), 8);
    }

    #[test]
    fn build_reference_has_shortcuts() {
        let panel = ShortcutReferencePanel::build_reference();
        assert!(panel.shortcut_count() > 50, "should have many shortcuts");
        for cat in &panel.categories {
            assert!(!cat.shortcuts.is_empty(), "category {} should have shortcuts", cat.name);
        }
    }

    #[test]
    fn search_empty_returns_all() {
        let panel = ShortcutReferencePanel::build_reference();
        let results = panel.search("");
        assert_eq!(results.len(), panel.shortcut_count());
    }

    #[test]
    fn search_by_action_name() {
        let panel = ShortcutReferencePanel::build_reference();
        let results = panel.search("undo");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.action == "Undo"));
    }

    #[test]
    fn search_by_shortcut_display() {
        let panel = ShortcutReferencePanel::build_reference();
        let results = panel.search("Ctrl+Z");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.shortcut_display == "Ctrl+Z"));
    }

    #[test]
    fn search_by_description() {
        let panel = ShortcutReferencePanel::build_reference();
        let results = panel.search("clipboard");
        assert!(results.len() >= 2, "cut and copy mention clipboard");
    }

    #[test]
    fn search_case_insensitive() {
        let panel = ShortcutReferencePanel::build_reference();
        let upper = panel.search("ZOOM");
        let lower = panel.search("zoom");
        assert_eq!(upper.len(), lower.len());
        assert!(!upper.is_empty());
    }

    #[test]
    fn search_no_match() {
        let panel = ShortcutReferencePanel::build_reference();
        let results = panel.search("xyznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn set_filter_updates_state() {
        let mut panel = ShortcutReferencePanel::build_reference();
        let results = panel.set_filter("save");
        assert!(!results.is_empty());
        assert_eq!(panel.search_filter, "save");
    }

    #[test]
    fn all_shortcuts_count_matches() {
        let panel = ShortcutReferencePanel::build_reference();
        assert_eq!(panel.all_shortcuts().len(), panel.shortcut_count());
    }
}
