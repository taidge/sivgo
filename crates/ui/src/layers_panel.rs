//! Layers panel component with visibility, lock, opacity, and reordering.

use crate::controls::{action_button, choice_field, number_field, panel};
use glory::prelude::*;
use sivgo_dom::NodeId;

/// Represents a single entry in the layers panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerEntry {
    pub id: NodeId,
    pub label: String,
    pub depth: usize,
    pub selected: bool,
    pub active: bool,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    pub blend_mode: String,
}

/// CSS/SVG blend mode keywords selectable per layer.
const BLEND_MODES: &[&str] = &[
    "normal",
    "multiply",
    "screen",
    "overlay",
    "darken",
    "lighten",
    "color-dodge",
    "color-burn",
    "hard-light",
    "soft-light",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity",
];

/// Event fired when a layer property changes.
#[derive(Debug, Clone)]
pub enum LayerAction {
    Select(NodeId),
    BeginRename(NodeId),
    Rename(NodeId, String),
    ToggleVisibility(NodeId),
    ToggleLock(NodeId),
    SetOpacity(NodeId, f64),
    SetBlendMode(NodeId, String),
    MoveUp(NodeId),
    MoveDown(NodeId),
}

/// A panel displaying the layer/object stack in the document.
#[derive(Clone)]
pub struct LayersPanel {
    pub layers: Vec<LayerEntry>,
    pub editing_id: Option<NodeId>,
    pub edit_value: String,
    pub on_action: crate::Callback<LayerAction>,
    pub on_edit_change: crate::Callback<String>,
}

/// A tree row uses real indentation and a single horizontal control strip.
pub(crate) fn tree_row(
    scope: &Scope,
    depth: usize,
    visible: Bond<bool>,
    selected: bool,
) -> Result<(Scope, glory::render::NodeId)> {
    let node = scope.cage(None);
    view()
        .node_ref(node)
        .style(scope.bond(move || {
            let visible = visible.get();
            let mut style = Style::new()
                .display(if visible {
                    Display::Flex
                } else {
                    Display::None
                })
                .flex_direction(FlexDirection::Row)
                .gap(2.0)
                .min_width(Length::Px(0.0));
            style.padding = Some(glory::style::Edges {
                left: (depth.min(12) * 12) as f64,
                ..glory::style::Edges::all(0.0)
            });
            style.align = Some(glory::style::Align::Center);
            style.shrink = Some(0.0);
            if selected {
                style.background = Some(crate::controls::editor_theme().surface);
            }
            style
        }))
        .show_in(scope)?;
    let child = scope.child_in(node.get().ok_or(glory::ui::Error::Gone)?)?;
    scope.retain(child.clone())?;
    Ok((child, node.get().ok_or(glory::ui::Error::Gone)?))
}

pub(crate) fn tree_button(
    label: impl Into<String>,
    name: impl Into<String>,
    selected: bool,
) -> sapid::Button {
    sapid::button()
        .theme(crate::controls::editor_theme())
        .size(sapid::Size::ExtraSmall)
        .label(label.into())
        .name(name.into())
        .variant(if selected {
            sapid::ButtonVariant::Primary
        } else {
            sapid::ButtonVariant::Subtle
        })
}

