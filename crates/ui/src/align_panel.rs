//! Align & Distribute panel with alignment buttons, distribution buttons,
//! and a "relative to" selector.

use crate::controls::{action_button, choice_field, panel};
use glory::prelude::*;

/// Alignment operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

/// Distribution operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistributeOp {
    HorizontalGaps,
    VerticalGaps,
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

/// What the alignment/distribution is relative to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignRelativeTo {
    Selection,
    Page,
}

/// Events emitted by the Align & Distribute panel.
#[derive(Debug, Clone)]
pub enum AlignAction {
    Align(AlignOp),
    Distribute(DistributeOp),
    SetRelativeTo(AlignRelativeTo),
}

/// A panel providing alignment and distribution controls for selected objects.
///
/// Includes 6 alignment buttons, 6 distribute buttons, and a "relative to"
/// selector (selection or page).
#[derive(Clone)]
pub struct AlignPanel {
    pub relative_to: String,
    pub on_action: crate::Callback<AlignAction>,
}

impl Widget for AlignPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "align-panel", "Align & Distribute")?;
        let callback = self.on_action.clone();
        choice_field(
            &scope,
            "Relative to",
            self.relative_to.clone(),
            &["Selection", "Page"],
            move |v| {
                callback.call(AlignAction::SetRelativeTo(if v == "Page" {
                    AlignRelativeTo::Page
                } else {
                    AlignRelativeTo::Selection
                }))
            },
        )?;
        for (label, op) in [
            ("Align left", AlignOp::Left),
            ("Center horizontally", AlignOp::CenterH),
            ("Align right", AlignOp::Right),
            ("Align top", AlignOp::Top),
            ("Center vertically", AlignOp::CenterV),
            ("Align bottom", AlignOp::Bottom),
        ] {
            let callback = self.on_action.clone();
            action_button(&scope, label, false, move || {
                callback.call(AlignAction::Align(op.clone()))
            })?;
        }
        for (label, op) in [
            ("Equal horizontal gaps", DistributeOp::HorizontalGaps),
            ("Equal vertical gaps", DistributeOp::VerticalGaps),
            ("Distribute left edges", DistributeOp::Left),
            ("Distribute horizontal centers", DistributeOp::CenterH),
            ("Distribute right edges", DistributeOp::Right),
            ("Distribute top edges", DistributeOp::Top),
            ("Distribute vertical centers", DistributeOp::CenterV),
            ("Distribute bottom edges", DistributeOp::Bottom),
        ] {
            let callback = self.on_action.clone();
            action_button(&scope, label, false, move || {
                callback.call(AlignAction::Distribute(op.clone()))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use glory::render::{NodeId, Props};
    use std::{cell::RefCell, rc::Rc};

    fn button(app: &App, label: &str) -> NodeId {
        let tree = app.tree();
        let mut nodes = tree.roots().to_vec();
        while let Some(id) = nodes.pop() {
            let node = tree.node(id).unwrap();
            if matches!(node.descriptor.view().props, Props::Button(props) if props.label == label)
            {
                return id;
            }
            nodes.extend(node.children.iter().copied());
        }
        panic!("missing button: {label}");
    }

    #[test]
    fn alignment_and_distribution_controls_dispatch_typed_actions() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let captured = actions.clone();
        let app = App::memory();
        app.mount(AlignPanel {
            relative_to: "Selection".into(),
            on_action: crate::Callback::new(move |a| captured.borrow_mut().push(a)),
        })
        .unwrap();
        app.dispatch(button(&app, "Align left"), Event::Activate)
            .unwrap();
        app.dispatch(button(&app, "Distribute bottom edges"), Event::Activate)
            .unwrap();
        assert!(matches!(
            actions.borrow().as_slice(),
            [
                AlignAction::Align(AlignOp::Left),
                AlignAction::Distribute(DistributeOp::Bottom)
            ]
        ));
        app.clear().unwrap();
        assert_eq!(app.stats().nodes, 0);
    }
}
