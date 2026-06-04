//! Keyboard shortcut system for the editor.
//!
//! Maps keyboard combinations to editor actions. Supports customizable
//! mappings and a default set matching Inkscape conventions.

use crate::input::{Key, Modifiers};
use crate::tool::ToolId;

/// An editor action that can be triggered by a keyboard shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    // Tool switching
    SwitchTool(ToolId),

    // File operations
    NewDocument,
    Open,
    Save,
    SaveAs,
    Export,
    Import,
    Quit,

    // Edit operations
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteInPlace,
    PasteStyle,
    Duplicate,
    Clone,
    Delete,
    SelectAll,
    SelectNone,
    Find,

    // View operations
    ZoomIn,
    ZoomOut,
    ZoomFitPage,
    ZoomFitSelection,
    ZoomFitDrawing,
    Zoom100,
    ToggleGrid,
    ToggleGuides,
    ToggleRulers,
    ToggleFullscreen,
    CycleDisplayMode,

    // Object operations
    Group,
    Ungroup,
    Raise,
    Lower,
    RaiseToTop,
    LowerToBottom,
    FlipHorizontal,
    FlipVertical,
    Rotate90CW,
    Rotate90CCW,
    ObjectProperties,

    // Path operations
    ObjectToPath,
    StrokeToPath,
    BoolUnion,
    BoolIntersection,
    BoolDifference,
    BoolExclusion,
    CombinePaths,
    BreakApart,
    SimplifyPath,
    ReversePath,

    // Layer operations
    AddLayer,
    DeleteLayer,
    RenameLayer,
    MoveToLayer,

    // Tab navigation
    TabNext,
    TabPrev,
}

/// A keyboard shortcut definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Shortcut {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Format the shortcut as a human-readable string.
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        parts.push(self.key.display_name());
        parts.join("+")
    }
}

/// Keyboard shortcut manager.
///
/// Stores a mapping from shortcuts to actions and allows lookup.
#[derive(Debug, Clone)]
pub struct ShortcutManager {
    bindings: Vec<(Shortcut, Action)>,
}

impl ShortcutManager {
    /// Create a new shortcut manager with default bindings.
    pub fn new() -> Self {
        let mut manager = Self {
            bindings: Vec::new(),
        };
        manager.load_defaults();
        manager
    }

    /// Look up the action for a key event.
    pub fn lookup(&self, key: Key, modifiers: Modifiers) -> Option<&Action> {
        let shortcut = Shortcut::new(key, modifiers);
        self.bindings
            .iter()
            .find(|(s, _)| *s == shortcut)
            .map(|(_, a)| a)
    }

    /// Add or update a shortcut binding.
    pub fn bind(&mut self, shortcut: Shortcut, action: Action) {
        // Remove existing binding for this shortcut
        self.bindings.retain(|(s, _)| *s != shortcut);
        self.bindings.push((shortcut, action));
    }

    /// Remove a shortcut binding.
    pub fn unbind(&mut self, shortcut: &Shortcut) {
        self.bindings.retain(|(s, _)| s != shortcut);
    }

    /// Get all bindings.
    pub fn bindings(&self) -> &[(Shortcut, Action)] {
        &self.bindings
    }

    /// Find the shortcut(s) for a given action.
    pub fn shortcuts_for(&self, action: &Action) -> Vec<&Shortcut> {
        self.bindings
            .iter()
            .filter(|(_, a)| a == action)
            .map(|(s, _)| s)
            .collect()
    }

