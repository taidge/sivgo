//! Pages panel for Inkscape multi-page documents.

use crate::controls::{action_button, number_field, panel, text_field};
use glory::prelude::*;
use sivgo_dom::NodeId;

const PX_PER_MM: f64 = 96.0 / 25.4;

/// Built-in page templates for creating new pages from the Pages inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTemplate {
    CurrentDocument,
    A4Portrait,
    A4Landscape,
    A3Portrait,
    LetterPortrait,
    LetterLandscape,
    Square1024,
}

impl PageTemplate {
    pub fn all() -> &'static [PageTemplate] {
        &[
            Self::CurrentDocument,
            Self::A4Portrait,
            Self::A4Landscape,
            Self::A3Portrait,
            Self::LetterPortrait,
            Self::LetterLandscape,
            Self::Square1024,
        ]
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::CurrentDocument => "current",
            Self::A4Portrait => "a4_portrait",
            Self::A4Landscape => "a4_landscape",
            Self::A3Portrait => "a3_portrait",
            Self::LetterPortrait => "letter_portrait",
            Self::LetterLandscape => "letter_landscape",
            Self::Square1024 => "square_1024",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|template| template.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CurrentDocument => "Document",
            Self::A4Portrait => "A4 Portrait",
            Self::A4Landscape => "A4 Landscape",
            Self::A3Portrait => "A3 Portrait",
            Self::LetterPortrait => "Letter Portrait",
            Self::LetterLandscape => "Letter Landscape",
            Self::Square1024 => "1024 Square",
        }
    }

    pub fn dimensions_px(self, document_width: f64, document_height: f64) -> (f64, f64) {
        match self {
            Self::CurrentDocument => (document_width.max(1.0), document_height.max(1.0)),
            Self::A4Portrait => mm_to_px_pair(210.0, 297.0),
            Self::A4Landscape => mm_to_px_pair(297.0, 210.0),
            Self::A3Portrait => mm_to_px_pair(297.0, 420.0),
            Self::LetterPortrait => mm_to_px_pair(215.9, 279.4),
            Self::LetterLandscape => mm_to_px_pair(279.4, 215.9),
            Self::Square1024 => (1024.0, 1024.0),
        }
    }
}

fn mm_to_px_pair(width_mm: f64, height_mm: f64) -> (f64, f64) {
    (width_mm * PX_PER_MM, height_mm * PX_PER_MM)
}

/// Canvas overlay CSS class for a page.
pub fn page_overlay_class(active: bool, page_tool_active: bool) -> &'static str {
    match (active, page_tool_active) {
        (true, true) => "page-overlay active page-tool-overlay",
        (false, true) => "page-overlay page-tool-overlay",
        (true, false) => "page-overlay active",
        (false, false) => "page-overlay",
    }
}

/// A page shown in the Pages inspector.
#[derive(Debug, Clone, PartialEq)]
pub struct PageEntry {
    pub index: usize,
    pub id: NodeId,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub active: bool,
}

/// Events emitted by the Pages panel.
#[derive(Debug, Clone)]
pub enum PageAction {
    Add,
    AddTemplate(PageTemplate),
    Navigate(NodeId),
    Delete(NodeId),
    SetLabel(NodeId, String),
    SetX(NodeId, f64),
    SetY(NodeId, f64),
    SetWidth(NodeId, f64),
    SetHeight(NodeId, f64),
}

/// A compact inspector for creating, navigating, deleting, and editing pages.
#[derive(Clone)]
pub struct PagesPanel {
    pub pages: Vec<PageEntry>,
    pub on_action: crate::Callback<PageAction>,
}

