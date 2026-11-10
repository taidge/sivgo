//! Preferences dialog with categorised settings sections:
//! Tools, Interface, I/O, Rendering, and Snapping.

use crate::controls::{action_button, choice_field, number_field, panel};
use glory::prelude::*;
use serde::{Deserialize, Serialize};

/// Top-level section within the preferences dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PreferencesSection {
    #[default]
    Tools,
    Interface,
    IO,
    Rendering,
    Snapping,
}

impl PreferencesSection {
    /// All available sections in display order.
    pub fn all() -> &'static [PreferencesSection] {
        &[
            Self::Tools,
            Self::Interface,
            Self::IO,
            Self::Rendering,
            Self::Snapping,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Interface => "Interface",
            Self::IO => "I/O",
            Self::Rendering => "Rendering",
            Self::Snapping => "Snapping",
        }
    }
}

/// Aggregate preferences state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreferencesState {
    pub section: PreferencesSection,
    // Tools
    pub default_shape_tool: String,
    pub bounding_box_display: bool,
    // Interface
    pub theme: String,
    pub icon_size: String,
    pub show_tooltips: bool,
    // I/O
    pub auto_save: bool,
    pub auto_save_interval_secs: u32,
    pub recent_files_count: u32,
    // Rendering
    pub anti_aliasing: bool,
    pub display_filter_quality: String,
    pub canvas_border: bool,
    // Snapping
    pub snap_to_nodes: bool,
    pub snap_to_paths: bool,
    pub snap_to_grid: bool,
    pub snap_to_guides: bool,
    pub snap_to_objects: bool,
    pub snap_to_intersections: bool,
    pub snap_to_equal_spacing: bool,
    pub snap_from_cursor: bool,
    pub snap_from_object_nodes: bool,
    pub snap_from_bbox_corners: bool,
    pub snap_from_bbox_centers: bool,
    pub snap_from_bbox_midpoints: bool,
    pub snap_distance: f64,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            section: PreferencesSection::Tools,
            default_shape_tool: "Rectangle".to_string(),
            bounding_box_display: true,
            theme: "Dark".to_string(),
            icon_size: "Medium".to_string(),
            show_tooltips: true,
            auto_save: true,
            auto_save_interval_secs: 300,
            recent_files_count: 10,
            anti_aliasing: true,
            display_filter_quality: "High".to_string(),
            canvas_border: true,
            snap_to_nodes: true,
            snap_to_paths: true,
            snap_to_grid: true,
            snap_to_guides: true,
            snap_to_objects: true,
            snap_to_intersections: true,
            snap_to_equal_spacing: true,
            snap_from_cursor: true,
            snap_from_object_nodes: true,
            snap_from_bbox_corners: true,
            snap_from_bbox_centers: true,
            snap_from_bbox_midpoints: true,
            snap_distance: 10.0,
        }
    }
}

/// Events emitted by the Preferences dialog.
#[derive(Debug, Clone)]
pub enum PreferencesAction {
    SetSection(PreferencesSection),
    // Tools
    SetDefaultShapeTool(String),
    SetBoundingBoxDisplay(bool),
    // Interface
    SetTheme(String),
    SetIconSize(String),
    SetShowTooltips(bool),
    // I/O
    SetAutoSave(bool),
    SetAutoSaveInterval(u32),
    SetRecentFilesCount(u32),
    // Rendering
    SetAntiAliasing(bool),
    SetDisplayFilterQuality(String),
    SetCanvasBorder(bool),
    // Snapping
    SetSnapToNodes(bool),
    SetSnapToPaths(bool),
    SetSnapToGrid(bool),
    SetSnapToGuides(bool),
    SetSnapToObjects(bool),
    SetSnapToIntersections(bool),
    SetSnapToEqualSpacing(bool),
    SetSnapFromCursor(bool),
    SetSnapFromObjectNodes(bool),
    SetSnapFromBBoxCorners(bool),
    SetSnapFromBBoxCenters(bool),
    SetSnapFromBBoxMidpoints(bool),
    SetSnapDistance(f64),
    /// Close the dialog.
    Close,
}

/// A dialog for editing application-wide preferences organised into
/// sections: Tools, Interface, I/O, Rendering, and Snapping.
#[derive(Clone)]
pub struct PreferencesDialog {
    pub state: PreferencesState,
    pub on_action: crate::Callback<PreferencesAction>,
}

