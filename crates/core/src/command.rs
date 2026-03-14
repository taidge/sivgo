//! Command trait and undo/redo history.
//!
//! Every editing operation is wrapped in a [`Command`] so that it can be
//! undone and redone.  The [`CommandHistory`] manages two stacks (undo and
//! redo) and enforces a maximum depth.

use sivgo_dom::SvgDocument;

/// A reversible editing command.
pub trait Command: std::fmt::Debug {
    /// Apply the command to the document.
    fn execute(&mut self, doc: &mut SvgDocument);

    /// Reverse the effect of [`execute`](Self::execute).
    fn undo(&mut self, doc: &mut SvgDocument);

    /// A short, human-readable description (shown in Edit menu).
    fn description(&self) -> &str;
}

/// Manages the undo and redo stacks.
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_depth: usize,
}

impl CommandHistory {
    /// Create a new history with the given maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth,
        }
    }

    /// Execute a command, push it onto the undo stack, and clear the redo
    /// stack.
    pub fn execute(&mut self, mut cmd: Box<dyn Command>, doc: &mut SvgDocument) {
        cmd.execute(doc);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();

        // Enforce max depth.
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the most recent command.  Returns `true` if there was something
    /// to undo.
    pub fn undo(&mut self, doc: &mut SvgDocument) -> bool {
        if let Some(mut cmd) = self.undo_stack.pop() {
            cmd.undo(doc);
            self.redo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    /// Redo the most recently undone command.  Returns `true` if there was
    /// something to redo.
    pub fn redo(&mut self, doc: &mut SvgDocument) -> bool {
        if let Some(mut cmd) = self.redo_stack.pop() {
            cmd.execute(doc);
            self.undo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    /// Whether there is at least one command that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is at least one command that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Description of the command that would be undone next.
    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|cmd| cmd.description())
    }

    /// Description of the command that would be redone next.
    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|cmd| cmd.description())
    }

    /// Clear both stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl std::fmt::Debug for CommandHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandHistory")
            .field("undo_depth", &self.undo_stack.len())
            .field("redo_depth", &self.redo_stack.len())
            .field("max_depth", &self.max_depth)
            .finish()
    }
}