    /// Load default keyboard shortcuts (Inkscape-compatible).
    fn load_defaults(&mut self) {
        let ctrl = |key: Key| {
            Shortcut::new(
                key,
                Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            )
        };
        let ctrl_shift = |key: Key| {
            Shortcut::new(
                key,
                Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
            )
        };
        let plain = |key: Key| {
            Shortcut::new(
                key,
                Modifiers {
                    ctrl: false,
                    shift: false,
                    alt: false,
                },
            )
        };
        let shift = |key: Key| {
            Shortcut::new(
                key,
                Modifiers {
                    ctrl: false,
                    shift: true,
                    alt: false,
                },
            )
        };

        // Tool shortcuts
        self.bindings.extend([
            (plain(Key::S), Action::SwitchTool(ToolId::Select)),
            (plain(Key::N), Action::SwitchTool(ToolId::Node)),
            (plain(Key::R), Action::SwitchTool(ToolId::Rect)),
            (plain(Key::E), Action::SwitchTool(ToolId::Ellipse)),
            (plain(Key::P), Action::SwitchTool(ToolId::Pen)),
            (plain(Key::B), Action::SwitchTool(ToolId::Pencil)),
            (plain(Key::T), Action::SwitchTool(ToolId::Text)),
            (plain(Key::G), Action::SwitchTool(ToolId::Gradient)),
            (plain(Key::D), Action::SwitchTool(ToolId::Dropper)),
            (plain(Key::Z), Action::SwitchTool(ToolId::Zoom)),
            (plain(Key::M), Action::SwitchTool(ToolId::Measure)),
        ]);

        // File shortcuts
        self.bindings.extend([
            (ctrl(Key::N), Action::NewDocument),
            (ctrl(Key::O), Action::Open),
            (ctrl(Key::S), Action::Save),
            (ctrl_shift(Key::S), Action::SaveAs),
            (ctrl_shift(Key::E), Action::Export),
            (ctrl(Key::I), Action::Import),
            (ctrl(Key::Q), Action::Quit),
        ]);

        // Edit shortcuts
        self.bindings.extend([
            (ctrl(Key::Z), Action::Undo),
            (ctrl(Key::Y), Action::Redo),
            (ctrl_shift(Key::Z), Action::Redo),
            (ctrl(Key::X), Action::Cut),
            (ctrl(Key::C), Action::Copy),
            (ctrl(Key::V), Action::Paste),
            (ctrl_shift(Key::V), Action::PasteInPlace),
            (ctrl(Key::D), Action::Duplicate),
            (plain(Key::Delete), Action::Delete),
            (ctrl(Key::A), Action::SelectAll),
            (ctrl(Key::F), Action::Find),
        ]);

        // View shortcuts
        self.bindings.extend([
            (plain(Key::Plus), Action::ZoomIn),
            (plain(Key::Minus), Action::ZoomOut),
            (plain(Key::Digit1), Action::Zoom100),
            (plain(Key::Digit3), Action::ZoomFitPage),
            (plain(Key::Digit5), Action::ZoomFitDrawing),
        ]);

        // Object shortcuts
        self.bindings.extend([
            (ctrl(Key::G), Action::Group),
            (ctrl_shift(Key::G), Action::Ungroup),
            (plain(Key::PageUp), Action::Raise),
            (plain(Key::PageDown), Action::Lower),
            (plain(Key::Home), Action::RaiseToTop),
            (plain(Key::End), Action::LowerToBottom),
        ]);

        // Path shortcuts
        self.bindings.extend([
            (ctrl_shift(Key::C), Action::ObjectToPath),
            (ctrl(Key::L), Action::SimplifyPath),
        ]);

        // Navigation
        self.bindings.extend([
            (plain(Key::Tab), Action::TabNext),
            (shift(Key::Tab), Action::TabPrev),
        ]);
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Key {
    /// Human-readable display name for this key.
    fn display_name(&self) -> &'static str {
        match self {
            Key::A => "A",
            Key::B => "B",
            Key::C => "C",
            Key::D => "D",
            Key::E => "E",
            Key::F => "F",
            Key::G => "G",
            Key::H => "H",
            Key::I => "I",
            Key::J => "J",
            Key::K => "K",
            Key::L => "L",
            Key::M => "M",
            Key::N => "N",
            Key::O => "O",
            Key::P => "P",
            Key::Q => "Q",
            Key::R => "R",
            Key::S => "S",
            Key::T => "T",
            Key::U => "U",
            Key::V => "V",
            Key::W => "W",
            Key::X => "X",
            Key::Y => "Y",
            Key::Z => "Z",
            Key::Plus => "+",
            Key::Minus => "-",
            Key::Digit0 => "0",
            Key::Digit1 => "1",
            Key::Digit2 => "2",
            Key::Digit3 => "3",
            Key::Digit4 => "4",
            Key::Digit5 => "5",
            Key::Digit6 => "6",
            Key::Digit7 => "7",
            Key::Digit8 => "8",
            Key::Digit9 => "9",
            Key::Delete => "Delete",
            Key::Backspace => "Backspace",
            Key::Enter => "Enter",
            Key::Escape => "Escape",
            Key::Tab => "Tab",
            Key::Space => "Space",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "Page Up",
            Key::PageDown => "Page Down",
            Key::ArrowUp => "Up",
            Key::ArrowDown => "Down",
            Key::ArrowLeft => "Left",
            Key::ArrowRight => "Right",
            Key::F1 => "F1",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::Char(_) => "?",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_exist() {
        let mgr = ShortcutManager::new();
        assert!(!mgr.bindings().is_empty());
    }

    #[test]
    fn lookup_ctrl_z() {
        let mgr = ShortcutManager::new();
        let action = mgr.lookup(
            Key::Z,
            Modifiers {
                ctrl: true,
                shift: false,
                alt: false,
            },
        );
        assert_eq!(action, Some(&Action::Undo));
    }

    #[test]
    fn lookup_tool_shortcut() {
        let mgr = ShortcutManager::new();
        let action = mgr.lookup(
            Key::R,
            Modifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        );
        assert_eq!(action, Some(&Action::SwitchTool(ToolId::Rect)));
    }

    #[test]
    fn custom_binding() {
        let mut mgr = ShortcutManager::new();
        let shortcut = Shortcut::new(Key::F1, Modifiers::default());
        mgr.bind(shortcut.clone(), Action::ToggleFullscreen);
        assert_eq!(
            mgr.lookup(Key::F1, Modifiers::default()),
            Some(&Action::ToggleFullscreen)
        );
    }

    #[test]
    fn shortcuts_for_action() {
        let mgr = ShortcutManager::new();
        let shortcuts = mgr.shortcuts_for(&Action::Undo);
        assert!(!shortcuts.is_empty());
    }

    #[test]
    fn default_shortcuts_cover_file_tool_and_zoom_interactions() {
        let mgr = ShortcutManager::new();
        assert_eq!(
            mgr.lookup(
                Key::O,
                Modifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                }
            ),
            Some(&Action::Open)
        );
        assert_eq!(
            mgr.lookup(Key::T, Modifiers::none()),
            Some(&Action::SwitchTool(ToolId::Text))
        );
        assert_eq!(
            mgr.lookup(Key::Plus, Modifiers::none()),
            Some(&Action::ZoomIn)
        );
        assert_eq!(
            mgr.lookup(Key::Minus, Modifiers::none()),
            Some(&Action::ZoomOut)
        );
    }

    #[test]
    fn display_format() {
        let s = Shortcut::new(
            Key::Z,
            Modifiers {
                ctrl: true,
                shift: false,
                alt: false,
            },
        );
        assert_eq!(s.display(), "Ctrl+Z");
    }
}