impl Widget for PagesPanel {
    fn build(&mut self, scope: &Scope) -> Result {
        let scope = panel(scope, "pages-panel", "Pages")?;
        let template = scope.cage(PageTemplate::CurrentDocument.id().to_string());
        crate::controls::choice_options(
            &scope,
            "Page template",
            template.get(),
            PageTemplate::all()
                .iter()
                .map(|t| (t.id().to_owned(), t.label().to_owned()))
                .collect(),
            move |value| template.revise(|current| *current = value),
        )?;
        let callback = self.on_action.clone();
        action_button(&scope, "Add template page", false, move || {
            if let Some(t) = PageTemplate::from_id(&template.get()) {
                callback.call(PageAction::AddTemplate(t));
            }
        })?;
        let callback = self.on_action.clone();
        action_button(&scope, "Add page", false, move || {
            callback.call(PageAction::Add)
        })?;
        for page in &self.pages {
            let scope = panel(
                &scope,
                if page.active {
                    "page-card active"
                } else {
                    "page-card"
                },
                &format!("Page {}", page.index + 1),
            )?;
            let id = page.id;
            let callback = self.on_action.clone();
            action_button(&scope, "Go to page", page.active, move || {
                callback.call(PageAction::Navigate(id))
            })?;
            let callback = self.on_action.clone();
            action_button(&scope, "Delete page", false, move || {
                callback.call(PageAction::Delete(id))
            })?;
            let callback = self.on_action.clone();
            text_field(&scope, "Page label", page.label.clone(), move |value| {
                callback.call(PageAction::SetLabel(id, value))
            })?;
            let callback = self.on_action.clone();
            number_field(&scope, "X", page.x, move |value| {
                callback.call(PageAction::SetX(id, value));
            })?;
            let callback = self.on_action.clone();
            number_field(&scope, "Y", page.y, move |value| {
                callback.call(PageAction::SetY(id, value));
            })?;
            let callback = self.on_action.clone();
            number_field(&scope, "Width", page.width, move |value| {
                if value > 0.0 {
                    callback.call(PageAction::SetWidth(id, value));
                }
            })?;
            let callback = self.on_action.clone();
            number_field(&scope, "Height", page.height, move |value| {
                if value > 0.0 {
                    callback.call(PageAction::SetHeight(id, value));
                }
            })?;
        }
        if self.pages.is_empty() {
            text()
                .content("No pages. Add a page to get started.")
                .show_in(&scope)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_template_lookup_round_trips_ids() {
        for template in PageTemplate::all().iter().copied() {
            assert_eq!(PageTemplate::from_id(template.id()), Some(template));
        }
        assert_eq!(PageTemplate::from_id("missing"), None);
    }

    #[test]
    fn page_template_dimensions_use_expected_units() {
        assert_eq!(
            PageTemplate::CurrentDocument.dimensions_px(800.0, 600.0),
            (800.0, 600.0)
        );
        assert_eq!(
            PageTemplate::Square1024.dimensions_px(1.0, 1.0),
            (1024.0, 1024.0)
        );

        let (a4_w, a4_h) = PageTemplate::A4Portrait.dimensions_px(1.0, 1.0);
        assert!((a4_w - 793.7007874015749).abs() < 0.001);
        assert!((a4_h - 1122.5196850393702).abs() < 0.001);

        let (landscape_w, landscape_h) = PageTemplate::A4Landscape.dimensions_px(1.0, 1.0);
        assert_eq!(landscape_w, a4_h);
        assert_eq!(landscape_h, a4_w);
    }

    #[test]
    fn page_overlay_class_tracks_active_and_page_tool_states() {
        assert_eq!(page_overlay_class(false, false), "page-overlay");
        assert_eq!(page_overlay_class(true, false), "page-overlay active");
        assert_eq!(
            page_overlay_class(false, true),
            "page-overlay page-tool-overlay"
        );
        assert_eq!(
            page_overlay_class(true, true),
            "page-overlay active page-tool-overlay"
        );
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use glory::render::Props;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn template_add_uses_owner_scoped_choice_and_dispatches_action() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let captured = actions.clone();
        let app = App::memory();
        app.mount(PagesPanel {
            pages: vec![],
            on_action: crate::Callback::new(move |a| captured.borrow_mut().push(a)),
        })
        .unwrap();
        let tree = app.tree();
        let mut nodes = tree.roots().to_vec();
        let mut button = None;
        while let Some(id) = nodes.pop() {
            let node = tree.node(id).unwrap();
            if matches!(node.descriptor.view().props, Props::Button(props) if props.label == "Add template page")
            {
                button = Some(id);
                break;
            }
            nodes.extend(node.children.iter().copied());
        }
        drop(tree);
        app.dispatch(button.expect("template action"), Event::Activate)
            .unwrap();
        assert!(matches!(
            actions.borrow().as_slice(),
            [PageAction::AddTemplate(PageTemplate::CurrentDocument)]
        ));
        app.clear().unwrap();
        assert_eq!(app.stats().nodes, 0);
    }
}
