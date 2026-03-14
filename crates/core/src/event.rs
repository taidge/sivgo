//! Editor event bus.
//!
//! Events are collected and can be polled by the UI layer.

use crate::tool::ToolId;

/// Events emitted by the editor engine.
#[derive(Debug, Clone)]
pub enum EditorEvent {
    /// The document was modified (a command was executed, undone, or redone).
    DocumentModified,
    /// The selection changed.
    SelectionChanged,
    /// The active tool changed.
    ToolChanged(ToolId),
    /// The viewport (pan/zoom) changed.
    ViewportChanged,
    /// The undo/redo availability changed.
    UndoRedoChanged,
}

/// A simple event collector.  Callers emit events and the UI layer drains
/// them each frame.
#[derive(Debug, Default)]
pub struct EventBus {
    pending: Vec<EditorEvent>,
}

impl EventBus {
    /// Create an empty event bus.
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Queue an event.
    pub fn emit(&mut self, event: EditorEvent) {
        self.pending.push(event);
    }

    /// Take all pending events, leaving the bus empty.
    pub fn drain(&mut self) -> Vec<EditorEvent> {
        std::mem::take(&mut self.pending)
    }
}
