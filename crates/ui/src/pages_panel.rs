//! Pages panel for Inkscape multi-page documents.

use dioxus::prelude::*;
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
#[component]
pub fn PagesPanel(pages: Vec<PageEntry>, on_action: EventHandler<PageAction>) -> Element {
    let mut selected_template = use_signal(|| PageTemplate::CurrentDocument.id().to_string());
    let template_value = selected_template.read().clone();

    rsx! {
        div { class: "pages-panel",
            div { class: "panel-header",
                span { "Pages" }
                button {
                    class: "panel-action-btn",
                    title: "Add page",
                    onclick: move |_| on_action.call(PageAction::Add),
                    "Add"
                }
            }
            div { class: "page-template-row",
                select {
                    class: "prop-select",
                    value: "{template_value}",
                    onchange: move |evt: Event<FormData>| {
                        selected_template.set(evt.value().to_string());
                    },
                    for template in PageTemplate::all().iter().copied() {
                        option {
                            value: "{template.id()}",
                            selected: template.id() == template_value,
                            "{template.label()}"
                        }
                    }
                }
                button {
                    class: "panel-action-btn",
                    title: "Add page from template",
                    onclick: move |_| {
                        if let Some(template) = PageTemplate::from_id(&selected_template.read()) {
                            on_action.call(PageAction::AddTemplate(template));
                        }
                    },
                    "Add Template"
                }
            }
            if pages.is_empty() {
                div { class: "prop-empty", "No pages" }
            } else {
                div { class: "page-list",
                    for page in pages.iter() {
                        {
                            let id = page.id;
                            rsx! {
                                div {
                                    key: "{page.index}",
                                    class: if page.active { "page-row active" } else { "page-row" },
                                    div { class: "page-row-head",
                                        button {
                                            class: "page-nav-btn",
                                            title: "Go to page",
                                            onclick: move |_| on_action.call(PageAction::Navigate(id)),
                                            "#{page.index}"
                                        }
                                        input {
                                            r#type: "text",
                                            class: "page-label-input",
                                            value: "{page.label}",
                                            onchange: move |evt: Event<FormData>| {
                                                on_action.call(PageAction::SetLabel(id, evt.value().to_string()));
                                            },
                                        }
                                        button {
                                            class: "page-delete-btn",
                                            title: "Delete page",
                                            onclick: move |_| on_action.call(PageAction::Delete(id)),
                                            "Delete"
                                        }
                                    }
                                    div { class: "page-geometry-grid",
                                        label { class: "prop-label", "X" }
                                        input {
                                            r#type: "number",
                                            class: "prop-input",
                                            value: "{page.x}",
                                            step: "1",
                                            onchange: move |evt: Event<FormData>| {
                                                if let Ok(value) = evt.value().parse::<f64>() {
                                                    on_action.call(PageAction::SetX(id, value));
                                                }
                                            },
                                        }
                                        label { class: "prop-label", "Y" }
                                        input {
                                            r#type: "number",
                                            class: "prop-input",
                                            value: "{page.y}",
                                            step: "1",
                                            onchange: move |evt: Event<FormData>| {
                                                if let Ok(value) = evt.value().parse::<f64>() {
                                                    on_action.call(PageAction::SetY(id, value));
                                                }
                                            },
                                        }
                                        label { class: "prop-label", "W" }
                                        input {
                                            r#type: "number",
                                            class: "prop-input",
                                            value: "{page.width}",
                                            min: "1",
                                            step: "1",
                                            onchange: move |evt: Event<FormData>| {
                                                if let Ok(value) = evt.value().parse::<f64>() {
                                                    on_action.call(PageAction::SetWidth(id, value));
                                                }
                                            },
                                        }
                                        label { class: "prop-label", "H" }
                                        input {
                                            r#type: "number",
                                            class: "prop-input",
                                            value: "{page.height}",
                                            min: "1",
                                            step: "1",
                                            onchange: move |evt: Event<FormData>| {
                                                if let Ok(value) = evt.value().parse::<f64>() {
                                                    on_action.call(PageAction::SetHeight(id, value));
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
