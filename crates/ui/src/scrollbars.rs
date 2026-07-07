//! Scrollbar components (horizontal and vertical) that reflect the viewport
//! position relative to the document extents.

use dioxus::prelude::*;

/// Minimum thumb size in pixels to keep it visible and clickable.
const MIN_THUMB_SIZE: f64 = 20.0;

/// Scrollbar thickness in pixels.
const SCROLLBAR_SIZE: f64 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScrollbarMetrics {
    thumb_size: f64,
    thumb_pos: f64,
    thumb_size_pct: f64,
    thumb_pos_pct: f64,
}

fn scrollbar_metrics(
    track_length: f64,
    document_length: f64,
    pan: f64,
    zoom: f64,
) -> ScrollbarMetrics {
    let track_length = track_length.max(1.0);
    let zoom = zoom.max(0.000_001);
    let visible_length = track_length / zoom;
    let total = document_length.max(visible_length);
    let thumb_ratio = (visible_length / total).min(1.0);
    let thumb_size = (thumb_ratio * track_length)
        .max(MIN_THUMB_SIZE)
        .min(track_length);
    let available_track = (track_length - thumb_size).max(0.0);
    let doc_offset = -pan / zoom;
    let scroll_ratio = if total > visible_length {
        (doc_offset / (total - visible_length)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_pos = scroll_ratio * available_track;
    ScrollbarMetrics {
        thumb_size,
        thumb_pos,
        thumb_size_pct: thumb_size / track_length * 100.0,
        thumb_pos_pct: thumb_pos / track_length * 100.0,
    }
}

/// Compute the visible length (in document coords) and the maximum document
/// offset for a scrollbar axis. Shared by both scrollbar components to derive
/// the click-to-scroll target.
fn scroll_extents(track_length: f64, document_length: f64, zoom: f64) -> (f64, f64, f64) {
    let visible_length = track_length.max(1.0) / zoom.max(0.000_001);
    let total = document_length.max(visible_length);
    let max_doc_offset = (total - visible_length).max(0.0);
    (visible_length, total, max_doc_offset)
}

fn horizontal_track_style(bar_h: f64) -> String {
    format!(
        "position: relative; width: 100%; height: {bar_h}px; background: #1e1e1e; border-top: 1px solid #444;"
    )
}

fn vertical_track_style(bar_w: f64) -> String {
    format!(
        "position: relative; width: {bar_w}px; height: 100%; background: #1e1e1e; border-left: 1px solid #444;"
    )
}

/// A horizontal scrollbar linked to the viewport pan.
///
/// Shows a thumb whose position and size are proportional to the viewport
/// width relative to the full document width.
#[component]
pub fn HorizontalScrollbar(
    /// Track length in pixels (the available scrollbar width).
    track_length: f64,
    /// The total document width in document coordinates.
    document_width: f64,
    /// Current viewport pan X offset (screen pixels).
    pan_x: f64,
    /// Current zoom factor.
    zoom: f64,
    /// Called when the user clicks on the scrollbar track to scroll.
    /// The value is the desired new pan X in screen pixels.
    on_scroll: EventHandler<f64>,
) -> Element {
    let bar_h = SCROLLBAR_SIZE;
    let metrics = scrollbar_metrics(track_length, document_width, pan_x, zoom);
    let (visible_width, total, max_doc_offset) = scroll_extents(track_length, document_width, zoom);
    let track_style = horizontal_track_style(bar_h);
    let thumb_pos_pct = metrics.thumb_pos_pct;
    let thumb_size_pct = metrics.thumb_size_pct;

    rsx! {
        div {
            class: "scrollbar scrollbar-horizontal",
            style: "{track_style}",
            onclick: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let click_ratio = coords.x / track_length;
                let new_doc_offset =
                    (click_ratio * total - visible_width / 2.0).clamp(0.0, max_doc_offset);
                let new_pan_x = -new_doc_offset * zoom;
                on_scroll.call(new_pan_x);
            },

            // Thumb
            div {
                style: "position: absolute; left: {thumb_pos_pct}%; top: 2px; width: {thumb_size_pct}%; min-width: {MIN_THUMB_SIZE}px; max-width: 100%; height: {bar_h - 4.0}px; background: #555; border-radius: 3px; cursor: pointer;",
                // Prevent click from propagating to track
                onclick: move |evt: Event<dioxus::events::MouseData>| {
                    evt.stop_propagation();
                },
            }
        }
    }
}

/// A vertical scrollbar linked to the viewport pan.
///
/// Shows a thumb whose position and size are proportional to the viewport
/// height relative to the full document height.
#[component]
pub fn VerticalScrollbar(
    /// Track length in pixels (the available scrollbar height).
    track_length: f64,
    /// The total document height in document coordinates.
    document_height: f64,
    /// Current viewport pan Y offset (screen pixels).
    pan_y: f64,
    /// Current zoom factor.
    zoom: f64,
    /// Called when the user clicks on the scrollbar track to scroll.
    /// The value is the desired new pan Y in screen pixels.
    on_scroll: EventHandler<f64>,
) -> Element {
    let bar_w = SCROLLBAR_SIZE;
    let metrics = scrollbar_metrics(track_length, document_height, pan_y, zoom);
    let (visible_height, total, max_doc_offset) =
        scroll_extents(track_length, document_height, zoom);
    let track_style = vertical_track_style(bar_w);
    let thumb_pos_pct = metrics.thumb_pos_pct;
    let thumb_size_pct = metrics.thumb_size_pct;

    rsx! {
        div {
            class: "scrollbar scrollbar-vertical",
            style: "{track_style}",
            onclick: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let click_ratio = coords.y / track_length;
                let new_doc_offset =
                    (click_ratio * total - visible_height / 2.0).clamp(0.0, max_doc_offset);
                let new_pan_y = -new_doc_offset * zoom;
                on_scroll.call(new_pan_y);
            },

            // Thumb
            div {
                style: "position: absolute; left: 2px; top: {thumb_pos_pct}%; width: {bar_w - 4.0}px; height: {thumb_size_pct}%; min-height: {MIN_THUMB_SIZE}px; max-height: 100%; background: #555; border-radius: 3px; cursor: pointer;",
                // Prevent click from propagating to track
                onclick: move |evt: Event<dioxus::events::MouseData>| {
                    evt.stop_propagation();
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_styles_stretch_to_grid_slot() {
        assert!(horizontal_track_style(SCROLLBAR_SIZE).contains("width: 100%"));
        assert!(vertical_track_style(SCROLLBAR_SIZE).contains("height: 100%"));
    }

    #[test]
    fn scrollbar_metrics_clamp_thumb_to_track() {
        let metrics = scrollbar_metrics(12.0, 500.0, 0.0, 1.0);
        assert_eq!(metrics.thumb_size, 12.0);
        assert_eq!(metrics.thumb_pos, 0.0);
        assert_eq!(metrics.thumb_size_pct, 100.0);
        assert_eq!(metrics.thumb_pos_pct, 0.0);
    }

    #[test]
    fn scrollbar_metrics_track_pan_position() {
        let metrics = scrollbar_metrics(100.0, 200.0, -50.0, 1.0);
        assert!((metrics.thumb_size - 50.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_pos - 25.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_size_pct - 50.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_pos_pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_metrics_activate_when_viewport_is_narrower_than_document() {
        let metrics = scrollbar_metrics(600.0, 800.0, 0.0, 1.0);
        assert!((metrics.thumb_size - 450.0).abs() < f64::EPSILON);
        assert_eq!(metrics.thumb_pos, 0.0);
        assert!((metrics.thumb_size_pct - 75.0).abs() < f64::EPSILON);

        let scrolled = scrollbar_metrics(600.0, 800.0, -200.0, 1.0);
        assert!((scrolled.thumb_pos - 150.0).abs() < f64::EPSILON);
        assert!((scrolled.thumb_pos_pct - 25.0).abs() < f64::EPSILON);
    }
}
