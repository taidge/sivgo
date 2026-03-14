//! Plugin UI primitives for building configuration dialogs.
//!
//! Plugins describe the UI they need declaratively via [`PluginDialog`]
//! and [`DialogElement`].  The editor host renders the dialog using its
//! own widget toolkit and returns a [`DialogResult`] containing the
//! user's input.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Dialog definition
// ---------------------------------------------------------------------------

/// A dialog window that a plugin asks the host to display.
#[derive(Debug, Clone)]
pub struct PluginDialog {
    /// Window / panel title.
    pub title: String,
    /// Ordered list of UI elements to render.
    pub elements: Vec<DialogElement>,
}

impl PluginDialog {
    /// Create a new dialog with the given title and no elements.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            elements: Vec::new(),
        }
    }

    /// Fluent builder: append an element.
    pub fn with(mut self, element: DialogElement) -> Self {
        self.elements.push(element);
        self
    }
}

/// Individual widget that can appear inside a [`PluginDialog`].
#[derive(Debug, Clone)]
pub enum DialogElement {
    /// Static text label (no associated value in the result).
    Label(String),

    /// Single-line text input.
    TextInput {
        id: String,
        label: String,
        default: String,
    },

    /// Numeric spinner / text input clamped to `[min, max]`.
    NumberInput {
        id: String,
        label: String,
        default: f64,
        min: f64,
        max: f64,
    },

    /// Boolean toggle.
    Checkbox {
        id: String,
        label: String,
        default: bool,
    },

    /// RGB colour picker.
    ColorPicker {
        id: String,
        label: String,
        default: [u8; 3],
    },

    /// Drop-down selection list.
    Dropdown {
        id: String,
        label: String,
        options: Vec<String>,
        /// Index of the initially selected option.
        default: usize,
    },

    /// A clickable button (triggers an action, not a value).
    Button { id: String, label: String },

    /// A horizontal separator / divider.
    Separator,
}

// ---------------------------------------------------------------------------
// Dialog result
// ---------------------------------------------------------------------------

/// The values collected from a rendered [`PluginDialog`] after the user
/// confirms.
#[derive(Debug, Clone)]
pub struct DialogResult {
    /// Mapping from element `id` to the value the user chose.
    pub values: HashMap<String, DialogValue>,
}

impl DialogResult {
    /// Create an empty result.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Insert a value.
    pub fn set(&mut self, id: impl Into<String>, value: DialogValue) {
        self.values.insert(id.into(), value);
    }

    /// Retrieve a text value, returning `None` if the id is missing or
    /// the value is not text.
    pub fn get_text(&self, id: &str) -> Option<&str> {
        match self.values.get(id)? {
            DialogValue::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Retrieve a number value.
    pub fn get_number(&self, id: &str) -> Option<f64> {
        match self.values.get(id)? {
            DialogValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Retrieve a boolean value.
    pub fn get_bool(&self, id: &str) -> Option<bool> {
        match self.values.get(id)? {
            DialogValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Retrieve a colour value.
    pub fn get_color(&self, id: &str) -> Option<[u8; 3]> {
        match self.values.get(id)? {
            DialogValue::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// Retrieve a choice (dropdown) index.
    pub fn get_choice(&self, id: &str) -> Option<usize> {
        match self.values.get(id)? {
            DialogValue::Choice(i) => Some(*i),
            _ => None,
        }
    }
}

impl Default for DialogResult {
    fn default() -> Self {
        Self::new()
    }
}

/// A concrete value produced by a single dialog element.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Color([u8; 3]),
    Choice(usize),
}

// ---------------------------------------------------------------------------
// Utility: build a DialogResult from the defaults in a PluginDialog
// ---------------------------------------------------------------------------

impl PluginDialog {
    /// Produce a [`DialogResult`] populated with every element's default
    /// value.  Useful for testing or when the user accepts without
    /// changing anything.
    pub fn defaults(&self) -> DialogResult {
        let mut result = DialogResult::new();
        for elem in &self.elements {
            match elem {
                DialogElement::TextInput { id, default, .. } => {
                    result.set(id, DialogValue::Text(default.clone()));
                }
                DialogElement::NumberInput { id, default, .. } => {
                    result.set(id, DialogValue::Number(*default));
                }
                DialogElement::Checkbox { id, default, .. } => {
                    result.set(id, DialogValue::Bool(*default));
                }
                DialogElement::ColorPicker { id, default, .. } => {
                    result.set(id, DialogValue::Color(*default));
                }
                DialogElement::Dropdown { id, default, .. } => {
                    result.set(id, DialogValue::Choice(*default));
                }
                // Label, Button, Separator produce no value.
                _ => {}
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_builder_and_defaults() {
        let dialog = PluginDialog::new("Grid settings")
            .with(DialogElement::Label("Configure the grid:".to_string()))
            .with(DialogElement::NumberInput {
                id: "rows".to_string(),
                label: "Rows".to_string(),
                default: 5.0,
                min: 1.0,
                max: 100.0,
            })
            .with(DialogElement::NumberInput {
                id: "cols".to_string(),
                label: "Columns".to_string(),
                default: 5.0,
                min: 1.0,
                max: 100.0,
            })
            .with(DialogElement::Checkbox {
                id: "fill".to_string(),
                label: "Fill cells".to_string(),
                default: true,
            })
            .with(DialogElement::Separator);

        assert_eq!(dialog.title, "Grid settings");
        assert_eq!(dialog.elements.len(), 5);

        let defaults = dialog.defaults();
        assert_eq!(defaults.get_number("rows"), Some(5.0));
        assert_eq!(defaults.get_number("cols"), Some(5.0));
        assert_eq!(defaults.get_bool("fill"), Some(true));
    }

    #[test]
    fn dialog_result_accessors() {
        let mut result = DialogResult::new();
        result.set("name", DialogValue::Text("hello".to_string()));
        result.set("size", DialogValue::Number(42.0));
        result.set("visible", DialogValue::Bool(false));
        result.set("color", DialogValue::Color([255, 128, 0]));
        result.set("mode", DialogValue::Choice(2));

        assert_eq!(result.get_text("name"), Some("hello"));
        assert_eq!(result.get_number("size"), Some(42.0));
        assert_eq!(result.get_bool("visible"), Some(false));
        assert_eq!(result.get_color("color"), Some([255, 128, 0]));
        assert_eq!(result.get_choice("mode"), Some(2));

        // Type mismatch returns None.
        assert_eq!(result.get_text("size"), None);
        // Missing key returns None.
        assert_eq!(result.get_text("nonexistent"), None);
    }

    #[test]
    fn all_element_variants_constructible() {
        // Verify that every variant can be constructed without issues.
        let elements = vec![
            DialogElement::Label("A label".to_string()),
            DialogElement::TextInput {
                id: "t".to_string(),
                label: "Text".to_string(),
                default: String::new(),
            },
            DialogElement::NumberInput {
                id: "n".to_string(),
                label: "Num".to_string(),
                default: 0.0,
                min: -100.0,
                max: 100.0,
            },
            DialogElement::Checkbox {
                id: "c".to_string(),
                label: "Check".to_string(),
                default: false,
            },
            DialogElement::ColorPicker {
                id: "col".to_string(),
                label: "Color".to_string(),
                default: [0, 0, 0],
            },
            DialogElement::Dropdown {
                id: "d".to_string(),
                label: "Drop".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
                default: 0,
            },
            DialogElement::Button {
                id: "b".to_string(),
                label: "OK".to_string(),
            },
            DialogElement::Separator,
        ];

        assert_eq!(elements.len(), 8);
    }
}
