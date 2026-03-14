//! Scrollbar components (horizontal and vertical) that reflect the viewport
//! position relative to the document extents.

use dioxus::prelude::*;

/// Minimum thumb size in pixels to keep it visible and clickable.
const MIN_THUMB_SIZE: f64 = 20.0;

/// Scrollbar thickness in pixels.
const SCROLLBAR_SIZE: f64 = 14.0;

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

    // Compute visible range in document coords.
    let visible_width = track_length / zoom;
    let total = document_width.max(visible_width);

    // Thumb size and position as fractions of the track.
    let thumb_ratio = (visible_width / total).min(1.0);
    let thumb_size = (thumb_ratio * track_length).max(MIN_THUMB_SIZE);
    let available_track = track_length - thumb_size;

    // Document offset of the left edge of the viewport.
    let doc_offset = -pan_x / zoom;
    let scroll_ratio = if total > visible_width {
        (doc_offset / (total - visible_width)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_pos = scroll_ratio * available_track;

    let tl = track_length as i64;

    rsx! {
        div {
            class: "scrollbar scrollbar-horizontal",
            style: "position: relative; width: {tl}px; height: {bar_h}px; background: #1e1e1e; border-top: 1px solid #444;",
            onclick: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let click_ratio = coords.x / track_length;
                let new_doc_offset = click_ratio * total - visible_width / 2.0;
                let new_pan_x = -new_doc_offset * zoom;
                on_scroll.call(new_pan_x);
            },

            // Thumb
            div {
                style: "position: absolute; left: {thumb_pos}px; top: 2px; width: {thumb_size}px; height: {bar_h - 4.0}px; background: #555; border-radius: 3px; cursor: pointer;",
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

    // Compute visible range in document coords.
    let visible_height = track_length / zoom;
    let total = document_height.max(visible_height);

    // Thumb size and position as fractions of the track.
    let thumb_ratio = (visible_height / total).min(1.0);
    let thumb_size = (thumb_ratio * track_length).max(MIN_THUMB_SIZE);
    let available_track = track_length - thumb_size;

    // Document offset of the top edge of the viewport.
    let doc_offset = -pan_y / zoom;
    let scroll_ratio = if total > visible_height {
        (doc_offset / (total - visible_height)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_pos = scroll_ratio * available_track;

    let tl = track_length as i64;

    rsx! {
        div {
            class: "scrollbar scrollbar-vertical",
            style: "position: relative; width: {bar_w}px; height: {tl}px; background: #1e1e1e; border-left: 1px solid #444;",
            onclick: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let click_ratio = coords.y / track_length;
                let new_doc_offset = click_ratio * total - visible_height / 2.0;
                let new_pan_y = -new_doc_offset * zoom;
                on_scroll.call(new_pan_y);
            },

            // Thumb
            div {
                style: "position: absolute; left: 2px; top: {thumb_pos}px; width: {bar_w - 4.0}px; height: {thumb_size}px; background: #555; border-radius: 3px; cursor: pointer;",
                // Prevent click from propagating to track
                onclick: move |evt: Event<dioxus::events::MouseData>| {
                    evt.stop_propagation();
                },
            }
        }
    }
}