impl Widget for LayersPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "layers-panel", "Layers")?;
        let mut ancestors: Vec<(usize, Cage<bool>)> = Vec::new();
        for (index, entry) in self.layers.iter().enumerate() {
            ancestors.retain(|(depth, _)| *depth < entry.depth);
            let parents: Vec<_> = ancestors.iter().map(|(_, open)| *open).collect();
            let visible = scope.bond(move || parents.iter().all(|open| open.get()));
            let (row, _) = tree_row(&scope, entry.depth, visible, entry.selected)?;
            if self
                .layers
                .get(index + 1)
                .is_some_and(|next| next.depth > entry.depth)
            {
                let open = scope.cage(true);
                tree_button("", format!("Expand or collapse {}", entry.label), false)
                    .label(open.map(|open| {
                        if *open {
                            "▾".to_owned()
                        } else {
                            "▸".to_owned()
                        }
                    }))
                    .on_press(move || open.revise(|value| *value = !*value))
                    .show_in(&row)?;
                ancestors.push((entry.depth, open));
            }
            let id = entry.id;
            let callback = self.on_action.clone();
            let mut name_style = Style::new().min_width(Length::Px(0.0));
            name_style.grow = Some(1.0);
            tree_button(
                format!("{}{}", entry.label, if entry.active { " •" } else { "" }),
                format!("Select layer {}", entry.label),
                entry.selected,
            )
            .style(name_style)
            .on_press(move || callback.call(LayerAction::Select(id)))
            .show_in(&row)?;
            let callback = self.on_action.clone();
            tree_button(
                if entry.visible { "●" } else { "○" },
                format!(
                    "{} layer {}",
                    if entry.visible { "Hide" } else { "Show" },
                    entry.label
                ),
                false,
            )
            .on_press(move || callback.call(LayerAction::ToggleVisibility(id)))
            .show_in(&row)?;
            let callback = self.on_action.clone();
            tree_button(
                if entry.locked { "🔒" } else { "◇" },
                format!(
                    "{} layer {}",
                    if entry.locked { "Unlock" } else { "Lock" },
                    entry.label
                ),
                entry.locked,
            )
            .on_press(move || callback.call(LayerAction::ToggleLock(id)))
            .show_in(&row)?;
        }
        // Keep one property editor even for a multi-selection.
        let focused = self
            .layers
            .iter()
            .find(|entry| self.editing_id == Some(entry.id))
            .or_else(|| self.layers.iter().find(|entry| entry.selected))
            .or_else(|| self.layers.iter().find(|entry| entry.active));
        if let Some(entry) = focused {
            let id = entry.id;
            let detail = panel(&scope, "layer-details", &entry.label)?;
            let callback = self.on_action.clone();
            action_button(
                &detail,
                "Rename layer",
                self.editing_id == Some(id),
                move || callback.call(LayerAction::BeginRename(id)),
            )?;
            if self.editing_id == Some(id) {
                let callback = self.on_action.clone();
                let changed = self.on_edit_change.clone();
                sapid::text_field()
                    .theme(crate::controls::editor_theme())
                    .size(sapid::Size::ExtraSmall)
                    .name("Layer name")
                    .default_value(self.edit_value.clone())
                    .on_value_change(move |value| changed.call(value))
                    .on_commit(move |value| callback.call(LayerAction::Rename(id, value)))
                    .show_in(&detail)?;
            }
            let index = self
                .layers
                .iter()
                .position(|layer| layer.id == id)
                .unwrap_or(0);
            let previous = self.layers[..index]
                .iter()
                .rev()
                .find(|layer| layer.depth <= entry.depth);
            let next = self.layers[index + 1..]
                .iter()
                .find(|layer| layer.depth <= entry.depth);
            let callback = self.on_action.clone();
            tree_button("Move layer up", "Move layer up", false)
                .enabled(previous.is_some_and(|layer| layer.depth == entry.depth))
                .on_press(move || callback.call(LayerAction::MoveUp(id)))
                .show_in(&detail)?;
            let callback = self.on_action.clone();
            tree_button("Move layer down", "Move layer down", false)
                .enabled(next.is_some_and(|layer| layer.depth == entry.depth))
                .on_press(move || callback.call(LayerAction::MoveDown(id)))
                .show_in(&detail)?;
            let callback = self.on_action.clone();
            number_field(&detail, "Opacity (0–1)", entry.opacity, move |value| {
                callback.call(LayerAction::SetOpacity(id, value.clamp(0.0, 1.0)))
            })?;
            let callback = self.on_action.clone();
            choice_field(
                &detail,
                "Blend mode",
                entry.blend_mode.clone(),
                BLEND_MODES,
                move |value| callback.call(LayerAction::SetBlendMode(id, value)),
            )?;
        } else {
            text()
                .content(if self.layers.is_empty() {
                    "No layers"
                } else {
                    "Select a layer to edit its properties"
                })
                .show_in(&scope)?;
        }
        Ok(())
    }
}
