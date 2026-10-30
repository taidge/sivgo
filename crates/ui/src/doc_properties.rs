//! Document Properties panel with page size, background, orientation,
//! metadata, and grid settings.

use crate::controls::{choice_field, color_field, editor_theme, number_field, panel, text_field};
use glory::prelude::*;

/// Page size preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSizePreset {
    A4,
    A3,
    Letter,
    Legal,
    Custom,
}

impl PageSizePreset {
    /// Returns (width, height) in millimetres for standard presets.
    /// Returns `None` for `Custom`.
    pub fn dimensions_mm(&self) -> Option<(f64, f64)> {
        match self {
            Self::A4 => Some((210.0, 297.0)),
            Self::A3 => Some((297.0, 420.0)),
            Self::Letter => Some((215.9, 279.4)),
            Self::Legal => Some((215.9, 355.6)),
            Self::Custom => None,
        }
    }

    /// All available presets in display order.
    pub fn all() -> &'static [PageSizePreset] {
        &[Self::A4, Self::A3, Self::Letter, Self::Legal, Self::Custom]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::A3 => "A3",
            Self::Letter => "Letter",
            Self::Legal => "Legal",
            Self::Custom => "Custom",
        }
    }
}

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

/// Grid settings for the document.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSettings {
    pub enabled: bool,
    pub spacing_x: f64,
    pub spacing_y: f64,
    pub snap_to_grid: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing_x: 10.0,
            spacing_y: 10.0,
            snap_to_grid: false,
        }
    }
}

/// Document metadata fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentMetadata {
    pub title: String,
    pub author: String,
    pub description: String,
}

/// Events emitted by the Document Properties panel.
#[derive(Debug, Clone)]
pub enum DocPropertiesAction {
    SetPreset(String),
    SetWidth(f64),
    SetHeight(f64),
    SetBackgroundColor(String),
    SetOrientation(PageOrientation),
    SetTitle(String),
    SetAuthor(String),
    SetDescription(String),
    SetGridEnabled(bool),
    SetGridSpacingX(f64),
    SetGridSpacingY(f64),
    SetSnapToGrid(bool),
}

/// A panel for editing document-level properties such as page size,
/// background color, orientation, metadata, and grid settings.
#[derive(Clone)]
pub struct DocPropertiesPanel {
    pub preset: String,
    pub width: f64,
    pub height: f64,
    pub background_color: String,
    pub orientation: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub grid_enabled: bool,
    pub grid_spacing_x: f64,
    pub grid_spacing_y: f64,
    pub snap_to_grid: bool,
    pub on_action: crate::Callback<DocPropertiesAction>,
}

impl Widget for DocPropertiesPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let root = panel(scope, "doc-properties-panel", "Document Properties")?;
        let page = panel(&root, "document-page-section", "Page")?;
        let callback = self.on_action.clone();
        choice_field(
            &page,
            "Page preset",
            self.preset.clone(),
            &["A4", "A3", "Letter", "Legal", "Custom"],
            move |value| callback.call(DocPropertiesAction::SetPreset(value)),
        )?;
        for (label, value, action) in [
            (
                "Width (mm)",
                self.width,
                DocPropertiesAction::SetWidth as fn(f64) -> DocPropertiesAction,
            ),
            ("Height (mm)", self.height, DocPropertiesAction::SetHeight),
        ] {
            let callback = self.on_action.clone();
            number_field(&page, label, value, move |value| {
                if value > 0.0 {
                    callback.call(action(value));
                }
            })?;
        }
        let callback = self.on_action.clone();
        choice_field(
            &page,
            "Orientation",
            self.orientation.clone(),
            &["Portrait", "Landscape"],
            move |value| {
                callback.call(DocPropertiesAction::SetOrientation(
                    if value == "Landscape" {
                        PageOrientation::Landscape
                    } else {
                        PageOrientation::Portrait
                    },
                ));
            },
        )?;
        let callback = self.on_action.clone();
        color_field(
            &page,
            "Background color",
            self.background_color.clone(),
            move |value| callback.call(DocPropertiesAction::SetBackgroundColor(value)),
        )?;
        let grid = panel(&root, "document-grid-section", "Grid")?;
        let callback = self.on_action.clone();
        sapid::checkbox()
            .theme(editor_theme())
            .size(sapid::Size::ExtraSmall)
            .label("Show grid")
            .default_checked(self.grid_enabled)
            .on_checked_change(move |value| {
                callback.call(DocPropertiesAction::SetGridEnabled(value))
            })
            .show_in(&grid)?;
        for (label, value, action) in [
            (
                "Grid spacing X",
                self.grid_spacing_x,
                DocPropertiesAction::SetGridSpacingX as fn(f64) -> DocPropertiesAction,
            ),
            (
                "Grid spacing Y",
                self.grid_spacing_y,
                DocPropertiesAction::SetGridSpacingY,
            ),
        ] {
            let callback = self.on_action.clone();
            number_field(&grid, label, value, move |value| {
                if value > 0.0 {
                    callback.call(action(value));
                }
            })?;
        }
        let callback = self.on_action.clone();
        sapid::checkbox()
            .theme(editor_theme())
            .size(sapid::Size::ExtraSmall)
            .label("Snap to grid")
            .default_checked(self.snap_to_grid)
            .on_checked_change(move |value| {
                callback.call(DocPropertiesAction::SetSnapToGrid(value))
            })
            .show_in(&grid)?;
        let metadata = panel(&root, "document-metadata-section", "Metadata")?;
        let callback = self.on_action.clone();
        text_field(&metadata, "Title", self.title.clone(), move |value| {
            callback.call(DocPropertiesAction::SetTitle(value))
        })?;
        let callback = self.on_action.clone();
        text_field(&metadata, "Author", self.author.clone(), move |value| {
            callback.call(DocPropertiesAction::SetAuthor(value))
        })?;
        text().content("Description").show_in(&metadata)?;
        let callback = self.on_action.clone();
        sapid::textarea()
            .theme(editor_theme())
            .size(sapid::Size::ExtraSmall)
            .name("Description")
            .default_value(self.description.clone())
            .style(
                Style::new()
                    .width(Length::Px(242.0))
                    .min_height(Length::Px(72.0)),
            )
            .on_commit(move |value| callback.call(DocPropertiesAction::SetDescription(value)))
            .show_in(&metadata)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_preset_dimensions() {
        let a4 = PageSizePreset::A4;
        let dims = a4.dimensions_mm().unwrap();
        assert!((dims.0 - 210.0).abs() < f64::EPSILON);
        assert!((dims.1 - 297.0).abs() < f64::EPSILON);

        assert!(PageSizePreset::Custom.dimensions_mm().is_none());
    }

    #[test]
    fn grid_settings_default() {
        let grid = GridSettings::default();
        assert!(!grid.enabled);
        assert!((grid.spacing_x - 10.0).abs() < f64::EPSILON);
        assert!((grid.spacing_y - 10.0).abs() < f64::EPSILON);
        assert!(!grid.snap_to_grid);
    }

    #[test]
    fn document_metadata_default() {
        let meta = DocumentMetadata::default();
        assert!(meta.title.is_empty());
        assert!(meta.author.is_empty());
        assert!(meta.description.is_empty());
    }
}
