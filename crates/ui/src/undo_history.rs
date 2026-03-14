//! Undo History panel displaying a list of commands with the ability
//! to jump to any point in the history.

use dioxus::prelude::*;

/// A single entry in the undo history.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// Zero-based index in the history stack.
    pub index: usize,
    /// Human-readable description of the command.
    pub description: String,
    /// Whether this entry is the current state.
    pub is_current: bool,
}

impl HistoryEntry {
    /// Create a new history entry.
    pub fn new(index: usize, description: impl Into<String>, is_current: bool) -> Self {
        Self {
            index,
            description: description.into(),
            is_current,
        }
    }
}

/// Events emitted by the Undo History panel.
#[derive(Debug, Clone)]
pub enum UndoHistoryAction {
    /// Jump to a specific point in the history by index.
    JumpTo(usize),
    /// Undo one step.
    Undo,
    /// Redo one step.
    Redo,
    /// Clear all history.
    Clear,
}

/// A panel showing the undo/redo history as a list of commands.
/// The user can click any entry to jump to that state.
#[component]
pub fn UndoHistoryPanel(
    entries: Vec<HistoryEntry>,
    current_index: usize,
    on_action: EventHandler<UndoHistoryAction>,
) -> Element {
    rsx! {
        div { class: "undo-history-panel",
            div { class: "panel-header", "Undo History" }

            // Toolbar
            div { class: "undo-history-toolbar",
                button {
                    class: "action-btn",
                    title: "Undo",
                    disabled: current_index == 0,
                    onclick: move |_| on_action.call(UndoHistoryAction::Undo),
                    "Undo"
                }
                button {
                    class: "action-btn",
                    title: "Redo",
                    disabled: entries.is_empty() || current_index >= entries.len().saturating_sub(1),
                    onclick: move |_| on_action.call(UndoHistoryAction::Redo),
                    "Redo"
                }
                button {
                    class: "action-btn",
                    title: "Clear history",
                    disabled: entries.is_empty(),
                    onclick: move |_| on_action.call(UndoHistoryAction::Clear),
                    "Clear"
                }
            }

            // History list
            div { class: "undo-history-list",
                for entry in entries.iter() {
                    {
                        let idx = entry.index;
                        let is_past = entry.index <= current_index;
                        let class_str = if entry.is_current {
                            "history-item current"
                        } else if is_past {
                            "history-item past"
                        } else {
                            "history-item future"
                        };
                        rsx! {
                            div {
                                class: "{class_str}",
                                onclick: move |_| on_action.call(UndoHistoryAction::JumpTo(idx)),
                                span { class: "history-index", "{entry.index}" }
                                span { class: "history-description", "{entry.description}" }
                            }
                        }
                    }
                }
                if entries.is_empty() {
                    div { class: "history-empty", "No history" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_entry_new() {
        let entry = HistoryEntry::new(0, "Initial state", true);
        assert_eq!(entry.index, 0);
        assert_eq!(entry.description, "Initial state");
        assert!(entry.is_current);
    }

    #[test]
    fn history_entry_ordering() {
        let entries = vec![
            HistoryEntry::new(0, "Initial", false),
            HistoryEntry::new(1, "Draw rect", false),
            HistoryEntry::new(2, "Move rect", true),
            HistoryEntry::new(3, "Change color", false),
        ];
        let current = entries.iter().find(|e| e.is_current).unwrap();
        assert_eq!(current.index, 2);
        assert_eq!(current.description, "Move rect");

        let past: Vec<_> = entries
            .iter()
            .filter(|e| e.index <= current.index)
            .collect();
        assert_eq!(past.len(), 3);
    }
}
