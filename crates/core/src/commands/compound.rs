//! Command: compound (batch) command.

use sivgo_dom::SvgDocument;

use crate::command::Command;

/// Groups multiple commands into a single undo step.
#[derive(Debug)]
pub struct CompoundCommand {
    commands: Vec<Box<dyn Command>>,
    desc: String,
}

impl CompoundCommand {
    /// Create a compound command from a list of sub-commands.
    pub fn new(desc: impl Into<String>, commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            commands,
            desc: desc.into(),
        }
    }
}

impl Command for CompoundCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        for cmd in &mut self.commands {
            cmd.execute(doc);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Undo in reverse order.
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(doc);
        }
    }

    fn description(&self) -> &str {
        &self.desc
    }
}
