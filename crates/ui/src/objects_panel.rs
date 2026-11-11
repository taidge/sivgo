//! Objects panel component showing the full object tree with visibility,
//! lock, type icons, selection, and rename support.

use crate::controls::{action_button, panel};
use crate::layers_panel::{tree_button, tree_row};
use glory::prelude::*;
use sivgo_dom::NodeId;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type DropTargets = Rc<RefCell<Vec<(Option<NodeId>, glory::render::NodeId)>>>;

fn object_drag_handle(
    scope: &Scope,
    id: NodeId,
    label: &str,
    row_node: glory::render::NodeId,
    targets: DropTargets,
    serial: Rc<Cell<u64>>,
    callback: crate::Callback<ObjectAction>,
) -> Result {
    use glory::render::{DragCapture, DragPhase, EventKind, Query, QueryValue, Semantics};
    let owner = scope.downgrade();
    let destinations = targets.clone();
    view()
        .name(format!("Move object {label}"))
        .class("object-drag-handle")
        .style(Style::new().padding(glory::style::Edges::all(6.0)))
        .semantics(Some(Box::new(Semantics {
            drag_capture: Some(DragCapture::Press),
            ..Default::default()
        })))
        .fill(crate::icons::EditorIcon::DragHandle.widget())
        .on(EventKind::Drag, move |event| {
            let Event::Drag { phase, x, y, .. } = event else {
                return;
            };
            if matches!(phase, DragPhase::Start | DragPhase::Cancel) {
                serial.set(serial.get().wrapping_add(1));
            }
            if phase != DragPhase::Drop {
                return;
            }
            let Some(scope) = owner.upgrade() else {
                return;
            };
            let stamp = serial.get();
            let targets = destinations.borrow().clone();
            let pending = Rc::new(RefCell::new((
                targets.len(),
                None::<(usize, Option<NodeId>)>,
            )));
            for (index, (destination, target)) in targets.into_iter().enumerate() {
                let callback = callback.clone();
                let pending = pending.clone();
                let serial = serial.clone();
                let result = scope.request(Query::Bounds(target), move |reply| {
                    if serial.get() != stamp {
                        return;
                    }
                    let mut pending = pending.borrow_mut();
                    if let Ok(QueryValue::Bounds(bounds)) = reply {
                        if bounds.width > 0.0
                            && bounds.height > 0.0
                            && x >= bounds.x
                            && y >= bounds.y
                            && x <= bounds.x + bounds.width
                            && y <= bounds.y + bounds.height
                            && pending
                                .1
                                .as_ref()
                                .is_none_or(|(previous, _)| index > *previous)
                        {
                            pending.1 = Some((index, destination));
                        }
                    }
                    pending.0 -= 1;
                    if pending.0 == 0 {
                        let destination = pending.1;
                        drop(pending);
                        if let Some((_, destination)) = destination {
                            if destination == Some(id) {
                                return;
                            }
                            // Publish the complete operation after the gesture,
                            // so row snapshots cannot retire the pointer owner.
                            callback.call(ObjectAction::BeginDrag(id));
                            callback.call(match destination {
                                Some(target) => ObjectAction::DropOn(target),
                                None => ObjectAction::DropToRoot,
                            });
                            callback.call(ObjectAction::EndDrag);
                        }
                    }
                });
                if let Err(error) = result {
                    scope.report_error(error);
                }
            }
        })
        .show_in(scope)?;
    targets.borrow_mut().push((Some(id), row_node));
    Ok(())
}

/// Represents a single object in the tree hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectEntry {
    pub id: NodeId,
    pub label: String,
    pub type_name: String,
    pub depth: usize,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
}

/// Editable details for the primary selected object.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDetails {
    pub id: NodeId,
    pub label: String,
    pub type_name: String,
    pub xml_id: String,
}

