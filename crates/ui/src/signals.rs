//! Editor state management / signal tracking (11.7.1-11.7.6).
//!
//! Provides version-counter-based change detection that UI components can
//! use to determine which parts of the editor state have changed and need
//! to be re-rendered.

/// Tracks version counters for various aspects of the editor state.
///
/// Each counter is incremented when the corresponding state changes.
/// UI components can compare two snapshots to determine what needs updating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSignals {
    /// Incremented on every document mutation (add/remove/modify elements).
    pub document_version: u64,
    /// Incremented whenever the selection changes.
    pub selection_version: u64,
    /// Incremented whenever the active tool changes.
    pub tool_version: u64,
    /// Incremented whenever the viewport (zoom, pan, rotation) changes.
    pub viewport_version: u64,
    /// Whether undo is currently available.
    pub can_undo: bool,
    /// Whether redo is currently available.
    pub can_redo: bool,
}

/// Describes which signal categories have changed between two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalChanges {
    /// Whether the document content has changed.
    pub document: bool,
    /// Whether the selection has changed.
    pub selection: bool,
    /// Whether the active tool has changed.
    pub tool: bool,
    /// Whether the viewport has changed.
    pub viewport: bool,
    /// Whether the undo/redo availability has changed.
    pub undo_redo: bool,
}

impl SignalChanges {
    /// Return `true` if any signal has changed.
    pub fn any(&self) -> bool {
        self.document || self.selection || self.tool || self.viewport || self.undo_redo
    }

    /// Return `true` if nothing has changed.
    pub fn none(&self) -> bool {
        !self.any()
    }
}

impl Default for EditorSignals {
    fn default() -> Self {
        Self {
            document_version: 0,
            selection_version: 0,
            tool_version: 0,
            viewport_version: 0,
            can_undo: false,
            can_redo: false,
        }
    }
}

impl EditorSignals {
    /// Create a new signal tracker with all counters at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bump the document version counter.
    pub fn notify_document_changed(&mut self) {
        self.document_version += 1;
    }

    /// Bump the selection version counter.
    pub fn notify_selection_changed(&mut self) {
        self.selection_version += 1;
    }

    /// Bump the tool version counter.
    pub fn notify_tool_changed(&mut self) {
        self.tool_version += 1;
    }

    /// Bump the viewport version counter.
    pub fn notify_viewport_changed(&mut self) {
        self.viewport_version += 1;
    }

    /// Update undo/redo availability flags.
    pub fn set_undo_redo(&mut self, can_undo: bool, can_redo: bool) {
        self.can_undo = can_undo;
        self.can_redo = can_redo;
    }

    /// Compare this snapshot against an older one and return which categories
    /// have changed.
    pub fn changed_since(&self, other: &EditorSignals) -> SignalChanges {
        SignalChanges {
            document: self.document_version != other.document_version,
            selection: self.selection_version != other.selection_version,
            tool: self.tool_version != other.tool_version,
            viewport: self.viewport_version != other.viewport_version,
            undo_redo: self.can_undo != other.can_undo || self.can_redo != other.can_redo,
        }
    }

    /// Take a snapshot of the current state (clone).
    pub fn snapshot(&self) -> EditorSignals {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_signals_are_equal() {
        let a = EditorSignals::new();
        let b = EditorSignals::new();
        let changes = a.changed_since(&b);
        assert!(changes.none());
    }

    #[test]
    fn document_change_detected() {
        let old = EditorSignals::new();
        let mut current = old;
        current.notify_document_changed();
        let changes = current.changed_since(&old);
        assert!(changes.document);
        assert!(!changes.selection);
        assert!(!changes.tool);
        assert!(!changes.viewport);
        assert!(changes.any());
    }

    #[test]
    fn multiple_changes_detected() {
        let old = EditorSignals::new();
        let mut current = old;
        current.notify_document_changed();
        current.notify_selection_changed();
        current.notify_viewport_changed();
        let changes = current.changed_since(&old);
        assert!(changes.document);
        assert!(changes.selection);
        assert!(!changes.tool);
        assert!(changes.viewport);
    }
}
