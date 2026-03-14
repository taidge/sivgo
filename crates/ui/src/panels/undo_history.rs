//! Undo History panel data model (11.4.12).
//!
//! Displays a linear list of undoable / redoable actions and supports
//! jumping to any point in the history.

/// A single entry in the undo/redo history.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// Zero-based position in the history list.
    pub index: usize,
    /// Human-readable description of the action.
    pub description: String,
    /// Timestamp as seconds since the session started (or Unix epoch).
    pub timestamp: f64,
}

impl HistoryEntry {
    /// Create a new history entry.
    pub fn new(index: usize, description: impl Into<String>, timestamp: f64) -> Self {
        Self {
            index,
            description: description.into(),
            timestamp,
        }
    }
}

/// State for the Undo History panel.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoHistoryPanel {
    /// Ordered list of history entries, oldest first.
    entries: Vec<HistoryEntry>,
    /// Index of the entry that represents the current document state.
    /// All entries after this index are "redo" candidates.
    current_index: usize,
}

impl Default for UndoHistoryPanel {
    fn default() -> Self {
        Self {
            entries: vec![HistoryEntry::new(0, "Initial State", 0.0)],
            current_index: 0,
        }
    }
}

impl UndoHistoryPanel {
    /// Return a reference to all history entries.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Return the current position in the history.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Jump to a specific history entry.
    ///
    /// Returns `true` if the jump was valid, `false` if the index was out of
    /// bounds.
    pub fn jump_to(&mut self, index: usize) -> bool {
        if index < self.entries.len() {
            self.current_index = index;
            true
        } else {
            false
        }
    }

    /// Append a new action to the history, discarding any redo entries beyond
    /// the current position.
    pub fn push(&mut self, description: impl Into<String>, timestamp: f64) {
        // Discard future entries when a new action branches off.
        self.entries.truncate(self.current_index + 1);
        let index = self.entries.len();
        self.entries
            .push(HistoryEntry::new(index, description, timestamp));
        self.current_index = index;
    }

    /// Return the description of the current entry.
    pub fn current_description(&self) -> &str {
        &self.entries[self.current_index].description
    }

    /// Return how many undo steps are available.
    pub fn undo_count(&self) -> usize {
        self.current_index
    }

    /// Return how many redo steps are available.
    pub fn redo_count(&self) -> usize {
        self.entries.len() - 1 - self.current_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_initial_state() {
        let panel = UndoHistoryPanel::default();
        assert_eq!(panel.entries().len(), 1);
        assert_eq!(panel.current_index(), 0);
        assert_eq!(panel.current_description(), "Initial State");
    }

    #[test]
    fn push_advances_index() {
        let mut panel = UndoHistoryPanel::default();
        panel.push("Draw Rectangle", 1.0);
        panel.push("Change Fill", 2.0);
        assert_eq!(panel.entries().len(), 3);
        assert_eq!(panel.current_index(), 2);
        assert_eq!(panel.current_description(), "Change Fill");
    }

    #[test]
    fn jump_to_valid() {
        let mut panel = UndoHistoryPanel::default();
        panel.push("A", 1.0);
        panel.push("B", 2.0);
        assert!(panel.jump_to(1));
        assert_eq!(panel.current_index(), 1);
        assert_eq!(panel.current_description(), "A");
    }

    #[test]
    fn jump_to_invalid() {
        let mut panel = UndoHistoryPanel::default();
        assert!(!panel.jump_to(99));
        assert_eq!(panel.current_index(), 0);
    }

    #[test]
    fn push_after_undo_discards_redo() {
        let mut panel = UndoHistoryPanel::default();
        panel.push("A", 1.0);
        panel.push("B", 2.0);
        panel.push("C", 3.0);
        // Undo back to A.
        panel.jump_to(1);
        // New action should discard B and C.
        panel.push("D", 4.0);
        assert_eq!(panel.entries().len(), 3); // Initial, A, D
        assert_eq!(panel.current_index(), 2);
        assert_eq!(panel.current_description(), "D");
    }

    #[test]
    fn undo_redo_counts() {
        let mut panel = UndoHistoryPanel::default();
        panel.push("A", 1.0);
        panel.push("B", 2.0);
        assert_eq!(panel.undo_count(), 2);
        assert_eq!(panel.redo_count(), 0);
        panel.jump_to(1);
        assert_eq!(panel.undo_count(), 1);
        assert_eq!(panel.redo_count(), 1);
    }
}