/// Events emitted by the objects panel.
#[derive(Debug, Clone)]
pub enum ObjectAction {
    /// Single-click: select the object.
    Select(NodeId),
    /// Double-click: begin renaming.
    BeginRename(NodeId),
    /// Commit a rename.
    Rename(NodeId, String),
    /// Commit a serialized SVG id change.
    SetXmlId(NodeId, String),
    /// Start dragging an object row.
    BeginDrag(NodeId),
    /// End object-row dragging.
    EndDrag,
    /// Drop the dragged object onto a target object row.
    DropOn(NodeId),
    /// Drop the dragged object at the document root level.
    DropToRoot,
    /// Toggle visibility of the object.
    ToggleVisibility(NodeId),
    /// Toggle lock state of the object.
    ToggleLock(NodeId),
}

/// Reuse the drawing-tool icon set for object types, with explicit resource icons.
fn type_icon(type_name: &str) -> impl Widget {
    use crate::icons::EditorIcon;
    use sivgo_core::ToolId;
    let kind = type_name.to_owned();
    widget(move |scope| {
        let tool = match kind.as_str() {
            "rect" => ToolId::Rect,
            "circle" | "ellipse" => ToolId::Ellipse,
            "line" | "polyline" | "path" => ToolId::Pen,
            "polygon" => ToolId::Star,
            "text" | "tspan" | "textPath" => ToolId::Text,
            _ => {
                return match kind.as_str() {
                    "image" => EditorIcon::ObjectImage,
                    "use" => EditorIcon::ObjectClone,
                    _ => EditorIcon::ObjectGroup,
                }
                .widget()
                .show_in(scope);
            }
        };
        image()
            .source(crate::toolbar::tool_icon_uri(tool))
            .alt(format!("{} object", kind))
            .style(EditorIcon::style())
            .show_in(scope)
    })
}

/// A panel showing the full object tree of the SVG document.
///
/// Each row displays: type icon, visibility toggle, lock toggle, and label.
/// Single-click selects; double-click begins renaming.
#[derive(Clone)]
pub struct ObjectsPanel {
    pub objects: Vec<ObjectEntry>,
    pub editing_id: Option<NodeId>,
    pub edit_value: String,
    pub selected_details: Option<ObjectDetails>,
    pub xml_id_edit_value: String,
    pub on_action: crate::Callback<ObjectAction>,
    pub on_edit_change: crate::Callback<String>,
    pub on_xml_id_change: crate::Callback<String>,
}

