//! Guides dialog for managing document guides: listing, adding, editing,
//! and deleting guides with position, angle, and label.

use dioxus::prelude::*;

/// A single guide entry.
#[derive(Debug, Clone, PartialEq)]
pub struct GuideEntry {
    /// Unique identifier.
    pub id: String,
    /// Position along the relevant axis (in document units).
    pub position: f64,
    /// Angle in degrees (0 = horizontal, 90 = vertical).
    pub angle: f64,
    /// Optional label for the guide.
    pub label: String,
}

impl GuideEntry {
    /// Create a new horizontal guide at the given position.
    pub fn horizontal(id: impl Into<String>, position: f64) -> Self {
        Self {
            id: id.into(),
            position,
            angle: 0.0,
            label: String::new(),
        }
    }

    /// Create a new vertical guide at the given position.
    pub fn vertical(id: impl Into<String>, position: f64) -> Self {
        Self {
            id: id.into(),
            position,
            angle: 90.0,
            label: String::new(),
        }
    }

    /// Returns true if this guide is vertical (angle == 90).
    pub fn is_vertical(&self) -> bool {
        (self.angle - 90.0).abs() < f64::EPSILON
    }

    /// Returns true if this guide is horizontal (angle == 0).
    pub fn is_horizontal(&self) -> bool {
        self.angle.abs() < f64::EPSILON
    }
}

/// Events emitted by the Guides dialog.
#[derive(Debug, Clone)]
pub enum GuidesAction {
    /// Add a new guide with position and angle.
    Add(f64, f64),
    /// Select a guide for editing.
    Select(String),
    /// Update the position of a guide.
    SetPosition(String, f64),
    /// Update the angle of a guide.
    SetAngle(String, f64),
    /// Update the label of a guide.
    SetLabel(String, String),
    /// Delete a guide by id.
    Delete(String),
    /// Delete all guides.
    DeleteAll,
}

/// A dialog for managing document guides with position, angle, and label.
#[component]
pub fn GuidesDialog(
    guides: Vec<GuideEntry>,
    selected_id: Option<String>,
    on_action: EventHandler<GuidesAction>,
) -> Element {
    rsx! {
        div { class: "guides-dialog",
            div { class: "panel-header", "Guides" }

            // ---- Add guide buttons ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Add Guide" }
                div { class: "prop-row",
                    button {
                        class: "action-btn",
                        onclick: move |_| on_action.call(GuidesAction::Add(0.0, 0.0)),
                        "Horizontal"
                    }
                    button {
                        class: "action-btn",
                        onclick: move |_| on_action.call(GuidesAction::Add(0.0, 90.0)),
                        "Vertical"
                    }
                }
            }

            // ---- Guides list ----
            div { class: "prop-section",
                div { class: "prop-section-header", "Guides ({guides.len()})" }
                div { class: "guides-list",
                    for guide in guides.iter() {
                        {
                            let gid_select = guide.id.clone();
                            let gid_del = guide.id.clone();
                            let is_selected = selected_id.as_deref() == Some(guide.id.as_str());
                            let orient = if guide.is_horizontal() {
                                "H"
                            } else if guide.is_vertical() {
                                "V"
                            } else {
                                "A"
                            };
                            let guide_label = if guide.label.is_empty() {
                                format!("{} @ {:.1}", orient, guide.position)
                            } else {
                                format!("{} - {}", orient, guide.label)
                            };
                            rsx! {
                                div {
                                    class: if is_selected { "guide-item selected" } else { "guide-item" },
                                    onclick: move |_| on_action.call(GuidesAction::Select(gid_select.clone())),

                                    span { class: "guide-label", "{guide_label}" }

                                    button {
                                        class: "guide-btn delete",
                                        title: "Delete guide",
                                        onclick: move |_| on_action.call(GuidesAction::Delete(gid_del.clone())),
                                        "X"
                                    }
                                }
                            }
                        }
                    }
                    if guides.is_empty() {
                        div { class: "guides-empty", "No guides" }
                    }
                }
            }

            // ---- Selected guide editor ----
            if let Some(ref sel_id) = selected_id {
                {
                    if let Some(selected) = guides.iter().find(|g| &g.id == sel_id) {
                        let gid_pos = sel_id.clone();
                        let gid_angle = sel_id.clone();
                        let gid_label = sel_id.clone();
                        let label_val = selected.label.clone();
                        rsx! {
                            div { class: "prop-section",
                                div { class: "prop-section-header", "Edit Guide" }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Position" }
                                    input {
                                        r#type: "number",
                                        class: "prop-input",
                                        value: "{selected.position}",
                                        step: "1",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<f64>() {
                                                on_action.call(GuidesAction::SetPosition(gid_pos.clone(), val));
                                            }
                                        },
                                    }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Angle" }
                                    input {
                                        r#type: "number",
                                        class: "prop-input",
                                        value: "{selected.angle}",
                                        step: "1",
                                        min: "0",
                                        max: "360",
                                        onchange: move |evt: Event<FormData>| {
                                            if let Ok(val) = evt.value().parse::<f64>() {
                                                on_action.call(GuidesAction::SetAngle(gid_angle.clone(), val));
                                            }
                                        },
                                    }
                                    span { class: "prop-unit", "deg" }
                                }
                                div { class: "prop-row",
                                    label { class: "prop-label", "Label" }
                                    input {
                                        r#type: "text",
                                        class: "prop-input",
                                        placeholder: "Guide label",
                                        value: "{label_val}",
                                        onchange: move |evt: Event<FormData>| {
                                            on_action.call(GuidesAction::SetLabel(gid_label.clone(), evt.value().to_string()));
                                        },
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! { }
                    }
                }
            }

            // ---- Bulk actions ----
            div { class: "dialog-actions",
                button {
                    class: "action-btn",
                    disabled: guides.is_empty(),
                    onclick: move |_| on_action.call(GuidesAction::DeleteAll),
                    "Delete All"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_entry_horizontal() {
        let guide = GuideEntry::horizontal("g1", 100.0);
        assert!(guide.is_horizontal());
        assert!(!guide.is_vertical());
        assert!((guide.position - 100.0).abs() < f64::EPSILON);
        assert!(guide.label.is_empty());
    }

    #[test]
    fn guide_entry_vertical() {
        let guide = GuideEntry::vertical("g2", 50.0);
        assert!(guide.is_vertical());
        assert!(!guide.is_horizontal());
        assert!((guide.position - 50.0).abs() < f64::EPSILON);
    }
}
