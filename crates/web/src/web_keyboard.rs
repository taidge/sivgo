//! Web keyboard handling and shortcut interception.
//!
//! Manages browser keyboard events, preventing default browser behaviour
//! for editor shortcuts and translating JavaScript key names to
//! application-level key identifiers.

use std::collections::HashSet;

/// Tracks which keyboard combinations should have their browser default
/// behaviour suppressed, and provides key translation.
#[derive(Debug, Clone)]
pub struct WebKeyboardHandler {
    /// Set of key combo strings (e.g. `"Ctrl+S"`) whose browser defaults
    /// should be prevented.
    pub prevented_defaults: HashSet<String>,
    /// Whether a shortcut is currently being processed.
    pub shortcut_active: bool,
}

impl Default for WebKeyboardHandler {
    fn default() -> Self {
        let mut prevented = HashSet::new();
        // Standard editor shortcuts that must not trigger browser behaviour.
        for combo in &[
            "Ctrl+S", "Ctrl+Z", "Ctrl+Y", "Ctrl+A", "Ctrl+N", "Ctrl+O", "Ctrl+P", "F5",
        ] {
            prevented.insert((*combo).to_string());
        }
        Self {
            prevented_defaults: prevented,
            shortcut_active: false,
        }
    }
}

impl WebKeyboardHandler {
    /// Create a handler with the default prevented shortcuts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determine whether the browser default should be prevented for a
    /// given key event.
    ///
    /// The combination is built from the modifier flags and key name and
    /// checked against the `prevented_defaults` set.
    pub fn should_prevent_default(
        &self,
        key: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        let combo = Self::build_combo(key, ctrl, shift, alt);
        self.prevented_defaults.contains(&combo)
    }

    /// Register an additional key combination to prevent.
    ///
    /// The combo should be in the format `"Ctrl+Shift+S"`, `"Alt+F4"`, etc.
    pub fn register_prevention(&mut self, combo: &str) {
        self.prevented_defaults.insert(combo.to_string());
    }

    /// Remove a key combination from the prevention set.
    pub fn unregister_prevention(&mut self, combo: &str) {
        self.prevented_defaults.remove(combo);
    }

    /// Translate a JavaScript `KeyboardEvent.key` value to an
    /// application-level key name.
    ///
    /// Returns `None` for keys that have no meaningful editor mapping.
    pub fn translate_web_key(js_key: &str) -> Option<String> {
        let mapped = match js_key {
            "ArrowUp" => "Up",
            "ArrowDown" => "Down",
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            "Escape" => "Escape",
            "Enter" => "Enter",
            "Backspace" => "Backspace",
            "Delete" => "Delete",
            "Tab" => "Tab",
            " " => "Space",
            "Control" | "Shift" | "Alt" | "Meta" => return None, // modifiers alone
            k if k.starts_with('F') && k.len() <= 3 => {
                // F1-F12
                if k[1..].parse::<u8>().is_ok() {
                    return Some(k.to_string());
                }
                return Some(k.to_string());
            }
            k if k.len() == 1 => {
                // Single printable character -- uppercase for consistency.
                return Some(k.to_uppercase());
            }
            _ => return None,
        };
        Some(mapped.to_string())
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Build a canonical combo string from modifiers + key.
    fn build_combo(key: &str, ctrl: bool, shift: bool, alt: bool) -> String {
        let mut parts = Vec::new();
        if ctrl {
            parts.push("Ctrl");
        }
        if shift {
            parts.push("Shift");
        }
        if alt {
            parts.push("Alt");
        }
        parts.push(key);
        parts.join("+")
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preventions() {
        let handler = WebKeyboardHandler::new();
        assert!(handler.should_prevent_default("S", true, false, false)); // Ctrl+S
        assert!(handler.should_prevent_default("Z", true, false, false)); // Ctrl+Z
        assert!(handler.should_prevent_default("Y", true, false, false)); // Ctrl+Y
        assert!(handler.should_prevent_default("A", true, false, false)); // Ctrl+A
        assert!(handler.should_prevent_default("N", true, false, false)); // Ctrl+N
        assert!(handler.should_prevent_default("O", true, false, false)); // Ctrl+O
        assert!(handler.should_prevent_default("P", true, false, false)); // Ctrl+P
        assert!(handler.should_prevent_default("F5", false, false, false)); // F5
    }

    #[test]
    fn non_prevented_key_returns_false() {
        let handler = WebKeyboardHandler::new();
        assert!(!handler.should_prevent_default("X", true, false, false));
        assert!(!handler.should_prevent_default("A", false, false, false)); // no ctrl
    }

    #[test]
    fn register_and_unregister_prevention() {
        let mut handler = WebKeyboardHandler::new();
        handler.register_prevention("Ctrl+Shift+S");
        assert!(handler.should_prevent_default("S", true, true, false));

        handler.unregister_prevention("Ctrl+Shift+S");
        assert!(!handler.should_prevent_default("S", true, true, false));
    }

    #[test]
    fn translate_arrow_keys() {
        assert_eq!(
            WebKeyboardHandler::translate_web_key("ArrowUp"),
            Some("Up".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("ArrowDown"),
            Some("Down".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("ArrowLeft"),
            Some("Left".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("ArrowRight"),
            Some("Right".into())
        );
    }

    #[test]
    fn translate_special_keys() {
        assert_eq!(
            WebKeyboardHandler::translate_web_key("Escape"),
            Some("Escape".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("Enter"),
            Some("Enter".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("Delete"),
            Some("Delete".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key(" "),
            Some("Space".into())
        );
    }

    #[test]
    fn translate_function_keys() {
        assert_eq!(
            WebKeyboardHandler::translate_web_key("F1"),
            Some("F1".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("F12"),
            Some("F12".into())
        );
    }

    #[test]
    fn translate_single_character() {
        assert_eq!(
            WebKeyboardHandler::translate_web_key("a"),
            Some("A".into())
        );
        assert_eq!(
            WebKeyboardHandler::translate_web_key("Z"),
            Some("Z".into())
        );
    }

    #[test]
    fn translate_modifiers_return_none() {
        assert!(WebKeyboardHandler::translate_web_key("Control").is_none());
        assert!(WebKeyboardHandler::translate_web_key("Shift").is_none());
        assert!(WebKeyboardHandler::translate_web_key("Alt").is_none());
        assert!(WebKeyboardHandler::translate_web_key("Meta").is_none());
    }

    #[test]
    fn build_combo() {
        assert_eq!(
            WebKeyboardHandler::build_combo("S", true, false, false),
            "Ctrl+S"
        );
        assert_eq!(
            WebKeyboardHandler::build_combo("S", true, true, false),
            "Ctrl+Shift+S"
        );
        assert_eq!(
            WebKeyboardHandler::build_combo("F5", false, false, false),
            "F5"
        );
        assert_eq!(
            WebKeyboardHandler::build_combo("Z", true, false, true),
            "Ctrl+Alt+Z"
        );
    }
}