impl Widget for ObjectsPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "objects-panel", "Objects")?;
        let targets: DropTargets = Rc::new(RefCell::new(Vec::new()));
        let drag_serial = Rc::new(Cell::new(0));
        let root_target = scope.cage(None);
        view()
            .name("Root level drop target")
            .class("object-root-drop")
            .node_ref(root_target)
            .style(Style::new().padding(glory::style::Edges::all(8.0)))
            .fill(text().content("Drop here to move to document root"))
            .show_in(&scope)?;
        targets
            .borrow_mut()
            .push((None, root_target.get().ok_or(glory::ui::Error::Gone)?));
        if let Some(details) = &self.selected_details {
            text()
                .content(format!("{} — {}", details.type_name, details.label))
                .show_in(&scope)?;
            let id = details.id;
            let callback = self.on_action.clone();
            let changed = self.on_xml_id_change.clone();
            sapid::text_field()
                .theme(crate::controls::editor_theme())
                .size(sapid::Size::ExtraSmall)
                .name("SVG object ID")
                .default_value(self.xml_id_edit_value.clone())
                .on_value_change(move |value| changed.call(value))
                .on_commit(move |value| callback.call(ObjectAction::SetXmlId(id, value)))
                .show_in(&scope)?;
        }

        let mut ancestors: Vec<(usize, Cage<bool>)> = Vec::new();
        for (index, entry) in self.objects.iter().enumerate() {
            ancestors.retain(|(depth, _)| *depth < entry.depth);
            let parents: Vec<_> = ancestors.iter().map(|(_, open)| *open).collect();
            let visible = scope.bond(move || parents.iter().all(|open| open.get()));
            let (row, row_node) = tree_row(&scope, entry.depth, visible, entry.selected)?;
            if self
                .objects
                .get(index + 1)
                .is_some_and(|next| next.depth > entry.depth)
            {
                let open = scope.cage(true);
                tree_button("", format!("Expand or collapse {}", entry.label), false)
                    .left_section(crate::layers_panel::disclosure_icon(open))
                    .on_press(move || open.revise(|value| *value = !*value))
                    .show_in(&row)?;
                ancestors.push((entry.depth, open));
            }
            let id = entry.id;
            object_drag_handle(
                &row,
                id,
                &entry.label,
                row_node,
                targets.clone(),
                drag_serial.clone(),
                self.on_action.clone(),
            )?;
            let callback = self.on_action.clone();
            let mut name_style = Style::new().min_width(Length::Px(0.0));
            name_style.grow = Some(1.0);
            tree_button(
                entry.label.clone(),
                format!("Select object {}", entry.label),
                entry.selected,
            )
            .left_section(type_icon(&entry.type_name))
            .style(name_style)
            .on_press(move || callback.call(ObjectAction::Select(id)))
            .show_in(&row)?;
            let callback = self.on_action.clone();
            tree_button(
                "",
                format!(
                    "{} object {}",
                    if entry.visible { "Hide" } else { "Show" },
                    entry.label
                ),
                false,
            )
            .left_section(
                if entry.visible {
                    crate::icons::EditorIcon::Eye
                } else {
                    crate::icons::EditorIcon::EyeOff
                }
                .widget(),
            )
            .on_press(move || callback.call(ObjectAction::ToggleVisibility(id)))
            .show_in(&row)?;
            let callback = self.on_action.clone();
            tree_button(
                "",
                format!(
                    "{} object {}",
                    if entry.locked { "Unlock" } else { "Lock" },
                    entry.label
                ),
                entry.locked,
            )
            .left_section(
                if entry.locked {
                    crate::icons::EditorIcon::Lock
                } else {
                    crate::icons::EditorIcon::Unlock
                }
                .widget(),
            )
            .on_press(move || callback.call(ObjectAction::ToggleLock(id)))
            .show_in(&row)?;
        }
        let focused = self
            .objects
            .iter()
            .find(|entry| self.editing_id == Some(entry.id))
            .or_else(|| self.objects.iter().find(|entry| entry.selected));
        if let Some(entry) = focused {
            let id = entry.id;
            let detail = panel(&scope, "object-details", &entry.label)?;
            let callback = self.on_action.clone();
            action_button(
                &detail,
                "Rename object",
                self.editing_id == Some(id),
                move || callback.call(ObjectAction::BeginRename(id)),
            )?;
            if self.editing_id == Some(id) {
                let callback = self.on_action.clone();
                let changed = self.on_edit_change.clone();
                sapid::text_field()
                    .theme(crate::controls::editor_theme())
                    .size(sapid::Size::ExtraSmall)
                    .name("Object name")
                    .default_value(self.edit_value.clone())
                    .on_value_change(move |value| changed.call(value))
                    .on_commit(move |value| callback.call(ObjectAction::Rename(id, value)))
                    .show_in(&detail)?;
            }
            // Keyboard-accessible alternatives to the drag handle, shown only
            // after selecting the object to move or its destination.
            for (label, action) in [
                ("Move this object", ObjectAction::BeginDrag(id)),
                ("Move dragged object here", ObjectAction::DropOn(id)),
                ("Move dragged object to root", ObjectAction::DropToRoot),
                ("Cancel object move", ObjectAction::EndDrag),
            ] {
                let callback = self.on_action.clone();
                action_button(&detail, label, false, move || callback.call(action.clone()))?;
            }
        }
        if self.objects.is_empty() {
            text().content("No objects").show_in(&scope)?;
        }
        Ok(())
    }
}