impl Widget for PreferencesDialog {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "preferences-dialog", "")?;
        let callback = self.on_action.clone();
        row()
            .style(Style {
                align: Some(glory::style::Align::Center),
                ..Style::new()
            })
            .fill((
                text()
                    .class("panel-header")
                    .content("Preferences")
                    .style(Style {
                        grow: Some(1.0),
                        ..Style::new()
                    }),
                sapid::close_button()
                    .theme(crate::controls::editor_theme())
                    .size(sapid::Size::ExtraSmall)
                    .name("Close preferences")
                    .on_press(move || callback.call(PreferencesAction::Close)),
            ))
            .show_in(&scope)?;
        for section in PreferencesSection::all() {
            let next = section.clone();
            let callback = self.on_action.clone();
            action_button(
                &scope,
                section.label(),
                self.state.section == *section,
                move || callback.call(PreferencesAction::SetSection(next.clone())),
            )?;
        }
        match self.state.section {
            PreferencesSection::Tools => {
                let callback = self.on_action.clone();
                choice_field(
                    &scope,
                    "Default shape tool",
                    self.state.default_shape_tool.clone(),
                    &["Rectangle", "Ellipse", "Star", "Spiral"],
                    move |value| callback.call(PreferencesAction::SetDefaultShapeTool(value)),
                )?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Show bounding box")
                    .default_checked(self.state.bounding_box_display)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetBoundingBoxDisplay(value))
                    })
                    .show_in(&scope)?;
            }
            PreferencesSection::Interface => {
                let callback = self.on_action.clone();
                choice_field(
                    &scope,
                    "Theme",
                    self.state.theme.clone(),
                    &["Dark", "Light", "System"],
                    move |value| callback.call(PreferencesAction::SetTheme(value)),
                )?;
                let callback = self.on_action.clone();
                choice_field(
                    &scope,
                    "Icon size",
                    self.state.icon_size.clone(),
                    &["Small", "Medium", "Large"],
                    move |value| callback.call(PreferencesAction::SetIconSize(value)),
                )?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Show tooltips")
                    .default_checked(self.state.show_tooltips)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetShowTooltips(value))
                    })
                    .show_in(&scope)?;
            }
            PreferencesSection::IO => {
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Automatically save recovery drafts")
                    .default_checked(self.state.auto_save)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetAutoSave(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                number_field(
                    &scope,
                    "Recovery interval (seconds)",
                    self.state.auto_save_interval_secs as f64,
                    move |value| {
                        callback.call(PreferencesAction::SetAutoSaveInterval(
                            value.clamp(10.0, 86400.0) as u32,
                        ));
                    },
                )?;
                let callback = self.on_action.clone();
                number_field(
                    &scope,
                    "Recent files",
                    self.state.recent_files_count as f64,
                    move |value| {
                        callback.call(PreferencesAction::SetRecentFilesCount(
                            value.clamp(0.0, 50.0) as u32,
                        ));
                    },
                )?;
            }
            PreferencesSection::Rendering => {
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Anti-aliasing")
                    .default_checked(self.state.anti_aliasing)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetAntiAliasing(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Canvas border")
                    .default_checked(self.state.canvas_border)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetCanvasBorder(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                choice_field(
                    &scope,
                    "Filter quality",
                    self.state.display_filter_quality.clone(),
                    &["Low", "Medium", "High"],
                    move |value| callback.call(PreferencesAction::SetDisplayFilterQuality(value)),
                )?;
            }
            PreferencesSection::Snapping => {
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to nodes")
                    .default_checked(self.state.snap_to_nodes)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToNodes(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to paths")
                    .default_checked(self.state.snap_to_paths)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToPaths(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to grid")
                    .default_checked(self.state.snap_to_grid)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToGrid(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to guides")
                    .default_checked(self.state.snap_to_guides)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToGuides(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to objects")
                    .default_checked(self.state.snap_to_objects)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToObjects(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap to intersections")
                    .default_checked(self.state.snap_to_intersections)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToIntersections(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Equal spacing")
                    .default_checked(self.state.snap_to_equal_spacing)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapToEqualSpacing(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap from cursor")
                    .default_checked(self.state.snap_from_cursor)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapFromCursor(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap from object nodes")
                    .default_checked(self.state.snap_from_object_nodes)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapFromObjectNodes(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap from box corners")
                    .default_checked(self.state.snap_from_bbox_corners)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapFromBBoxCorners(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap from box centers")
                    .default_checked(self.state.snap_from_bbox_centers)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapFromBBoxCenters(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                sapid::checkbox()
                    .theme(crate::controls::editor_theme())
                    .label("Snap from box midpoints")
                    .default_checked(self.state.snap_from_bbox_midpoints)
                    .on_checked_change(move |value| {
                        callback.call(PreferencesAction::SetSnapFromBBoxMidpoints(value))
                    })
                    .show_in(&scope)?;
                let callback = self.on_action.clone();
                number_field(
                    &scope,
                    "Snap distance",
                    self.state.snap_distance,
                    move |value| {
                        callback.call(PreferencesAction::SetSnapDistance(value.clamp(1.0, 512.0)));
                    },
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_state_default() {
        let state = PreferencesState::default();
        assert_eq!(state.section, PreferencesSection::Tools);
        assert!(state.auto_save);
        assert_eq!(state.auto_save_interval_secs, 300);
        assert!(state.anti_aliasing);
        assert!(state.snap_to_nodes);
        assert!(state.snap_to_guides);
        assert!(state.snap_to_objects);
        assert!(state.snap_to_intersections);
        assert!(state.snap_to_equal_spacing);
        assert!(state.snap_from_cursor);
        assert!(state.snap_from_object_nodes);
        assert!(state.snap_from_bbox_corners);
        assert!(state.snap_from_bbox_centers);
        assert!(state.snap_from_bbox_midpoints);
        assert!((state.snap_distance - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn section_labels() {
        assert_eq!(PreferencesSection::Tools.label(), "Tools");
        assert_eq!(PreferencesSection::IO.label(), "I/O");
        assert_eq!(PreferencesSection::Snapping.label(), "Snapping");
        assert_eq!(PreferencesSection::all().len(), 5);
    }
}
