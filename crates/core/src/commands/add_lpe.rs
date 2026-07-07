//! Command: append a Live Path Effect to a `<path>` element's LPE stack.
//!
//! This is the command emitted by the LPE tool when the user applies an effect
//! to a clicked path. It reuses the engine's LPE-stack encode/decode helpers so
//! the on-canvas tool and the LPE panel produce identical, undoable edits.

use sivgo_dom::{NodeId, PresentationAttrs, SvgDocument, SvgElement};

use crate::command::Command;
use crate::engine::{EditorEngine, EngineLpeEntry};

/// Append an LPE of the given `kind` to the LPE stack of `node_id`.
#[derive(Debug)]
pub struct AddLpeCommand {
    node_id: NodeId,
    kind: String,
    /// Captured at execute time so the edit can be undone.
    old_element: Option<SvgElement>,
    old_presentation: Option<PresentationAttrs>,
}

impl AddLpeCommand {
    /// Create a command that adds the named effect to `node_id`. The kind is
    /// normalised to the engine's canonical spelling; an unknown kind is kept
    /// verbatim (and will simply add no usable entry).
    pub fn new(node_id: NodeId, kind: &str) -> Self {
        let kind = EditorEngine::normalize_lpe_kind(kind)
            .map(str::to_string)
            .unwrap_or_else(|| kind.to_string());
        Self {
            node_id,
            kind,
            old_element: None,
            old_presentation: None,
        }
    }
}

impl Command for AddLpeCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        let Some(node) = doc.get_node(self.node_id) else {
            return;
        };
        let SvgElement::Path(attrs) = &node.element else {
            return;
        };

        let mut entries = EditorEngine::decode_lpe_stack(attrs.inkscape_path_effect.as_deref());
        let id = EditorEngine::next_lpe_id(&entries);
        let params = EditorEngine::default_lpe_params(&self.kind);
        entries.push(EngineLpeEntry {
            id,
            kind: self.kind.clone(),
            enabled: true,
            params,
        });
        let new_attrs = EditorEngine::rebuild_lpe_path_attrs(attrs, &entries);

        self.old_element = Some(node.element.clone());
        self.old_presentation = Some(node.presentation.clone());

        if let Some(node) = doc.get_node_mut(self.node_id) {
            node.element = SvgElement::Path(new_attrs);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        if let (Some(element), Some(presentation)) =
            (self.old_element.as_ref(), self.old_presentation.as_ref())
        {
            if let Some(node) = doc.get_node_mut(self.node_id) {
                node.element = element.clone();
                node.presentation = presentation.clone();
            }
        }
    }

    fn description(&self) -> &str {
        "Add LPE"
    }
}
