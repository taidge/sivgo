//! Align & Distribute panel with alignment buttons, distribution buttons,
//! and a "relative to" selector.

use dioxus::prelude::*;

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
#[component]
pub fn AlignPanel(relative_to: String, on_action: EventHandler<AlignAction>) -> Element {
    let rel_value = relative_to.clone();

    rsx! {
        div { class: "align-panel",
            div { class: "panel-header", "Align & Distribute" }

            // Relative-to selector
            div { class: "prop-row",
                label { class: "prop-label", "Relative to" }
                select {
                    class: "prop-select",
                    value: "{rel_value}",
                    onchange: move |evt: Event<FormData>| {
                        let val = evt.value().to_string();
                        let rel = if val == "Page" {
                            AlignRelativeTo::Page
                        } else {
                            AlignRelativeTo::Selection
                        };
                        on_action.call(AlignAction::SetRelativeTo(rel));
                    },
                    option { value: "Selection", selected: rel_value == "Selection", "Selection" }
                    option { value: "Page", selected: rel_value == "Page", "Page" }
                }
            }

            // ---- Alignment buttons ----
            div { class: "align-section",
                div { class: "align-section-header", "Align" }
                div { class: "align-buttons",
                    button {
                        class: "align-btn",
                        title: "Align left edges",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::Left)),
                        "⫷ Left"
                    }
                    button {
                        class: "align-btn",
                        title: "Center on vertical axis",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::CenterH)),
                        "⫿ Center H"
                    }
                    button {
                        class: "align-btn",
                        title: "Align right edges",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::Right)),
                        "⫸ Right"
                    }
                    button {
                        class: "align-btn",
                        title: "Align top edges",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::Top)),
                        "⊤ Top"
                    }
                    button {
                        class: "align-btn",
                        title: "Center on horizontal axis",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::CenterV)),
                        "⊞ Center V"
                    }
                    button {
                        class: "align-btn",
                        title: "Align bottom edges",
                        onclick: move |_| on_action.call(AlignAction::Align(AlignOp::Bottom)),
                        "⊥ Bottom"
                    }
                }
            }

            // ---- Distribution buttons ----
            div { class: "align-section",
                div { class: "align-section-header", "Distribute" }
                div { class: "align-buttons",
                    button {
                        class: "align-btn",
                        title: "Make horizontal gaps equal",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::HorizontalGaps)),
                        "↔ H Gaps"
                    }
                    button {
                        class: "align-btn",
                        title: "Make vertical gaps equal",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::VerticalGaps)),
                        "↕ V Gaps"
                    }
                    button {
                        class: "align-btn",
                        title: "Distribute left edges equidistantly",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::Left)),
                        "⫷ D-Left"
                    }
                    button {
                        class: "align-btn",
                        title: "Distribute centers horizontally",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::CenterH)),
                        "⫿ D-CtrH"
                    }
                    button {
                        class: "align-btn",
                        title: "Distribute top edges equidistantly",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::Top)),
                        "⊤ D-Top"
                    }
                    button {
                        class: "align-btn",
                        title: "Distribute centers vertically",
                        onclick: move |_| on_action.call(AlignAction::Distribute(DistributeOp::CenterV)),
                        "⊞ D-CtrV"
                    }
                }
            }
        }
    }
}
